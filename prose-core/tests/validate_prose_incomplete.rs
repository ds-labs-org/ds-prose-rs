//! Strict `prose.*` rules do not guess. Where the vocabulary is incomplete
//! or the policy's `@context` was not read, a closed operand's hints are not
//! a reading the validator can confirm, so the rules that depend on them stay
//! silent and the verdict is incomplete, as the `profile.*` term rules do.
//!
//! Per rule, under a declared profile that is not loaded (a gap):
//! - `prose.value-not-allowed` is unsound: the missing profile may add a
//!   value narrower (`skos:broader`) than an allowed one, so it is silent
//!   for an IRI value. A literal is never such a value, so it still fires.
//! - `prose.operator-not-allowed`, `prose.unit-not-allowed`,
//!   `prose.datatype-lexical`, `prose.datatype-conflict` and
//!   `prose.iri-expected` read only the loaded term's own hints and the
//!   written value, which a missing profile cannot change, so they hold.
//!
//! Under an unread `@context` every one of them is silent: it may rebind the
//! prefix, so the left operand may not be the profile's term at all.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::Severity;
use prose_core::profile::Wording;
use prose_core::validate::*;
use serde_json::{Value, json};

const RESEARCH: &str = "https://example.org/odrl/profile/research";
const EXT: &str = "https://example.org/odrl/profile/ext";
const EX: &str = "https://example.org/odrl/profile/research/";
const B: &str = "https://example.org/odrl/profile/ext/";
const REMOTE: &str = "https://example.net/contexts/dataspace.jsonld";

fn closed() -> Wording {
    wording_of(&["research-closed.jsonld"])
}

fn rules(r: &Report) -> Vec<String> {
    r.findings.iter().map(|f| f.rule.to_string()).collect()
}

fn prose_errors(r: &Report) -> Vec<String> {
    r.findings
        .iter()
        .filter(|f| f.rule.to_string().starts_with("prose.") && f.severity == Severity::Error)
        .map(|f| f.rule.to_string())
        .collect()
}

fn policy(context: Value, profiles: Value, constraint: Value) -> String {
    json!({
        "@context": context,
        "@type": "Set", "uid": "urn:policy:p",
        "profile": profiles,
        "permission": [{"target": "urn:asset:1", "action": "use", "constraint": [constraint]}]
    })
    .to_string()
}

fn local(profiles: Value, constraint: Value) -> String {
    policy(
        json!(["http://www.w3.org/ns/odrl.jsonld", {"ex": EX, "b": B}]),
        profiles,
        constraint,
    )
}

fn run(text: &str) -> Report {
    validate_json(text, &closed(), &Options::strict())
}

#[test]
fn an_unread_context_silences_every_closed_hint() {
    let bad_operand = json!({"leftOperand": "ex:researchPurpose", "operator": "lt",
                             "rightOperand": {"@id": "ex:whatever"}});
    let bad_count = json!({"leftOperand": "ex:dataSubjectCount", "operator": "lteq",
                           "rightOperand": "ten"});
    for c in [bad_operand, bad_count] {
        let text = policy(
            json!(["http://www.w3.org/ns/odrl.jsonld", REMOTE]),
            json!(RESEARCH),
            c.clone(),
        );
        let r = run(&text);
        assert_eq!(
            prose_errors(&r),
            Vec::<String>::new(),
            "{c}: {:?}",
            rules(&r)
        );
        assert_eq!(r.verdict, Verdict::Incomplete, "{c}: {:?}", rules(&r));
    }
}

#[test]
fn the_same_policy_with_a_known_context_is_still_an_error() {
    // The control: only the unread context changes the verdict.
    let text = local(
        json!(RESEARCH),
        json!({"leftOperand": "ex:researchPurpose", "operator": "lt",
               "rightOperand": {"@id": "ex:whatever"}}),
    );
    let r = run(&text);
    assert_eq!(
        prose_errors(&r),
        ["prose.operator-not-allowed", "prose.value-not-allowed"]
    );
    assert_eq!(r.verdict, Verdict::Violations);
}

#[test]
fn a_value_a_missing_profile_may_narrow_is_not_an_error() {
    // b:clinicalTrial is skos:broader ex:academic in the profile that is not
    // loaded: it is an allowed value there.
    let text = local(
        json!([RESEARCH, EXT]),
        json!({"leftOperand": "ex:researchPurpose", "operator": "eq",
               "rightOperand": {"@id": "b:clinicalTrial"}}),
    );
    let r = run(&text);
    assert!(rules(&r).contains(&"profile.not-loaded".to_string()));
    assert_eq!(prose_errors(&r), Vec::<String>::new(), "{:?}", rules(&r));
    assert_eq!(r.verdict, Verdict::Incomplete);
}

