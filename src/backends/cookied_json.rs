//! Reddit's web host with the logged-in sessions from `REDLIB_REDDIT_COOKIE`.

use super::{check_json, Backend, Failure, Fetch, Kind};
use crate::client::{reroll_client, session_cookies, IntoHyperResponse, ALTERNATIVE_REDDIT_URL_BASE, CLIENT};
use bytes::Buf;
use log::{trace, warn};
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use wreq::{header as wreq_header, Response as WreqResponse};

pub struct CookiedJson;

impl Backend for CookiedJson {
	fn name(&self) -> &'static str {
		"cookied-json"
	}

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
		Box::pin(fetch(path))
	}
}

/// Index of the session the next request starts with.
static NEXT_SESSION: AtomicUsize = AtomicUsize::new(0);

/// Sessions take turns; a refused one passes the request on to the next.
/// When every session is refused, Reddit may have flagged the browser
/// emulation, so switch to another one and try once more.
async fn fetch(path: &str) -> Result<Value, Failure> {
	let path = &*Kind::json_path(path);
	let sessions = session_cookies();
	let mut response = web_session_get(path, &sessions).await;
	if response.is_none() && reroll_client("Reddit refused every session") {
		response = web_session_get(path, &sessions).await;
	}
	let response = response.ok_or(Failure::Blocked)?;

	let body = response
		.into_hyper_response()
		.into_body()
		.collect_bytes()
		.await
		.map_err(|e| Failure::Network(format!("Failed receiving body from Reddit: {e} | {path}")))?;
	// An empty or non-JSON body from the web host is a refusal, not the
	// OAuth rate limit.
	match serde_json::from_reader(body.reader()) {
		Ok(json) => check_json(json, path).await,
		Err(_) => Err(Failure::Blocked),
	}
}

/// Tries `path` on Reddit's web host with each session in turn, starting at a
/// rotating index, and returns the first successful response.
async fn web_session_get(path: &str, sessions: &[(String, String)]) -> Option<WreqResponse> {
	let start = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
	for i in 0..sessions.len() {
		let (label, cookie) = &sessions[(start + i) % sessions.len()];
		match reddit_web_get(path, cookie).await {
			Ok(web_response) if web_response.status().is_success() => {
				trace!("Retried {path} with session {label}: status {}", web_response.status());
				return Some(web_response);
			}
			Ok(web_response) => warn!("Session {label} was refused for {path}: status {}", web_response.status()),
			Err(e) => warn!("Couldn't reach Reddit's web host for {path}: {e}"),
		}
	}
	None
}

/// Fetches `path` from Reddit's web host as the logged-in session `cookie`.
///
/// Only the cookie is overridden: the client's browser emulation already sends
/// the TLS, HTTP/2 and default headers Reddit expects next to a session cookie,
/// and a mismatch between them is what gets a cookie refused. Redirects are not
/// followed, because Reddit sends an expired session to its login page and that
/// must be reported rather than parsed as JSON.
async fn reddit_web_get(path: &str, cookie: &str) -> Result<WreqResponse, String> {
	let url = format!("{ALTERNATIVE_REDDIT_URL_BASE}{path}");

	CLIENT.load_full().get(&url).header(wreq_header::COOKIE, cookie).send().await.map_err(|e| e.to_string())
}
