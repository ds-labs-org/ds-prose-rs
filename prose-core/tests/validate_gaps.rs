//! Strict validation never passes on what it could not check: a declared
//! profile that is not loaded, a profile a loaded one builds on, or a node
//! the reader kept as written make the verdict incomplete, and the term
//! rules stay silent rather than call a missing profile's terms unknown.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::{ProfileLimits, ProfileVocab, Wording};
use prose_core::validate::*;
use serde_json::json;

const EXT: &str = "https://example.org/odrl/profile/research-ext";
const BASE: &str = "https://example.org/odrl/profile/base";

fn strict_text(name: &str) -> String {
    std::fs::read_to_string(format!("{}/strict/{name}", tests_dir())).unwrap()
}

fn summary(r: &Report) -> Vec<(String, String, Severity, bool)> {
    r.findings
        .iter()
        .map(|f| (f.rule.to_string(), target_str(&f.target), f.severity, f.gap))
        .collect()
}

fn row(
    rule: &str,
    target: &str,
    severity: Severity,
    gap: bool,
) -> (String, String, Severity, bool) {
    (rule.to_string(), target.to_string(), severity, gap)
}

#[test]
fn hostile_gap_and_input_rules() {
    hostile_trigger_only_their_rule(&[
        "profile.not-loaded",
        "profile.import-not-loaded",
        "input.unreadable",
        "input.unchecked",
        "input.reader-note",
    ]);
}

#[test]
fn not_loaded_is_incomplete_and_suppresses_term_rules() {
    let r = validate_json(
        &strict_text("not-loaded.json"),
        &wording_of(&["research.jsonld"]),
        &Options::strict(),
    );
    // foo:bar may be the missing profile's: not called unknown.
    assert_eq!(
        summary(&r),
        [row(
            "profile.not-loaded",
            "policy[0]",
            Severity::Error,
            true
        )]
    );
    assert_eq!(r.verdict, Verdict::Incomplete);
    assert!(!r.conforms());
    assert_eq!(r.gaps, r.findings);
    let f = &r.findings[0];
    assert_eq!(
        f.profile.as_deref(),
        Some("https://example.org/odrl/profile/missing")
    );
}

#[test]
fn errors_and_gaps_is_violations_with_gaps_listed() {
    // Two policies: the first declares a missing profile, the second does
    // not, so its unknown term is an error all the same.
    let text = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"foo": "https://example.net/foo/"}],
        "@graph": [
            {"@type": "Set", "uid": "urn:policy:a",
             "profile": "https://example.org/odrl/profile/missing",
             "permission": [{"target": "urn:asset:1", "action": "foo:bar"}]},
            {"@type": "Set", "uid": "urn:policy:b",
             "permission": [{"target": "urn:asset:1", "action": "foo:bar"}]}
        ]
    })
    .to_string();
    let r = validate_json(&text, &Wording::new(), &Options::strict());
    assert_eq!(
        summary(&r),
        [
            row("profile.not-loaded", "policy[0]", Severity::Error, true),
            row(
                "profile.unknown-term",
                "policy[1].permission[0].action[0]#name",
                Severity::Error,
                false
            ),
        ]
    );
    assert_eq!(r.verdict, Verdict::Violations);
    assert_eq!(r.gaps.len(), 1);
    assert_eq!(r.gaps[0].rule, RuleId::ProfileNotLoaded);
}

fn ext_policy(extra_action: &str) -> String {
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld",
                     {"ext": "https://example.org/odrl/profile/research-ext/",
                      "foo": "https://example.net/foo/"}],
        "@type": "Set", "uid": "urn:policy:ext", "profile": EXT,
        "permission": [{"target": "urn:asset:1", "action": "ext:summarise"},
                       {"target": "urn:asset:1", "action": extra_action}]
    })
    .to_string()
}

#[test]
fn import_not_loaded_incomplete() {
    let w = wording_of(&["research-imports.jsonld", "research.jsonld"]);
    let r = validate_json(&ext_policy("foo:bar"), &w, &Options::strict());
    // research is imported and loaded, ODRL needs no loading; base is the
    // gap, and while it is missing foo:bar is not called unknown.
    assert_eq!(
        summary(&r),
        [row(
            "profile.import-not-loaded",
            "doc",
            Severity::Warning,
            true
        )]
    );
    assert_eq!(
        r.findings[0].message,
        format!("the loaded profile <{EXT}> builds on <{BASE}>, which is not loaded")
    );
    assert_eq!(r.findings[0].profile.as_deref(), Some(BASE));
    assert_eq!(r.verdict, Verdict::Incomplete);
    // Advisory has never said this.
    assert!(
        validate_json(&ext_policy("use"), &w, &Options::advisory())
            .findings
            .is_empty()
    );
    // Loading base closes the gap; foo:bar is then unknown.
    let base = ProfileVocab::from_value(
        &json!({
            "@context": {"odrl": "http://www.w3.org/ns/odrl/2/",
                         "owl": "http://www.w3.org/2002/07/owl#"},
            "@graph": [{"@id": BASE, "@type": "owl:Ontology"},
                       {"@id": "https://example.org/odrl/profile/base/x", "@type": "odrl:Action"}]
        }),
        &ProfileLimits::default(),
    )
    .unwrap();
    let w = w.with_profile(base);
    assert!(validate_json(&ext_policy("use"), &w, &Options::strict()).conforms());
    let r = validate_json(&ext_policy("foo:bar"), &w, &Options::strict());
    assert_eq!(
        summary(&r),
        [row(
            "profile.unknown-term",
            "policy[0].permission[1].action[0]#name",
            Severity::Error,
            false
        )]
    );
}