#[test]
fn rules_a_missing_profile_cannot_change_still_fire() {
    // The operator, datatype and unit come from the loaded term itself.
    let text = local(
        json!([RESEARCH, EXT]),
        json!({"leftOperand": "ex:dataSubjectCount", "operator": "lteq", "rightOperand": "ten"}),
    );
    let r = run(&text);
    let got = prose_errors(&r);
    assert!(
        got.contains(&"prose.operator-not-allowed".to_string()),
        "{got:?}"
    );
    assert!(
        got.contains(&"prose.datatype-lexical".to_string()),
        "{got:?}"
    );
    assert_eq!(r.verdict, Verdict::Violations);
}

#[test]
fn advisory_is_unchanged_by_an_unread_context() {
    let text = policy(
        json!(["http://www.w3.org/ns/odrl.jsonld", REMOTE]),
        json!(RESEARCH),
        json!({"leftOperand": "ex:researchPurpose", "operator": "lt",
               "rightOperand": {"@id": "ex:whatever"}}),
    );
    let r = validate_json(&text, &closed(), &Options::advisory());
    assert!(
        rules(&r).iter().any(|x| x == "prose.operator-not-allowed"),
        "{:?}",
        rules(&r)
    );
}

fn findings_of(r: &Report, rule: &str) -> Vec<Severity> {
    r.findings
        .iter()
        .filter(|f| f.rule.to_string() == rule)
        .map(|f| f.severity)
        .collect()
}

#[test]
fn a_string_naming_an_allowed_value_is_still_iri_expected() {
    // Whatever the missing profile adds, an allowed value written as a
    // plain string is not that IRI.
    let text = local(
        json!([RESEARCH, EXT]),
        json!({"leftOperand": "ex:researchPurpose", "operator": "eq",
               "rightOperand": "ex:academic"}),
    );
    let r = run(&text);
    assert_eq!(prose_errors(&r), ["prose.iri-expected"], "{:?}", rules(&r));
    assert_eq!(r.verdict, Verdict::Violations);
}

#[test]
fn an_open_operand_keeps_its_suggestions_under_an_unread_context() {
    // Only a closed operand's hints wait for the context; an open one's
    // are suggestions, never errors, and are still made.
    let text = policy(
        json!(["http://www.w3.org/ns/odrl.jsonld", REMOTE]),
        json!(RESEARCH),
        json!({"leftOperand": "ex:retentionPeriod", "operator": "gt",
               "rightOperand": {"@value": "P1Y", "@type": "xsd:duration"}}),
    );
    let r = run(&text);
    assert_eq!(
        findings_of(&r, "prose.operator-not-allowed"),
        [Severity::Info],
        "{:?}",
        rules(&r)
    );
    assert_eq!(r.verdict, Verdict::Incomplete);
}

#[test]
fn an_open_operand_keeps_its_value_suggestion_under_a_missing_profile() {
    // research.jsonld leaves ex:researchPurpose open.
    let text = local(
        json!([RESEARCH, EXT]),
        json!({"leftOperand": "ex:researchPurpose", "operator": "eq",
               "rightOperand": {"@id": "ex:whatever"}}),
    );
    let r = validate_json(&text, &wording_of(&["research.jsonld"]), &Options::strict());
    assert_eq!(
        findings_of(&r, "prose.value-not-allowed"),
        [Severity::Info],
        "{:?}",
        rules(&r)
    );
}

#[test]
fn advisory_levels_set_by_hand_are_unchanged_by_a_missing_profile() {
    // The wait is strict mode's; advisory reports what it was asked to.
    let text = local(
        json!([RESEARCH, EXT]),
        json!({"leftOperand": "ex:researchPurpose", "operator": "eq",
               "rightOperand": {"@id": "b:clinicalTrial"}}),
    );
    let o = Options::advisory().level(RuleId::ProseValueNotAllowed, Level::Error);
    let r = validate_json(&text, &closed(), &o);
    assert_eq!(
        findings_of(&r, "prose.value-not-allowed"),
        [Severity::Error],
        "{:?}",
        rules(&r)
    );
}

#[test]
fn a_literal_no_profile_can_narrow_is_still_an_error() {
    // A missing profile can add only narrower IRIs (prose:rightOperand and
    // skos:broader name IRIs), never a literal, so a literal on a closed
    // IRI-valued operand is an error whatever it holds.
    for v in [
        json!("ten"),
        json!({"@value": "10", "@type": "xsd:integer"}),
    ] {
        let text = local(
            json!([RESEARCH, EXT]),
            json!({"leftOperand": "ex:researchPurpose", "operator": "eq", "rightOperand": v}),
        );
        let r = run(&text);
        assert_eq!(
            prose_errors(&r),
            ["prose.value-not-allowed"],
            "{v}: {:?}",
            rules(&r)
        );
        assert_eq!(r.verdict, Verdict::Violations, "{v}");
    }
}
