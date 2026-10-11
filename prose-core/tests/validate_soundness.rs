//! Strict validation claims a violation only where the vocabulary it checks
//! against is the one the policy is read with. A `@context` the reader does
//! not apply (a remote one, an `@import`, one nested in a node) can bind any
//! prefix or term, so the term rules stay silent and the verdict is
//! incomplete. A term two loaded profiles declare is read with both, and a
//! namespace a missing profile may share is not called closed.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::{ProfileLimits, ProfileVocab, Wording};
use prose_core::validate::*;
use serde_json::{Value, json};

const RESEARCH: &str = "https://example.org/odrl/profile/research";
const REMOTE: &str = "https://example.net/contexts/dataspace.jsonld";

/// (rule, target, severity, gap) of every finding, in order.
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

/// A Set declaring the research profile, with `context` and two
/// permissions: `ex:train`, whose prefix only the profile binds, and the
/// bare `summarise`, which no loaded vocabulary defines.
fn policy_with_context(context: Value) -> String {
    json!({
        "@context": context,
        "@type": "Set",
        "uid": "urn:policy:context",
        "profile": RESEARCH,
        "permission": [
            {"target": "urn:asset:1", "action": "ex:train"},
            {"target": "urn:asset:1", "action": "summarise"}
        ]
    })
    .to_string()
}

#[test]
fn a_known_context_keeps_the_term_rules() {
    // The control: the ODRL context is known, so both terms are errors.
    let r = validate_json(
        &policy_with_context(json!("http://www.w3.org/ns/odrl.jsonld")),
        &wording_of(&["research.jsonld"]),
        &Options::strict(),
    );
    assert_eq!(
        summary(&r),
        [
            row(
                "profile.unbound-prefix",
                "policy[0].permission[0].action[0]#name",
                Severity::Error,
                false
            ),
            row(
                "profile.unknown-term",
                "policy[0].permission[1].action[0]#name",
                Severity::Error,
                false
            ),
        ]
    );
    assert_eq!(r.verdict, Verdict::Violations);
}

#[test]
fn a_remote_context_is_a_gap_not_an_error() {
    // The remote context may bind "ex" and define "summarise": neither can
    // be called a violation without reading it.
    for context in [
        json!(REMOTE),
        json!([REMOTE, "http://www.w3.org/ns/odrl.jsonld"]),
        json!(["http://www.w3.org/ns/odrl.jsonld", {"@import": REMOTE}]),
    ] {
        let r = validate_json(
            &policy_with_context(context.clone()),
            &wording_of(&["research.jsonld"]),
            &Options::strict(),
        );
        assert_eq!(
            summary(&r),
            [row("input.unchecked", "doc", Severity::Warning, true)],
            "{context}"
        );
        assert_eq!(r.verdict, Verdict::Incomplete, "{context}");
        let f = &r.findings[0];
        assert!(f.message.contains(REMOTE), "{}", f.message);
        assert_eq!(f.term.as_ref().map(|t| t.written.as_str()), Some(REMOTE));
        assert_eq!(r.gaps.len(), 1);
    }
}

#[test]
fn a_remote_context_fails_closed_when_asked() {
    let r = validate_json(
        &policy_with_context(json!(REMOTE)),
        &wording_of(&["research.jsonld"]),
        &Options::strict().fail_closed(true),
    );
    assert_eq!(
        summary(&r),
        [row("input.unchecked", "doc", Severity::Error, false)]
    );
    assert_eq!(r.verdict, Verdict::Violations);
}

#[test]
fn a_nested_context_is_a_gap_not_an_error() {
    // A context on the rule binds "ex" for that rule; the reader applies
    // only the policy's and the document's.
    let text = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set",
        "uid": "urn:policy:nested",
        "profile": RESEARCH,
        "permission": [{
            "@context": {"ex": "https://example.org/odrl/profile/research/"},
            "target": "urn:asset:1",
            "action": "ex:train"
        }]
    })
    .to_string();
    let r = validate_json(&text, &wording_of(&["research.jsonld"]), &Options::strict());
    let rules: Vec<String> = r.findings.iter().map(|f| f.rule.to_string()).collect();
    assert!(
        !rules.iter().any(|r| r == "profile.unbound-prefix"),
        "{rules:?}"
    );
    assert!(rules.iter().any(|r| r == "input.unchecked"), "{rules:?}");
    assert_eq!(r.verdict, Verdict::Incomplete);
}

#[test]
fn context_gaps_say_nothing_in_advisory() {
    let w = wording_of(&["research.jsonld"]);
    let text = policy_with_context(json!(REMOTE));
    assert!(
        validate_json(&text, &w, &Options::advisory())
            .findings
            .is_empty()
    );
    let doc = read_model(&text).unwrap();
    assert!(check_profile(&doc, &w).is_empty());
}

/// A small profile `iri` that declares `term` as `kind`.
fn profile_declaring(iri: &str, term: &str, kind: &str) -> ProfileVocab {
    let v = json!({
        "@context": {"odrl": "http://www.w3.org/ns/odrl/2/",
                     "rdfs": "http://www.w3.org/2000/01/rdf-schema#"},
        "@graph": [
            {"@id": iri, "@type": "http://www.w3.org/2002/07/owl#Ontology"},
            {"@id": term, "@type": kind, "rdfs:isDefinedBy": {"@id": iri}}
        ]
    });
    ProfileVocab::from_value(&v, &ProfileLimits::default()).unwrap()
}

