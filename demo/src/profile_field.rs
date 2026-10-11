//! The Profile field: one textarea that holds the profile the prose is
//! worded with, shared by the Read and Edit tabs. Applying it is explicit;
//! typing never parses. The state is a pure reducer, tested on the host.
use std::rc::Rc;

use prose_yew::SharedWording;
use prose_yew::prose_core::profile::{ProfileLimits, ProfileVocab, TermKind, Wording};
use serde_json::Value;
use yew::Reducible;

/// A profile larger than this is not read (prose-core's default limit).
pub const MAX_BYTES: usize = 4 * 1024 * 1024;

/// A profile the field offers as a button.
pub struct Preset {
    /// The button's id.
    pub id: &'static str,
    pub label: &'static str,
    pub text: &'static str,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "profile-sample",
        label: "Use the sample research profile",
        text: include_str!("../../prose-core/tests/profiles/research.jsonld"),
    },
    Preset {
        id: "profile-sample-ttl",
        label: "The same, in Turtle",
        text: include_str!("../../prose-core/tests/profiles/research.ttl"),
    },
    Preset {
        id: "profile-odrl22",
        label: "W3C ODRL 2.2 vocabulary",
        text: include_str!("../profiles/odrl22-trimmed.json"),
    },
];

#[derive(Clone, Debug, PartialEq)]
pub enum ProfileMsg {
    /// The user typed in the field.
    Edit(String),
    /// Read the field's text as the profile.
    Apply,
    /// Fill the field with this text and apply it (a preset).
    Use(String),
    /// A file read or a fetch starts; what an older one brings is ignored.
    Begin,
    /// A file was read for the load started at `generation`.
    FileRead { generation: u32, text: String },
    /// The load started at `generation` failed.
    Failed { generation: u32, msg: String },
    /// A fetch started at `generation` answered with this body and
    /// `Content-Type`, from an address that is not known.
    Fetched {
        generation: u32,
        text: String,
        content_type: Option<String>,
    },
    /// As `Fetched`, from `url`: relative IRIs in the profile are read
    /// against it.
    FetchedFrom {
        generation: u32,
        url: String,
        text: String,
        content_type: Option<String>,
    },
    /// The user typed in the URL field (or it was prefilled).
    Url(String),
    /// Empty the field and use no profile.
    Clear,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileState {
    /// The field's text.
    pub text: String,
    /// The profile last applied from the field; `None` reads as before.
    pub wording: Option<SharedWording>,
    pub error: Option<String>,
    /// What is loaded, in one sentence.
    pub status: Option<String>,
    /// What the reader could not read, or read with a guess.
    pub warnings: Vec<String>,
    /// The URL field's text.
    pub url: String,
    /// The address the field's text was fetched from: its relative IRIs are
    /// read against it. Kept while the text is edited; a preset, a file or
    /// Clear drops it.
    pub base: Option<String>,
    /// Bumped by `Begin` and `Clear`: a load that started before is stale.
    pub generation: u32,
    pub loading: bool,
}

/// The state after `m`. A newer user action (Apply, a preset, Clear) also
/// makes a load in flight stale.
pub fn reduce(s: &ProfileState, m: ProfileMsg) -> ProfileState {
    let mut n = s.clone();
    match m {
        ProfileMsg::Edit(text) => n.text = text,
        ProfileMsg::Apply => {
            n.generation = n.generation.wrapping_add(1);
            n.loading = false;
            apply(&mut n);
        }
        ProfileMsg::Use(text) => {
            n.generation = n.generation.wrapping_add(1);
            n.loading = false;
            n.text = text;
            n.base = None;
            apply(&mut n);
        }
        ProfileMsg::Begin => {
            n.generation = n.generation.wrapping_add(1);
            n.loading = true;
        }
        ProfileMsg::FileRead { generation, text } if generation == s.generation => {
            n.loading = false;
            n.text = text;
            n.base = None;
            apply(&mut n);
        }
        // The field still holds what was applied, so that profile stays.
        ProfileMsg::Failed { generation, msg } if generation == s.generation => {
            n.loading = false;
            n.error = Some(msg);
        }
        ProfileMsg::FileRead { .. } | ProfileMsg::Failed { .. } => {}
        ProfileMsg::Fetched {
            generation,
            text,
            content_type,
        } if generation == s.generation => fetched(&mut n, None, text, content_type),
        ProfileMsg::FetchedFrom {
            generation,
            url,
            text,
            content_type,
        } if generation == s.generation => fetched(&mut n, Some(url), text, content_type),
        ProfileMsg::Fetched { .. } | ProfileMsg::FetchedFrom { .. } => {}
        ProfileMsg::Url(url) => n.url = url,
        ProfileMsg::Clear => {
            n = ProfileState {
                generation: s.generation.wrapping_add(1),
                url: s.url.clone(),
                ..ProfileState::default()
            }
        }
    }
    n
}

/// A fetched answer, from `url` when it is known.
fn fetched(n: &mut ProfileState, url: Option<String>, text: String, content_type: Option<String>) {
    n.loading = false;
    match content_type_problem(content_type.as_deref()) {
        // The field still holds what was applied, so that profile stays.
        Some(msg) => n.error = Some(msg),
        None => {
            n.text = text;
            n.base = url;
            apply(n);
        }
    }
}

/// Why `target` (an absolute URL) is not fetched from the page at `page`,
/// if it is not: an http:// URL from an https page is blocked by the
/// browser as mixed content, and only http(s) and blob: URLs are loaded.
pub fn url_problem(page: &str, target: &str) -> Option<String> {
    let scheme = |u: &str| u.split_once(':').map(|(s, _)| s.to_ascii_lowercase());
    match scheme(target).as_deref() {
        Some("https" | "blob") => None,
        Some("http") if scheme(page).as_deref() == Some("https") => {
            Some("The browser blocks http:// URLs from this https page.".to_string())
        }
        Some("http") => None,
        _ => Some("Only http:// and https:// URLs are loaded.".to_string()),
    }
}

/// Why an answer with this `Content-Type` is not read as a profile, if it
/// is not. JSON, JSON-LD and Turtle are read; so are the generic types
/// static hosts use for any file (text/plain, octet-stream) and a missing
/// type, since the reader tells the syntax from the text itself.
pub fn content_type_problem(content_type: Option<&str>) -> Option<String> {
    const READ: &[&str] = &[
        "application/ld+json",
        "application/json",
        "text/turtle",
        "application/x-turtle",
        "text/plain",
        "application/octet-stream",
    ];
    let mime = content_type?
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime.is_empty() || READ.contains(&mime.as_str()) || mime.ends_with("+json") {
        return None;
    }
    Some(format!(
        "The server answered with {mime}; only JSON-LD and Turtle profiles are read."
    ))
}

/// The first profile IRI the policy declares (`profile` or
/// `odrl:profile`): of the top-level node, else of the first node in a
/// `@graph` or top-level array that declares one.
pub fn declared_profile(policy: &str) -> Option<String> {
    let v: Value = serde_json::from_str(policy).ok()?;
    let nodes: Vec<&Value> = match &v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) => match o.get("@graph") {
            Some(Value::Array(g)) => std::iter::once(&v).chain(g.iter()).collect(),
            _ => vec![&v],
        },
        _ => return None,
    };
    const KEYS: &[&str] = &[
        "profile",
        "odrl:profile",
        "http://www.w3.org/ns/odrl/2/profile",
    ];
    nodes
        .iter()
        .filter_map(|n| n.as_object())
        .flat_map(|o| KEYS.iter().filter_map(|k| o.get(*k)))
        .find_map(first_iri)
}

