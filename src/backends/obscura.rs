use super::{check_json, non_json_failure, Backend, Failure, Fetch};
use crate::client::session_cookies;
use log::{trace, warn};
use obscura::Browser;
use serde_json::Value;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, OnceCell};

const WEB_BASE: &str = "https://www.reddit.com";
const TIMEOUT: Duration = Duration::from_secs(15);

type Reply = Result<Fetched, String>;
type Fetched = (u16, Vec<u8>);

struct Job {
	path: String,
	reply: oneshot::Sender<Reply>,
}

struct Session {
	label: String,
	cookie: String,
	browser: OnceCell<Option<Browser>>,
}

static NEXT_SESSION: AtomicUsize = AtomicUsize::new(0);

// Obscura pages hold V8 isolates, which can't leave their thread, so every
// request runs on one dedicated thread.
static WORKER: LazyLock<mpsc::UnboundedSender<Job>> = LazyLock::new(|| {
	let (tx, rx) = mpsc::unbounded_channel();
	std::thread::Builder::new()
		.name("obscura".into())
		.spawn(move || worker(rx))
		.expect("Should always be able to spawn the obscura thread");
	tx
});

/// Reddit's web host through Obscura with the logged-in sessions from `REDLIB_REDDIT_COOKIE`.
pub struct ObscuraCookiedJson;

impl Backend for ObscuraCookiedJson {
	fn name(&self) -> &'static str {
		"obscura-cookied-json"
	}

	/// Only with a logged-in session: Reddit refuses logged-out `.json`
	/// requests on its web host.
	fn available(&self) -> bool {
		!session_cookies().is_empty()
	}

	fn logged_in(&self) -> bool {
		true
	}

	fn cost(&self) -> f64 {
		2.0
	}

	fn fetch<'a>(&'a self, path: &'a str, _quarantine: bool) -> Fetch<'a> {
		Box::pin(async move {
			let (status, body) = get(path).await.map_err(|e| Failure::Network(format!("Couldn't send request to Reddit: {e} | {path}")))?;
			match serde_json::from_slice::<Value>(&body) {
				Ok(json) => check_json(json, path).await,
				Err(e) => Err(match status {
					429 | 500..=599 => non_json_failure(status, &e.to_string(), path),
					_ => Failure::Blocked,
				}),
			}
		})
	}
}

/// Fetches `path` from www.reddit.com with the configured sessions in turn and
/// returns the status and body of the first success, or of the last refusal.
async fn get(path: &str) -> Reply {
	let (reply, rx) = oneshot::channel();
	let job = Job { path: path.to_string(), reply };
	WORKER.send(job).map_err(|_| "Obscura thread is gone".to_string())?;
	rx.await.map_err(|_| "Obscura request was dropped".to_string())?
}

fn worker(mut rx: mpsc::UnboundedReceiver<Job>) {
	let rt = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("Should always be able to build the obscura runtime");
	let local = tokio::task::LocalSet::new();
	local.block_on(&rt, async move {
		let mut sessions: Vec<Rc<Session>> = Vec::new();
		while let Some(job) = rx.recv().await {
			// The sessions can change while running; a session that stays keeps its browser.
			sessions = session_cookies()
				.into_iter()
				.map(|(label, cookie)| match sessions.iter().find(|session| session.cookie == cookie) {
					Some(session) => session.clone(),
					None => Rc::new(Session {
						label,
						cookie,
						browser: OnceCell::new(),
					}),
				})
				.collect();
			let sessions = sessions.clone();
			tokio::task::spawn_local(async move {
				let _ = job.reply.send(fetch(&sessions, &job.path).await);
			});
		}
	});
}

/// Starts a browser holding the session's cookies, on the session's first request.
///
/// A real browser carries the tracking cookies Reddit sets on its pages
/// (`loid`, `session_tracker`, ...), so the front page is loaded once first.
async fn start_browser(session: &Session) -> Option<Browser> {
	let browser = match Browser::builder().stealth(true).build() {
		Ok(browser) => browser,
		Err(e) => {
			warn!("Couldn't start Obscura for session {}: {e}", session.label);
			return None;
		}
	};
	for pair in session.cookie.split(';').map(str::trim).filter(|pair| !pair.is_empty()) {
		if browser.cookies().set(&format!("{pair}; Domain=.reddit.com; Path=/; Secure"), WEB_BASE).is_err() {
			warn!("Couldn't load session {} into Obscura", session.label);
			return None;
		}
	}
	let _ = navigate(&browser, &format!("{WEB_BASE}/")).await;
	Some(browser)
}

async fn fetch(sessions: &[Rc<Session>], path: &str) -> Reply {
	let start = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
	let mut last = Err("No usable session for Obscura".to_string());
	for i in 0..sessions.len() {
		let session = &sessions[(start + i) % sessions.len()];
		let Some(browser) = session.browser.get_or_init(|| start_browser(session)).await else {
			continue;
		};
		last = navigate(browser, &format!("{WEB_BASE}{path}")).await;
		match &last {
			Ok((status, _)) if (200..300).contains(status) => {
				trace!("Fetched {path} with session {}", session.label);
				break;
			}
			Ok((status, _)) => warn!("Session {} was refused for {path}: status {status}", session.label),
			Err(e) => warn!("Couldn't reach Reddit for {path}: {e}"),
		}
	}
	last
}

/// Loads `url` in a new page and returns the document response as received.
async fn navigate(browser: &Browser, url: &str) -> Reply {
	let mut page = browser.new_page().await.map_err(|e| e.to_string())?;
	let response: Arc<Mutex<Option<Fetched>>> = Arc::default();
	let slot = response.clone();
	page.on_response(Arc::new(move |_, resp| {
		let mut slot = slot.lock().unwrap();
		if slot.is_none() {
			*slot = Some((resp.status, resp.body.clone()));
		}
	}));
	match tokio::time::timeout(TIMEOUT, page.goto(url)).await {
		Ok(Ok(())) => {}
		Ok(Err(e)) => return Err(e.to_string()),
		Err(_) => return Err(format!("timed out after {}s", TIMEOUT.as_secs())),
	}
	let response = response.lock().unwrap().take();
	response.ok_or_else(|| "Obscura got no response".into())
}