const SHARED: &str = "https://example.org/shared/terms/harvest";

fn shared_policy(declared: &[&str], at: &str) -> String {
    let rule = match at {
        "action" => json!({"target": "urn:asset:1", "action": {"@id": SHARED}}),
        _ => json!({"target": "urn:asset:1", "action": "use",
                    "constraint": [{"leftOperand": {"@id": SHARED},
                                    "operator": "eq", "rightOperand": "x"}]}),
    };
    json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set",
        "uid": "urn:policy:shared",
        "profile": declared,
        "permission": [rule]
    })
    .to_string()
}

#[test]
fn a_term_two_loaded_profiles_declare_needs_only_one_declared() {
    let (a, b) = ("https://example.org/a", "https://example.org/b");
    let w = Wording::new()
        .with_profile(profile_declaring(a, SHARED, "odrl:Action"))
        .with_profile(profile_declaring(b, SHARED, "odrl:Action"));
    for declared in [&[a][..], &[b][..], &[a, b][..]] {
        let r = validate_json(&shared_policy(declared, "action"), &w, &Options::strict());
        assert!(r.findings.is_empty(), "{declared:?}: {:?}", summary(&r));
        assert_eq!(r.verdict, Verdict::Conforms);
    }
    // Declaring neither is still an error, naming the first.
    let r = validate_json(&shared_policy(&[], "action"), &w, &Options::strict());
    assert_eq!(
        summary(&r),
        [row(
            "profile.not-declared",
            "policy[0].permission[0].action[0]#name",
            Severity::Error,
            false
        )]
    );
    assert_eq!(r.findings[0].profile.as_deref(), Some(a));
}

#[test]
fn a_term_is_every_kind_its_loaded_profiles_give_it() {
    // a reads it as a left operand, b as an action: V has it as both.
    let (a, b) = ("https://example.org/a", "https://example.org/b");
    let w = Wording::new()
        .with_profile(profile_declaring(a, SHARED, "odrl:LeftOperand"))
        .with_profile(profile_declaring(b, SHARED, "odrl:Action"));
    for at in ["action", "leftOperand"] {
        let r = validate_json(&shared_policy(&[a, b], at), &w, &Options::strict());
        assert!(r.findings.is_empty(), "{at}: {:?}", summary(&r));
    }
    // Neither gives it the operator kind.
    let w = Wording::new().with_profile(profile_declaring(a, SHARED, "odrl:LeftOperand"));
    let r = validate_json(&shared_policy(&[a], "action"), &w, &Options::strict());
    assert_eq!(r.findings.len(), 1);
    assert_eq!(r.findings[0].rule, RuleId::ProfileWrongKind);
}

#[test]
fn a_namespace_is_not_closed_while_a_declared_profile_is_missing() {
    // The missing profile may declare ex:distil in the research namespace.
    let text = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld",
                     {"ex": "https://example.org/odrl/profile/research/"}],
        "@type": "Set",
        "uid": "urn:policy:ns",
        "profile": [RESEARCH, "https://example.org/odrl/profile/research-v2"],
        "permission": [{"target": "urn:asset:1", "action": "ex:distil"}]
    })
    .to_string();
    let w = wording_of(&["research.jsonld"]);
    let r = validate_json(&text, &w, &Options::strict());
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
    // Advisory keeps its warning, as check_profile always has.
    let a = validate_json(&text, &w, &Options::advisory());
    let rules: Vec<String> = a.findings.iter().map(|f| f.rule.to_string()).collect();
    assert_eq!(
        rules,
        ["profile.not-loaded", "profile.undeclared-in-namespace"]
    );
}

#[test]
fn a_level_set_in_advisory_switches_the_rule_on() {
    let w = wording_of(&["research.jsonld"]);
    let text = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set",
        "uid": "urn:policy:levels",
        "permission": [{
            "target": "urn:asset:1",
            "action": "summarise",
            "constraint": [{"leftOperand": "purpose", "operator": "lteq",
                            "rightOperand": {"@id": "http://example.com/p"}}]
        }]
    })
    .to_string();
    assert!(
        validate_json(&text, &w, &Options::advisory())
            .findings
            .is_empty()
    );
    let o = Options::advisory()
        .level(RuleId::ProfileUnknownTerm, Level::Warning)
        .level(RuleId::HeuristicOrderedOnUnordered, Level::Info);
    let r = validate_json(&text, &w, &o);
    assert_eq!(
        summary(&r),
        [
            row(
                "profile.unknown-term",
                "policy[0].permission[0].action[0]#name",
                Severity::Warning,
                false
            ),
            row(
                "heuristic.ordered-on-unordered",
                "policy[0].permission[0].constraint[0]#operator",
                Severity::Info,
                false
            ),
        ]
    );
    // Also with no profile loaded.
    let r = validate_json(&text, &Wording::new(), &o);
    assert_eq!(r.findings.len(), 2, "{:?}", summary(&r));
}
