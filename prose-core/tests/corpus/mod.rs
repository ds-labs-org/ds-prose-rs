//! The policies and wordings the validation tests run over, and the golden
//! file rule they share.
#![allow(dead_code)]
use prose_core::edit::*;
use prose_core::profile::{ProfileLimits, ProfileVocab, Wording};
use serde_json::Value;

pub fn tests_dir() -> String {
    format!("{}/tests", env!("CARGO_MANIFEST_DIR"))
}

pub fn profile_text(name: &str) -> String {
    std::fs::read_to_string(format!("{}/profiles/{name}", tests_dir()))
        .unwrap_or_else(|_| panic!("no profile {name}"))
}

pub fn profile(name: &str) -> ProfileVocab {
    ProfileVocab::parse(&profile_text(name)).unwrap()
}

/// A wording with the named profile fixtures loaded, in order.
pub fn wording_of(names: &[&str]) -> Wording {
    names
        .iter()
        .fold(Wording::new(), |w, n| w.with_profile(profile(n)))
}

/// The wordings every policy is checked under.
pub fn wordings() -> Vec<(&'static str, Wording)> {
    vec![
        ("none", Wording::new()),
        ("research", wording_of(&["research.jsonld"])),
        ("research-closed", wording_of(&["research-closed.jsonld"])),
        ("odrl22-excerpt", wording_of(&["odrl22-excerpt.json"])),
        (
            "research+other",
            wording_of(&["research.jsonld", "other.jsonld"]),
        ),
        (
            "research-imports+research",
            wording_of(&["research-imports.jsonld", "research.jsonld"]),
        ),
    ]
}

fn json_files(dir: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort();
    names
}

/// A hostile case: one policy that triggers exactly one rule, with the
/// profiles and options it is checked under.
pub struct Hostile {
    pub rule: String,
    pub profiles: Vec<ProfileVocab>,
    pub options: Value,
    /// The policy as text, as a host would hand it over.
    pub policy: String,
}

pub fn hostile_cases() -> Vec<Hostile> {
    let dir = format!("{}/strict/hostile", tests_dir());
    json_files(&dir)
        .into_iter()
        .map(|name| {
            let v: Value =
                serde_json::from_str(&std::fs::read_to_string(format!("{dir}/{name}")).unwrap())
                    .unwrap();
            let profiles = v["profiles"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| match p {
                    Value::String(n) => profile(n),
                    inline => ProfileVocab::from_value(inline, &ProfileLimits::default()).unwrap(),
                })
                .collect();
            Hostile {
                rule: name.trim_end_matches(".json").to_string(),
                profiles,
                options: v["options"].clone(),
                policy: v["policy"].to_string(),
            }
        })
        .collect()
}

/// (name, text) of every policy fixture: the v0.1 fixtures, the profile
/// policies, the strict fixtures and the policies of the hostile cases.
pub fn policy_corpus() -> Vec<(String, String)> {
    let d = tests_dir();
    let mut out = Vec::new();
    for name in json_files(&format!("{d}/fixtures")) {
        let text = std::fs::read_to_string(format!("{d}/fixtures/{name}")).unwrap();
        out.push((format!("v01-{}", name.trim_end_matches(".json")), text));
    }
    for name in ["research-policy.json", "hostile/collision-policy.json"] {
        let text = std::fs::read_to_string(format!("{d}/profiles/{name}")).unwrap();
        let stem = name.trim_end_matches(".json").replace('/', "-");
        out.push((format!("profile-{stem}"), text));
    }
    for name in json_files(&format!("{d}/strict")) {
        let text = std::fs::read_to_string(format!("{d}/strict/{name}")).unwrap();
        out.push((format!("strict-{}", name.trim_end_matches(".json")), text));
    }
    for h in hostile_cases() {
        out.push((format!("hostile-{}", h.rule), h.policy));
    }
    out
}

pub fn target_str(t: &IssueTarget) -> String {
    match t {
        IssueTarget::Doc => "doc".to_string(),
        IssueTarget::Node(n) => n.to_string(),
        IssueTarget::Slot(s) => s.to_string(),
        IssueTarget::List(l) => l.to_string(),
        other => format!("{other:?}"),
    }
}

pub fn issue_line(i: &Issue) -> String {
    format!("{:?} {} {}", i.severity, target_str(&i.target), i.message)
}

/// Compares `got` with the golden at `tests/golden_validate/{rel}`, writing
/// it first when `PROSE_BLESS_GOLDEN` is set and the file is missing.
pub fn compare_golden(rel: &str, got: &str) {
    let path = format!("{}/golden_validate/{rel}", tests_dir());
    if std::env::var_os("PROSE_BLESS_GOLDEN").is_some() && !std::path::Path::new(&path).exists() {
        if let Some(parent) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, got).unwrap();
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing golden {path}"));
    assert_eq!(got, want, "golden mismatch for {rel}");
}