#[test]
fn locked_opaque_unchecked_incomplete() {
    let text = strict_text("locked-opaque.json");
    let w = wording_of(&["research.jsonld"]);
    let r = validate_json(&text, &w, &Options::strict());
    assert_eq!(
        summary(&r),
        [
            row("input.reader-note", "doc", Severity::Info, false),
            row("input.reader-note", "doc", Severity::Info, false),
            row(
                "input.unchecked",
                "policy[0].permission[0].constraint[0]",
                Severity::Warning,
                true
            ),
            row(
                "input.unchecked",
                "policy[0].permission[0].constraint[1]",
                Severity::Warning,
                true
            ),
            row(
                "input.unchecked",
                "policy[0].permission[1]",
                Severity::Warning,
                true
            ),
            row(
                "input.unchecked",
                "policy[0].permission[2].action[0]",
                Severity::Warning,
                true
            ),
        ]
    );
    let messages: Vec<&str> = r.findings.iter().map(|f| f.message.as_str()).collect();
    assert_eq!(
        messages,
        [
            "a constraint has several logical operators",
            "skipped an unreadable action",
            "this constraint could not be checked: several logical operators",
            "the constraint <urn:constraint:shared> is only referenced, so it could not be checked",
            "the rule <urn:rule:shared> is only referenced, so it could not be checked",
            "this action could not be checked: not an action",
        ]
    );
    assert_eq!(r.verdict, Verdict::Incomplete);
    assert_eq!(r.gaps.len(), 4);
    // Advisory says nothing about any of it, as before.
    assert!(
        validate_json(&text, &w, &Options::advisory())
            .findings
            .is_empty()
    );
}

#[test]
fn locked_policy_and_entity_are_unchecked() {
    let text = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@graph": [
            "urn:policy:elsewhere",
            {"@type": "Set", "uid": "urn:policy:e",
             "permission": [{"action": "use", "target": {"refinement": []}}]}
        ]
    })
    .to_string();
    let doc = read_model(&text).unwrap();
    let r = validate(&doc, &Wording::new(), &Options::strict());
    let unchecked: Vec<String> = r
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::InputUnchecked)
        .map(|f| target_str(&f.target))
        .collect();
    assert_eq!(
        unchecked,
        ["policy[0]", "policy[1].permission[0].target[0]"]
    );
    assert_eq!(r.verdict, Verdict::Incomplete);
}

#[test]
fn fail_closed_makes_unchecked_error() {
    let text = strict_text("locked-opaque.json");
    let w = wording_of(&["research.jsonld"]);
    let r = validate_json(&text, &w, &Options::strict().fail_closed(true));
    let unchecked: Vec<&Finding> = r
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::InputUnchecked)
        .collect();
    assert_eq!(unchecked.len(), 4);
    assert!(
        unchecked
            .iter()
            .all(|f| f.severity == Severity::Error && !f.gap)
    );
    assert!(r.gaps.is_empty());
    assert_eq!(r.verdict, Verdict::Violations);
}

#[test]
fn warnings_and_notes_alone_conform() {
    let text = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set", "uid": "urn:policy:w", "comment": "kept as written",
        "permission": [{"target": "urn:asset:1", "action": "commercialize"}]
    })
    .to_string();
    let r = validate_json(&text, &Wording::new(), &Options::strict());
    assert_eq!(
        summary(&r),
        [
            row("input.reader-note", "doc", Severity::Info, false),
            row(
                "profile.deprecated-term",
                "policy[0].permission[0].action[0]#name",
                Severity::Warning,
                false
            ),
        ]
    );
    assert_eq!(r.verdict, Verdict::Conforms);
}

#[test]
fn many_missing_profiles_are_counted_once() {
    let ids: Vec<String> = (0..25)
        .map(|i| format!("https://example.org/missing/{i}"))
        .collect();
    let text = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set", "uid": "urn:policy:m", "profile": ids,
        "permission": [{"target": "urn:asset:1", "action": "use"}]
    })
    .to_string();
    let r = validate_json(&text, &Wording::new(), &Options::strict());
    assert_eq!(r.findings.len(), 21);
    assert!(
        r.findings
            .iter()
            .all(|f| f.rule == RuleId::ProfileNotLoaded && f.gap)
    );
    assert_eq!(
        r.findings[20].message,
        "the policy declares 5 more profiles that are not loaded"
    );
    assert_eq!(r.verdict, Verdict::Incomplete);
}
