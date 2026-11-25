//! Which IRIs a catalog page links. A policy may refer to a constraint it
//! does not declare, or name the terms an `acceptedTerms` constraint accepts
//! by a URL in a plain string; the page links either only when it answers
//! HTTP 200.
//!
//! The check runs `curl`, which follows the build host's proxy and CA
//! settings and adds no HTTP or TLS stack to the workspace. It reads the
//! response headers as they come and stops the transfer at the final status,
//! so a big document is not downloaded. Whatever goes wrong (no curl, a
//! network error, a timeout) reads as "does not answer": the IRI stays plain
//! text and the build goes on.
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// How long the check of a constraint reference may take.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// How long the check of a URL of accepted terms may take, unless the build
/// says otherwise (`--terms-timeout <ms>`).
pub const TERMS_TIMEOUT: Duration = Duration::from_millis(200);

/// Whether `iri`, an `http` or `https` URL, finally answers 200 to a GET,
/// after at most five redirects, all within `timeout`. The transfer stops as
/// soon as the final status is in: no body is read. Any other IRI is not
/// checked.
pub fn answers_200(iri: &str, timeout: Duration) -> bool {
    if !(iri.starts_with("http://") || iri.starts_with("https://")) {
        return false;
    }
    status(iri, timeout) == Some(200)
}

/// The final status of a GET of `url`, following redirects; `None` when
/// none came within `timeout`.
fn status(url: &str, timeout: Duration) -> Option<u16> {
    let mut curl = Command::new("curl")
        .args(["--silent", "--location", "--max-redirs", "5"])
        .args(["--proto", "=http,https", "--proto-redir", "=http,https"])
        // Headers to stdout, as they come; the body is never written.
        .args(["--dump-header", "-", "--output", "/dev/null"])
        .arg("--max-time")
        .arg(format!("{:.3}", timeout.as_secs_f64()))
        .arg("--url")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let headers = BufReader::new(curl.stdout.take()?);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(final_status(headers.lines().map_while(Result::ok)));
    });
    let got = rx.recv_timeout(timeout).ok().flatten();
    // Stops the transfer, whether or not a status came.
    let _ = curl.kill();
    let _ = curl.wait();
    got
}

/// The status of the last response in `lines`, the headers of each response
/// in turn: a 1xx is interim, and a 3xx with a `Location` is followed.
/// Returns at the blank line that ends the final response's headers.
fn final_status(lines: impl Iterator<Item = String>) -> Option<u16> {
    let mut code = None;
    let mut moved = false;
    for line in lines {
        let line = line.trim_end();
        if line.starts_with("HTTP/") {
            code = line.split_whitespace().nth(1)?.parse::<u16>().ok();
            moved = false;
        } else if line.to_ascii_lowercase().starts_with("location:") {
            moved = true;
        } else if line.is_empty() {
            match code {
                Some(c) if (100..200).contains(&c) => {}
                Some(c) if (300..400).contains(&c) && moved => {}
                c => return c,
            }
        }
    }
    None
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