/// A string, an `{"@id": ..}`, or the first of an array of them.
fn first_iri(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("@id").and_then(Value::as_str).map(str::to_string),
        Value::Array(a) => a.iter().find_map(first_iri),
        _ => None,
    }
}

/// Reads the field's text as the profile. Blank text is no profile; an
/// unreadable one is an error and no profile, never the last good one.
fn apply(n: &mut ProfileState) {
    n.wording = None;
    n.error = None;
    n.status = None;
    n.warnings.clear();
    if n.text.trim().is_empty() {
        return;
    }
    let read = match &n.base {
        Some(base) => ProfileVocab::parse_with_base(&n.text, base, &ProfileLimits::default()),
        None => ProfileVocab::parse(&n.text),
    };
    match read {
        Ok(p) => {
            n.status = Some(status_line(&p));
            n.warnings = p.warnings().to_vec();
            n.wording = Some(SharedWording::from(Wording::new().with_profile(p)));
        }
        Err(e) => n.error = Some(sentence(&e.to_string())),
    }
}

/// What is loaded, for the status line: "Using {title} {version}: 3
/// actions, 1 operator." Only the kinds the edit suggestions use are
/// counted; a profile with none of them gives its number of terms.
pub fn status_line(p: &ProfileVocab) -> String {
    let mut name = match (p.title().pick(&[]), p.iri()) {
        (Some(t), _) => t.to_string(),
        (None, Some(iri)) => iri.to_string(),
        (None, None) => "an untitled profile".to_string(),
    };
    if let Some(v) = p.version()
        && !name.contains(v)
    {
        name = format!("{name} {v}");
    }
    let s = p.summary();
    let kinds = [
        (TermKind::Action, "action", "actions"),
        (TermKind::LeftOperand, "left operand", "left operands"),
        (TermKind::Operator, "operator", "operators"),
        (TermKind::RightOperand, "value", "values"),
        (TermKind::ConflictTerm, "conflict term", "conflict terms"),
    ];
    let counts: Vec<String> = kinds
        .iter()
        .filter_map(|&(k, one, many)| match s.count(k) {
            0 => None,
            n => Some(count(n, one, many)),
        })
        .collect();
    let what = if counts.is_empty() {
        count(s.terms, "term", "terms")
    } else {
        counts.join(", ")
    };
    format!("Using {name}: {what}.")
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A reader message as a sentence: capitalised, with a full stop.
fn sentence(msg: &str) -> String {
    let mut c = msg.chars();
    let mut out = match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => return String::new(),
    };
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

impl Reducible for ProfileState {
    type Action = ProfileMsg;

    fn reduce(self: Rc<Self>, m: ProfileMsg) -> Rc<Self> {
        Rc::new(reduce(&self, m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> String {
        PRESETS[0].text.to_string()
    }

    fn applied(text: &str) -> ProfileState {
        let s = reduce(&ProfileState::default(), ProfileMsg::Edit(text.to_string()));
        reduce(&s, ProfileMsg::Apply)
    }

    #[test]
    fn apply_valid_sets_wording_and_status() {
        let s = applied(&sample());
        assert!(s.wording.is_some());
        assert_eq!(s.error, None);
        assert_eq!(
            s.status.as_deref(),
            Some(
                "Using Research Data Profile 0.1.0: 3 actions, 3 left operands, 1 operator, 3 values, 1 conflict term."
            )
        );
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    }

    #[test]
    fn apply_invalid_keeps_text_sets_error_no_wording() {
        let good = applied(&sample());
        let bad = "<!doctype html><title>Profile</title>";
        let s = reduce(&good, ProfileMsg::Edit(bad.to_string()));
        let s = reduce(&s, ProfileMsg::Apply);
        assert_eq!(s.text, bad, "the text is kept");
        assert_eq!(s.wording, None, "no hidden last-good profile");
        assert_eq!(s.status, None);
        let e = s.error.expect("an error");
        assert!(e.contains("HTML page"), "{e}");
    }

    #[test]
    fn edit_does_not_parse() {
        let good = applied(&sample());
        let s = reduce(&good, ProfileMsg::Edit("{".to_string()));
        assert_eq!(s.text, "{");
        assert_eq!(s.wording, good.wording, "the applied profile stays");
        assert_eq!(s.error, None);
        assert_eq!(s.status, good.status);
    }

    #[test]
    fn apply_blank_is_no_profile() {
        let good = applied(&sample());
        let s = reduce(&good, ProfileMsg::Edit("  \n".to_string()));
        let s = reduce(&s, ProfileMsg::Apply);
        assert_eq!(s.wording, None);
        assert_eq!(s.error, None);
        assert_eq!(s.status, None);
    }

    #[test]
    fn use_fills_the_field_and_applies() {
        let s = reduce(&ProfileState::default(), ProfileMsg::Use(sample()));
        assert_eq!(s.text, sample());
        assert!(s.wording.is_some());
        assert_eq!(
            s,
            ProfileState {
                wording: s.wording.clone(),
                ..applied(&sample())
            }
        );
    }

    #[test]
    fn turtle_preset_reads_like_its_json_ld_twin() {
        let ttl = reduce(
            &ProfileState::default(),
            ProfileMsg::Use(PRESETS[1].text.into()),
        );
        let json = applied(&sample());
        assert_eq!(ttl.status, json.status);
        assert_eq!(
            ttl.wording.as_ref().map(|w| w.0.profiles().next().cloned()),
            json.wording
                .as_ref()
                .map(|w| w.0.profiles().next().cloned())
        );
    }

    #[test]
    fn odrl22_preset_reads_the_whole_vocabulary() {
        let s = reduce(
            &ProfileState::default(),
            ProfileMsg::Use(PRESETS[2].text.into()),
        );
        assert_eq!(
            s.status.as_deref(),
            Some(
                "Using ODRL Version 2.2: 72 actions, 34 left operands, 12 operators, 1 value, 3 conflict terms."
            ),
            "{:?}",
            s.error
        );
    }

    #[test]
    fn warnings_are_listed() {
        let text = r#"{"@id":"https://example.org/x","@type":"http://www.w3.org/ns/odrl/2/Action","skos:definition":"d"}"#;
        let s = applied(text);
        assert!(s.wording.is_some());
        assert!(
            s.warnings.iter().any(|w| w.contains("skos")),
            "{:?}",
            s.warnings
        );
        assert_eq!(
            s.status.as_deref(),
            Some("Using an untitled profile: 1 action."),
            "no title, and no profile node to name"
        );
    }

    #[test]
    fn clear_bumps_gen() {
        let good = applied(&sample());
        let s = reduce(&good, ProfileMsg::Clear);
        assert_eq!(s.generation, good.generation + 1);
        assert_eq!(s.text, "");
        assert_eq!(s.wording, None);
        assert_eq!(s.status, None);
        assert_eq!(s.error, None);
        assert!(s.warnings.is_empty());
    }

    #[test]
    fn stale_file_read_ignored() {
        let s = reduce(&ProfileState::default(), ProfileMsg::Begin);
        assert!(s.loading);
        let started = s.generation;
        let s = reduce(&s, ProfileMsg::Clear);
        let s = reduce(
            &s,
            ProfileMsg::FileRead {
                generation: started,
                text: sample(),
            },
        );
        assert_eq!(s.text, "", "a read started before Clear is dropped");
        assert_eq!(s.wording, None);
        assert!(!s.loading);
    }

    #[test]
    fn current_file_read_applies() {
        let s = reduce(&ProfileState::default(), ProfileMsg::Begin);
        let s = reduce(
            &s,
            ProfileMsg::FileRead {
                generation: s.generation,
                text: sample(),
            },
        );
        assert!(!s.loading);
        assert_eq!(s.text, sample());
        assert!(s.wording.is_some());
    }

    #[test]
    fn failed_load_reports_and_keeps_the_applied_profile() {
        let good = applied(&sample());
        let s = reduce(&good, ProfileMsg::Begin);
        let s = reduce(
            &s,
            ProfileMsg::Failed {
                generation: s.generation,
                msg: "too large".into(),
            },
        );
        assert_eq!(s.error.as_deref(), Some("too large"));
        assert!(!s.loading);
        // The field still holds what was applied, so the profile stays.
        assert_eq!(s.text, good.text);
        assert_eq!(s.wording, good.wording);
        // A stale failure is ignored.
        let s2 = reduce(
            &reduce(&good, ProfileMsg::Begin),
            ProfileMsg::Failed {
                generation: good.generation,
                msg: "old".into(),
            },
        );
        assert_eq!(s2.error, None);
    }

    #[test]
    fn a_new_apply_clears_the_error() {
        let s = applied("<html>");
        assert!(s.error.is_some());
        let s = reduce(&s, ProfileMsg::Use(sample()));
        assert_eq!(s.error, None);
    }

    fn fetched(s: &ProfileState, text: &str, content_type: Option<&str>) -> ProfileState {
        reduce(
            s,
            ProfileMsg::Fetched {
                generation: s.generation,
                text: text.to_string(),
                content_type: content_type.map(str::to_string),
            },
        )
    }

    #[test]
    fn fetched_profile_fills_the_field_and_applies() {
        let s = reduce(&ProfileState::default(), ProfileMsg::Begin);
        let s = fetched(&s, &sample(), Some("application/ld+json; charset=utf-8"));
        assert!(!s.loading);
        assert_eq!(s.text, sample());
        assert!(s.wording.is_some());
        let s = reduce(&ProfileState::default(), ProfileMsg::Begin);
        let s = fetched(&s, PRESETS[1].text, Some("text/turtle"));
        assert!(s.wording.is_some(), "{:?}", s.error);
        // GitHub's raw files and many static servers say text/plain.
        let s = reduce(&ProfileState::default(), ProfileMsg::Begin);
        let s = fetched(&s, &sample(), Some("text/plain"));
        assert!(s.wording.is_some());
        let s = reduce(&ProfileState::default(), ProfileMsg::Begin);
        let s = fetched(&s, &sample(), None);
        assert!(s.wording.is_some());
    }

    #[test]
    fn non_json_content_type_message() {
        assert_eq!(
            content_type_problem(Some("text/html; charset=utf-8")).as_deref(),
            Some("The server answered with text/html; only JSON-LD and Turtle profiles are read.")
        );
        assert!(content_type_problem(Some("application/rdf+xml")).is_some());
        for ok in [
            "application/ld+json",
            "application/json",
            "Application/JSON; charset=UTF-8",
            "text/turtle",
            "application/x-turtle",
            "text/plain",
            "application/octet-stream",
            "application/vnd.example+json",
        ] {
            assert_eq!(content_type_problem(Some(ok)), None, "{ok}");
        }
        assert_eq!(content_type_problem(None), None);
        // A fetched landing page is refused, and the applied profile stays.
        let good = applied(&sample());
        let s = reduce(&good, ProfileMsg::Begin);
        let s = fetched(&s, "<!doctype html>", Some("text/html"));
        assert!(s.error.as_deref().unwrap().contains("text/html"));
        assert_eq!(s.text, good.text);
        assert_eq!(s.wording, good.wording);
        assert!(!s.loading);
    }

    #[test]
    fn http_on_https_message() {
        assert_eq!(
            url_problem(
                "https://ds-labs-org.github.io/ds-prose-rs/",
                "http://example.org/p.jsonld"
            )
            .as_deref(),
            Some("The browser blocks http:// URLs from this https page.")
        );
        assert_eq!(
            url_problem("http://127.0.0.1:8080/", "http://example.org/p.jsonld"),
            None
        );
        assert_eq!(
            url_problem("https://e.org/", "https://example.org/p.jsonld"),
            None
        );
        assert_eq!(
            url_problem("https://e.org/", "blob:https://e.org/1234"),
            None
        );
        assert_eq!(
            url_problem("https://e.org/", "file:///etc/passwd").as_deref(),
            Some("Only http:// and https:// URLs are loaded.")
        );
        assert_eq!(
            url_problem("https://e.org/", "javascript:alert(1)").as_deref(),
            Some("Only http:// and https:// URLs are loaded.")
        );
    }

    #[test]
    fn stale_fetch_gen_ignored() {
        let s = reduce(&ProfileState::default(), ProfileMsg::Begin);
        let started = s.generation;
        let s = reduce(&s, ProfileMsg::Clear);
        let s = reduce(
            &s,
            ProfileMsg::Fetched {
                generation: started,
                text: sample(),
                content_type: Some("application/ld+json".into()),
            },
        );
        assert_eq!(s.text, "");
        assert_eq!(s.wording, None);
        // A preset chosen while a fetch runs wins over it too.
        let s = reduce(&ProfileState::default(), ProfileMsg::Begin);
        let started = s.generation;
        let s = reduce(&s, ProfileMsg::Use(PRESETS[2].text.into()));
        let s = reduce(
            &s,
            ProfileMsg::Fetched {
                generation: started,
                text: sample(),
                content_type: None,
            },
        );
        assert_eq!(s.text, PRESETS[2].text);
    }

    #[test]
    fn fetched_profile_reads_relative_iris_against_its_url() {
        let ttl = "@prefix odrl: <http://www.w3.org/ns/odrl/2/> .\n\
                   <#train> a odrl:Action .\n";
        let s = reduce(&ProfileState::default(), ProfileMsg::Begin);
        let s = reduce(
            &s,
            ProfileMsg::FetchedFrom {
                generation: s.generation,
                url: "https://e.org/p/profile.ttl".into(),
                text: ttl.into(),
                content_type: Some("text/turtle".into()),
            },
        );
        let w = s.wording.as_ref().expect("applied");
        assert!(w.0.lookup("https://e.org/p/profile.ttl#train").is_some());
        // Editing the fetched text keeps its URL; a preset or a file does not.
        let s = reduce(&s, ProfileMsg::Edit(format!("{ttl}\n")));
        let s = reduce(&s, ProfileMsg::Apply);
        assert!(s.wording.is_some(), "{:?}", s.error);
        let s = reduce(&s, ProfileMsg::Use(ttl.into()));
        assert!(s.wording.is_none());
        assert!(s.error.is_some());
    }

    #[test]
    fn url_is_kept_by_clear() {
        let s = reduce(
            &ProfileState::default(),
            ProfileMsg::Url("https://e.org/p.ttl".into()),
        );
        assert_eq!(s.url, "https://e.org/p.ttl");
        let s = reduce(&s, ProfileMsg::Clear);
        assert_eq!(s.url, "https://e.org/p.ttl");
    }

    #[test]
    fn declared_profile_is_the_first_profile_iri() {
        let policy = include_str!("../../prose-core/tests/profiles/research-policy.json");
        assert_eq!(
            declared_profile(policy).as_deref(),
            Some("https://example.org/odrl/profile/research")
        );
        let cases = [
            (
                r#"{"profile": ["https://a.org/p", "https://b.org/q"]}"#,
                Some("https://a.org/p"),
            ),
            (
                r#"{"profile": {"@id": "https://a.org/p"}}"#,
                Some("https://a.org/p"),
            ),
            (
                r#"{"odrl:profile": "https://a.org/p"}"#,
                Some("https://a.org/p"),
            ),
            (
                r#"{"@graph": [{"uid": "x"}, {"profile": "https://a.org/p"}]}"#,
                Some("https://a.org/p"),
            ),
            (
                r#"[{"profile": "https://a.org/p"}]"#,
                Some("https://a.org/p"),
            ),
            (r#"{"uid": "x"}"#, None),
            (r#"{"profile": 3}"#, None),
            ("{", None),
        ];
        for (json, want) in cases {
            assert_eq!(declared_profile(json).as_deref(), want, "{json}");
        }
    }
}
