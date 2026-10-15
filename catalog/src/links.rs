//! Which IRIs a catalog page links. A policy may refer to a constraint it
//! does not declare; the reading names the constraint's IRI, and the page
//! links it only when the IRI answers HTTP 200.
//!
//! The check runs `curl`, which follows the build host's proxy and CA
//! settings and adds no HTTP or TLS stack to the workspace. Whatever goes
//! wrong (no curl, a network error, a timeout) reads as "does not answer":
//! the IRI stays plain text and the build goes on.
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::time::Duration;

/// How long one request of a check may take.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// Whether `iri`, an `http` or `https` URL, finally answers 200: to HEAD,
/// else to GET (some servers refuse HEAD), after at most five redirects,
/// each request within `timeout`. Any other IRI is not checked.
pub fn answers_200(iri: &str, timeout: Duration) -> bool {
    if !(iri.starts_with("http://") || iri.starts_with("https://")) {
        return false;
    }
    match status(iri, timeout, true) {
        Some(200) => true,
        Some(_) => status(iri, timeout, false) == Some(200),
        None => false,
    }
}

/// The status of the last response to a HEAD (or GET) of `url`, following
/// redirects; `None` when there is none.
fn status(url: &str, timeout: Duration, head: bool) -> Option<u16> {
    let mut curl = Command::new("curl");
    curl.args(["--silent", "--location", "--max-redirs", "5"])
        .args(["--proto", "=http,https", "--proto-redir", "=http,https"])
        .args(["--output", "/dev/null", "--write-out", "%{http_code}"])
        .arg("--max-time")
        .arg(format!("{:.3}", timeout.as_secs_f64()));
    if head {
        curl.arg("--head");
    }
    let out = curl
        .arg("--url")
        .arg(url)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

/// `check`, asked at most once per IRI.
pub fn cached(mut check: impl FnMut(&str) -> bool) -> impl FnMut(&str) -> bool {
    let mut answered = HashMap::new();
    move |iri| {
        *answered
            .entry(iri.to_string())
            .or_insert_with(|| check(iri))
    }
}
