//! Strict `prose.*` rules do not guess. Where the vocabulary is incomplete
//! or the policy's `@context` was not read, a closed operand's hints are not
//! a reading the validator can confirm, so the rules that depend on them stay
//! silent and the verdict is incomplete, as the `profile.*` term rules do.
//!
//! Per rule, under a declared profile that is not loaded (a gap):
//! - `prose.value-not-allowed` is unsound: the missing profile may add a
//!   value narrower (`skos:broader`) than an allowed one, so it is silent.
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
