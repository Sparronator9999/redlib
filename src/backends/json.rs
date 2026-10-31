//! Reddit's OAuth JSON API with the instance's anonymous app token.

use super::{check_json, non_json_failure, Backend, Failure, Fetch, Kind};
use crate::client::{
	burn_rate_limit_unit, current_emulation, fingerprint_blocked, is_fingerprint_block, reddit_get, IntoHyperResponse, OAUTH_IS_ROLLING_OVER, OAUTH_RATELIMIT_REMAINING,
};
use crate::oauth::force_refresh_token;
use bytes::Buf;
use log::{error, trace, warn};
use serde_json::Value;
use std::sync::atomic::Ordering;
use wreq::header as wreq_header;

pub struct Json;

impl Backend for Json {
	fn name(&self) -> &'static str {
		"json"
	}

	fn fetch<'a>(&'a self, path: &'a str, quarantine: bool) -> Fetch<'a> {
		Box::pin(fetch(path, quarantine))
	}
}

/// Only listings tell: threads and user pages are blocked for every fingerprint.
fn blames_fingerprint(path: &str) -> bool {
	Kind::of(path) == Kind::Listing
}

async fn fetch(path: &str, quarantine: bool) -> Result<Value, Failure> {
	let path = &*Kind::json_path(path);
	// First, handle rolling over the OAUTH_CLIENT if need be.
	let current_rate_limit = OAUTH_RATELIMIT_REMAINING.load(Ordering::SeqCst);
	let is_rolling_over = OAUTH_IS_ROLLING_OVER.load(Ordering::SeqCst);
	if current_rate_limit < 10 && !is_rolling_over {
		warn!("Rate limit {current_rate_limit} is low. Spawning force_refresh_token()");
		tokio::spawn(force_refresh_token());
	}
	burn_rate_limit_unit();

	let emulation = current_emulation();
	let response = reddit_get(path.to_string(), quarantine)
		.await
		.map_err(|e| Failure::Network(format!("Couldn't send request to Reddit: {e} | {path}")))?;

	// Reddit's network security block is an HTML page; private and
	// quarantined subreddits answer with JSON 403s.
	let status = response.status();
	let content_type = response.headers().get(wreq_header::CONTENT_TYPE).map(|v| v.as_bytes().to_vec());
	if status == wreq::StatusCode::FORBIDDEN && content_type.as_ref().is_some_and(|v| v.starts_with(b"text/html")) {
		let body = response.bytes().await.unwrap_or_default();
		if blames_fingerprint(path) && is_fingerprint_block(status.as_u16(), content_type.as_deref(), &body) {
			fingerprint_blocked(emulation);
		}
		return Err(Failure::Blocked);
	}

	let reset = match (
		response.headers().get("x-ratelimit-remaining").and_then(|val| val.to_str().ok().map(|s| s.to_string())),
		response.headers().get("x-ratelimit-reset").and_then(|val| val.to_str().ok().map(|s| s.to_string())),
		response.headers().get("x-ratelimit-used").and_then(|val| val.to_str().ok().map(|s| s.to_string())),
	) {
		(Some(remaining), Some(reset), Some(used)) => {
			trace!(
				"Ratelimit remaining: Header says {remaining}, we have {current_rate_limit}. Resets in {reset}. Rollover: {}. Ratelimit used: {used}",
				if is_rolling_over { "yes" } else { "no" },
			);

			// If can parse remaining as a float, round to a u16 and save
			if let Ok(val) = remaining.parse::<f32>() {
				OAUTH_RATELIMIT_REMAINING.store(val.round() as u16, Ordering::SeqCst);
			}

			Some(reset)
		}
		_ => None,
	};

	let body = response
		.into_hyper_response()
		.into_body()
		.collect_bytes()
		.await
		.map_err(|e| Failure::Network(format!("Failed receiving body from Reddit: {e} | {path}")))?;

	if !body.has_remaining() {
		// Rate limited, so spawn a force_refresh_token()
		tokio::spawn(force_refresh_token());
		return Err(Failure::RateLimited(match reset {
			Some(val) => format!(
				"Reddit rate limit exceeded. Try refreshing in a few seconds.\
				 Rate limit will reset in: {val}"
			),
			None => "Reddit rate limit exceeded".to_string(),
		}));
	}

	match serde_json::from_reader(body.reader()) {
		Ok(json) => check_json(json, path).await,
		Err(e) => {
			error!("Got an invalid response from reddit {e}. Status code: {status}");
			Err(non_json_failure(status.as_u16(), &e.to_string(), path))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_only_listing_blocks_blame_the_fingerprint() {
		assert!(blames_fingerprint("/r/rust/hot.json?raw_json=1"));
		assert!(blames_fingerprint("/r/popular/hot.json?geo_filter=GLOBAL&raw_json=1"));
		assert!(!blames_fingerprint("/r/rust/comments/abc123.json?raw_json=1"));
		assert!(!blames_fingerprint("/user/spez/about.json?raw_json=1"));
	}
}
