//! Loading a profile from a URL: the one place the demo goes on the
//! network, and only when the user asks. The checks that decide what is
//! fetched and read are in `profile_field`, where the host tests reach them.
use gloo_net::http::Request;
use gloo_timers::callback::Timeout;
use web_sys::{AbortController, RequestCredentials, RequestMode};

use crate::profile_field::{MAX_BYTES, url_problem};

/// JSON-LD first; Turtle and plain JSON are read too.
const ACCEPT: &str = "application/ld+json, text/turtle;q=0.95, application/json;q=0.9";
const TIMEOUT_MS: u32 = 15_000;
const NETWORK: &str = "Network error, or the server does not allow cross-origin requests (CORS).";

/// `url` resolved against the document's base URI, if it is one the page
/// may fetch.
pub fn target(url: &str) -> Result<String, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Enter the profile's URL first.".to_string());
    }
    let window = web_sys::window().ok_or("No window.")?;
    let document = window.document().ok_or("No document.")?;
    let base = document
        .base_uri()
        .ok()
        .flatten()
        .unwrap_or_else(|| window.location().href().unwrap_or_default());
    let href = web_sys::Url::new_with_base(url, &base)
        .map_err(|_| format!("{url} is not a URL."))?
        .href();
    let page = window.location().href().unwrap_or_default();
    match url_problem(&page, &href) {
        Some(msg) => Err(msg),
        None => Ok(href),
    }
}

/// The body and `Content-Type` of `url`: without credentials, with a
/// 15-second limit, and refused past 4 MiB (by `Content-Length` when the
/// server sends it, else once read).
pub async fn fetch(url: &str) -> Result<(String, Option<String>), String> {
    let controller = AbortController::new().map_err(|_| NETWORK.to_string())?;
    let signal = controller.signal();
    // Dropping the timeout cancels it.
    let _timeout = Timeout::new(TIMEOUT_MS, move || controller.abort());
    let failed = |signal: &web_sys::AbortSignal| {
        if signal.aborted() {
            "No answer within 15 seconds.".to_string()
        } else {
            NETWORK.to_string()
        }
    };
    let resp = Request::get(url)
        .mode(RequestMode::Cors)
        .credentials(RequestCredentials::Omit)
        .header("Accept", ACCEPT)
        .abort_signal(Some(&signal))
        .send()
        .await
        .map_err(|_| failed(&signal))?;
    if !resp.ok() {
        return Err(format!(
            "The server answered {} {}.",
            resp.status(),
            resp.status_text()
        )
        .replace(" .", "."));
    }
    let headers = resp.headers();
    let too_large =
        |n: usize| format!("The profile is {n} bytes; profiles over 4 MiB are not read.");
    if let Some(n) = headers
        .get("content-length")
        .and_then(|l| l.trim().parse::<usize>().ok())
        && n > MAX_BYTES
    {
        return Err(too_large(n));
    }
    let text = resp.text().await.map_err(|_| failed(&signal))?;
    if text.len() > MAX_BYTES {
        return Err(too_large(text.len()));
    }
    Ok((text, headers.get("content-type")))
}
