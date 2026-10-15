//! The check behind a link to a constraint declared outside the policy:
//! only an IRI that finally answers HTTP 200 is linked. Run against a
//! server on the loopback interface; nothing here reaches the network.
use compliance_catalog::links::{answers_200, cached};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(2);

/// A server on 127.0.0.1 that answers by method and path; its base URL.
fn server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || answer(stream));
        }
    });
    base
}

fn answer(mut stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() {
        return;
    }
    // The headers, up to the blank line.
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
        line.clear();
    }
    let mut parts = first.split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let (status, location) = match (method, path) {
        (_, "/ok") => ("200 OK", None),
        (_, "/missing") => ("404 Not Found", None),
        (_, "/moved") => ("301 Moved Permanently", Some("/ok")),
        (_, "/moved-away") => ("302 Found", Some("/missing")),
        (_, "/no-content") => ("204 No Content", None),
        ("HEAD", "/get-only") => ("405 Method Not Allowed", None),
        ("GET", "/get-only") => ("200 OK", None),
        (_, "/slow") => {
            std::thread::sleep(Duration::from_secs(10));
            ("200 OK", None)
        }
        _ => ("500 Internal Server Error", None),
    };
    let location = location.map_or(String::new(), |l| format!("Location: {l}\r\n"));
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\n{location}Content-Length: 0\r\nConnection: close\r\n\r\n"
    );
}

#[test]
fn a_200_is_a_link() {
    assert!(answers_200(&format!("{}/ok", server()), TIMEOUT));
}

#[test]
fn a_redirect_counts_by_where_it_ends() {
    let base = server();
    assert!(answers_200(&format!("{base}/moved"), TIMEOUT));
    assert!(!answers_200(&format!("{base}/moved-away"), TIMEOUT));
}

#[test]
fn anything_but_a_final_200_is_not_a_link() {
    let base = server();
    for path in ["/missing", "/no-content", "/broken"] {
        assert!(!answers_200(&format!("{base}{path}"), TIMEOUT), "{path}");
    }
}

#[test]
fn a_server_that_refuses_head_is_asked_with_get() {
    assert!(answers_200(&format!("{}/get-only", server()), TIMEOUT));
}

#[test]
fn a_slow_server_is_given_up_on() {
    let start = Instant::now();
    assert!(!answers_200(
        &format!("{}/slow", server()),
        Duration::from_millis(500)
    ));
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn an_unreachable_host_is_not_an_error() {
    // A port that was free a moment ago: nothing listens there.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert!(!answers_200(
        &format!("http://127.0.0.1:{port}/ok"),
        TIMEOUT
    ));
}

#[test]
fn only_http_iris_are_checked() {
    for iri in [
        "urn:c:1",
        "file:///etc/hostname",
        "ftp://example.com/c",
        "http",
    ] {
        assert!(!answers_200(iri, TIMEOUT), "{iri}");
    }
}

#[test]
fn each_iri_is_checked_once() {
    let mut asked = Vec::new();
    {
        let mut check = cached(|iri: &str| {
            asked.push(iri.to_string());
            iri.ends_with("/ok")
        });
        assert!(check("https://x/ok"));
        assert!(!check("https://x/no"));
        assert!(check("https://x/ok"));
        assert!(!check("https://x/no"));
    }
    assert_eq!(asked, ["https://x/ok", "https://x/no"]);
}
