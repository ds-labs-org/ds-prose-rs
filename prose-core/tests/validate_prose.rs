//! Strict validation, `prose.*` tier: the ds-prose hints. They stay
//! suggestions (info) unless the profile closes the left operand with
//! `prose:closed true`; then they bind. A ds-prose convention, not ODRL.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::{ProfileLimits, ProfileVocab, Wording};
use prose_core::validate::*;
use serde_json::{Value, json};

const RESEARCH: &str = "https://example.org/odrl/profile/research";
const EX: &str = "https://example.org/odrl/profile/research/";
const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

fn summary(r: &Report) -> Vec<(String, String, Severity)> {
    r.findings
        .iter()
        .map(|f| (f.rule.to_string(), target_str(&f.target), f.severity))
        .collect()
}

fn row(rule: &str, target: &str, severity: Severity) -> (String, String, Severity) {
    (rule.to_string(), target.to_string(), severity)
}

fn open() -> Wording {
    wording_of(&["research.jsonld"])
}

fn closed() -> Wording {
    wording_of(&["research-closed.jsonld"])
}

/// A Set declaring `profiles` with one permission constrained by
/// `constraint`.
fn declaring(profiles: Value, constraint: Value) -> String {
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld",
                     {"ex": EX, "u": "https://example.org/odrl/profile/units/"}],
        "@type": "Set", "uid": "urn:policy:p",
        "profile": profiles,
        "permission": [{"target": "urn:asset:1", "action": "use", "constraint": [constraint]}]
    })
    .to_string()
}

/// [`declaring`] the research profile.
fn with_constraint(constraint: Value) -> String {
    declaring(json!(RESEARCH), constraint)
}

/// [`declaring`] the research and units profiles.
fn with_units(constraint: Value) -> String {
    declaring(
        json!([RESEARCH, "https://example.org/odrl/profile/units"]),
        constraint,
    )
}

const C: &str = "policy[0].permission[0].constraint[0]";

fn at(field: &str) -> String {
    format!("{C}#{field}")
}

fn strict(text: &str, w: &Wording) -> Report {
    validate_json(text, w, &Options::strict())
}

#[test]
fn hostile_prose_and_heuristic_rules() {
    hostile_trigger_only_their_rule(&[
        "prose.operator-not-allowed",
        "prose.value-not-allowed",
        "prose.iri-expected",
        "prose.unit-not-allowed",
        "prose.datatype-lexical",
        "prose.datatype-conflict",
        "prose.ill-typed-literal",
        "heuristic.ordered-on-unordered",
    ]);
}

#[test]
fn open_operand_stays_info_in_strict() {
    // Hints are suggestions unless the profile closes the operand: strict
    // mode does not read them as more than that.
    let text = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": EX}],
        "@type": "Set", "uid": "urn:policy:p", "profile": RESEARCH,
        "permission": [{"target": "urn:asset:1", "action": "use", "constraint": [
            {"leftOperand": "ex:dataSubjectCount", "operator": "lteq", "rightOperand": "ten"},
            {"leftOperand": "ex:researchPurpose", "operator": "eq",
             "rightOperand": {"@id": "https://example.net/elsewhere"}},
            {"leftOperand": "ex:researchPurpose", "operator": "eq", "rightOperand": "ex:academic"}
        ]}]
    })
    .to_string();
    let r = strict(&text, &open());
    let p = "policy[0].permission[0]";
    assert_eq!(
        summary(&r),
        [
            row(
                "prose.operator-not-allowed",
                &format!("{p}.constraint[0]#operator"),
                Severity::Info
            ),
            row(
                "prose.datatype-lexical",
                &format!("{p}.constraint[0]#rightOperand[0]"),
                Severity::Info
            ),
            row(
                "prose.value-not-allowed",
                &format!("{p}.constraint[1]#rightOperand[0]"),
                Severity::Info
            ),
        ]
    );
    // The same messages as the advisory mode.
    assert_eq!(
        r.findings[0].message,
        "the profile suggests the operators gteq or gt for \"ex:dataSubjectCount\""
    );
    assert_eq!(r.verdict, Verdict::Conforms);
    assert!(!r.findings[0].expected.as_ref().unwrap().closed);
}

#[test]
fn closed_operator_error() {
    let text = with_constraint(json!({
        "leftOperand": "ex:dataSubjectCount", "operator": "lteq", "rightOperand": 50
    }));
    let r = strict(&text, &closed());
    assert_eq!(
        summary(&r),
        [row(
            "prose.operator-not-allowed",
            &at("operator"),
            Severity::Error
        )]
    );
    let f = &r.findings[0];
    assert_eq!(
        f.message,
        "the profile allows only the operators gteq or gt for \"ex:dataSubjectCount\""
    );
    let e = f.expected.as_ref().unwrap();
    assert!(e.closed);
    assert_eq!(e.operators, [format!("{ODRL}gteq"), format!("{ODRL}gt")]);
    assert_eq!(r.verdict, Verdict::Violations);
    // Advisory keeps its info, closed or not.
    let r = validate_json(&text, &closed(), &Options::advisory());
    assert_eq!(
        summary(&r),
        [row(
            "prose.operator-not-allowed",
            &at("operator"),
            Severity::Info
        )]
    );
    assert_eq!(
        r.findings[0].message,
        "the profile suggests the operators gteq or gt for \"ex:dataSubjectCount\""
    );
}

#[test]
fn value_tree_descendant_allowed() {
    let text = with_constraint(json!({
        "leftOperand": "ex:researchPurpose", "operator": "isAnyOf",
        "rightOperand": [{"@id": "ex:clinical"}, {"@id": "ex:academic"},
                         {"@id": "https://example.org/odrl/profile/research/commercialResearch"}]
    }));
    let r = strict(&text, &closed());
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

#[test]
fn closed_value_error() {
    let text = with_constraint(json!({
        "leftOperand": "ex:researchPurpose", "operator": "isAnyOf",
        "rightOperand": [{"@id": "ex:academic"}, {"@id": "https://example.net/elsewhere"}, "teaching", 7]
    }));
    let r = strict(&text, &closed());
    assert_eq!(
        summary(&r),
        [
            row(
                "prose.value-not-allowed",
                &at("rightOperand[1]"),
                Severity::Error
            ),
            row(
                "prose.value-not-allowed",
                &at("rightOperand[2]"),
                Severity::Error
            ),
            row(
                "prose.value-not-allowed",
                &at("rightOperand[3]"),
                Severity::Error
            ),
        ]
    );
    assert_eq!(
        r.findings[0].message,
        "\"https://example.net/elsewhere\" is not a value the profile allows for \"ex:researchPurpose\""
    );
    let e = r.findings[0].expected.as_ref().unwrap();
    assert!(e.closed);
    assert_eq!(
        e.values,
        [
            format!("{EX}academic"),
            format!("{EX}clinical"),
            format!("{EX}commercialResearch")
        ]
    );
}

#[test]
fn iri_expected_fix() {
    let text = with_constraint(json!({
        "leftOperand": "ex:researchPurpose", "operator": "eq", "rightOperand": "ex:academic"
    }));
    // On an open operand a string is a string.
    assert!(strict(&text, &open()).findings.is_empty());
    let r = strict(&text, &closed());
    assert_eq!(
        summary(&r),
        [row(
            "prose.iri-expected",
            &at("rightOperand[0]"),
            Severity::Error
        )]
    );
    let f = &r.findings[0];
    assert_eq!(
        f.message,
        "\"ex:academic\" names a value the profile allows for \"ex:researchPurpose\", but as a plain string; write it as an IRI ({\"@id\": \"ex:academic\"})"
    );
    let fix = &f.fixes[0];
    assert!(fix.safe);
    assert_eq!(fix.replacement, "ex:academic");
    let mut doc = read_model(&text).unwrap();
    for ev in &fix.events {
        doc.apply(ev, &EditRules::default()).unwrap();
    }
    assert_eq!(
        write_jsonld(&doc)["permission"][0]["constraint"][0]["rightOperand"],
        json!({"@id": "ex:academic"})
    );
    assert!(validate(&doc, &closed(), &Options::strict()).conforms());
}

fn units(closed: bool) -> Wording {
    Wording::new().with_profile(
        ProfileVocab::from_value(
            &json!({
                "@context": {"odrl": "http://www.w3.org/ns/odrl/2/",
                             "prose": "https://ds-labs-org.github.io/ds-prose-rs/profile#",
                             "owl": "http://www.w3.org/2002/07/owl#",
                             "ex": "https://example.org/odrl/profile/units/"},
                "@graph": [{"@id": "https://example.org/odrl/profile/units", "@type": "owl:Ontology"},
                           {"@id": "ex:storage", "@type": "odrl:LeftOperand",
                            "prose:unit": [{"@id": "ex:gigabyte"}], "prose:closed": closed}]
            }),
            &ProfileLimits::default(),
        )
        .unwrap(),
    )
    .with_profile(profile("research.jsonld"))
}

#[test]
fn unit_closed() {
    let bad = with_units(json!({
        "leftOperand": "u:storage", "operator": "lteq", "rightOperand": 5,
        "unit": "http://dbpedia.org/resource/Megabyte"
    }));
    let r = strict(&bad, &units(true));
    assert_eq!(
        summary(&r),
        [row("prose.unit-not-allowed", &at("unit"), Severity::Error)]
    );
    assert_eq!(
        r.findings[0].message,
        "the profile allows only the units gigabyte for \"u:storage\""
    );
    assert_eq!(
        r.findings[0].expected.as_ref().unwrap().units,
        ["https://example.org/odrl/profile/units/gigabyte"]
    );
    assert_eq!(
        summary(&strict(&bad, &units(false))),
        [row("prose.unit-not-allowed", &at("unit"), Severity::Info)]
    );
    let good = with_units(json!({
        "leftOperand": "u:storage", "operator": "lteq", "rightOperand": 5, "unit": "u:gigabyte"
    }));
    assert!(strict(&good, &units(true)).conforms());
    // No unit at all is not a wrong unit.
    let none = with_units(json!({
        "leftOperand": "u:storage", "operator": "lteq", "rightOperand": 5
    }));
    assert!(strict(&none, &units(true)).conforms());
}

#[test]
fn odrl_datatype_checked_without_profile() {
    // A typed literal outside its own datatype's lexical space, checked
    // with no profile at all.
    let text = std::fs::read_to_string(format!("{}/strict/datatype.json", tests_dir())).unwrap();
    let r = strict(&text, &Wording::new());
    let p = "policy[0].permission[0]";
    assert_eq!(
        summary(&r),
        [
            row("profile.not-loaded", "policy[0]", Severity::Error),
            row(
                "prose.ill-typed-literal",
                &format!("{p}.constraint[0]#rightOperand[0]"),
                Severity::Error
            ),
            row(
                "prose.ill-typed-literal",
                &format!("{p}.constraint[2]#rightOperand[0]"),
                Severity::Error
            ),
        ]
    );
    assert_eq!(
        r.findings[1].message,
        "\"ten\" is not an integer, so the literal typed xsd:integer is ill-typed"
    );
    // Advisory has never checked it.
    let r = validate_json(&text, &open(), &Options::advisory());
    assert!(r.findings.is_empty(), "{:?}", r.findings);
    // Datatypes it has no lexical check for are not checked.
    let text = with_constraint(json!({
        "leftOperand": "language", "operator": "eq",
        "rightOperand": {"@value": "anything", "@type": "xsd:string"}
    }));
    assert!(strict(&text, &open()).conforms());
}

#[test]
fn datatype_conflict() {
    let text = with_constraint(json!({
        "leftOperand": "ex:dataSubjectCount", "operator": "gteq",
        "rightOperand": {"@value": "50", "@type": "xsd:decimal"}
    }));
    let want = |s| [row("prose.datatype-conflict", &at("rightOperand[0]"), s)];
    assert_eq!(summary(&strict(&text, &open())), want(Severity::Warning));
    let r = strict(&text, &closed());
    assert_eq!(summary(&r), want(Severity::Error));
    assert_eq!(
        r.findings[0].message,
        format!(
            "\"50\" is typed <{XSD}decimal>, but the profile gives \"ex:dataSubjectCount\" the datatype <{XSD}integer>"
        )
    );
    assert_eq!(
        r.findings[0].expected.as_ref().unwrap().datatype.as_deref(),
        Some(&*format!("{XSD}integer"))
    );
    // The constraint's own odrl:dataType disagrees too.
    let text = with_constraint(json!({
        "leftOperand": "ex:dataSubjectCount", "operator": "gteq", "rightOperand": 50,
        "dataType": "xsd:string"
    }));
    let r = strict(&text, &closed());
    assert!(
        r.findings
            .iter()
            .any(|f| f.rule == RuleId::ProseDatatypeConflict
                && f.target == IssueTarget::Node(C.parse().unwrap())),
        "{:?}",
        r.findings
    );
}

#[test]
fn a_literal_ill_typed_for_the_hinted_datatype_is_reported_once() {
    let text = with_constraint(json!({
        "leftOperand": "ex:dataSubjectCount", "operator": "gteq",
        "rightOperand": {"@value": "ten", "@type": "xsd:integer"}
    }));
    assert_eq!(
        summary(&strict(&text, &closed())),
        [row(
            "prose.datatype-lexical",
            &at("rightOperand[0]"),
            Severity::Error
        )]
    );
}

#[test]
fn ordering_an_unordered_value_is_opt_in() {
    let text = with_constraint(json!({
        "leftOperand": "purpose", "operator": "lt",
        "rightOperand": {"@id": "https://example.net/purposes/research"}
    }));
    assert!(strict(&text, &open()).conforms());
    let o = Options::strict().level(RuleId::HeuristicOrderedOnUnordered, Level::Warning);
    let r = validate_json(&text, &open(), &o);
    assert_eq!(
        summary(&r),
        [row(
            "heuristic.ordered-on-unordered",
            &at("operator"),
            Severity::Warning
        )]
    );
    assert_eq!(
        r.findings[0].message,
        "\"lt\" orders values, but \"purpose\" is compared with an IRI or a boolean, which have no order"
    );
    let text =
        with_constraint(json!({"leftOperand": "count", "operator": "gteq", "rightOperand": true}));
    assert_eq!(validate_json(&text, &open(), &o).findings.len(), 1);
    let text =
        with_constraint(json!({"leftOperand": "count", "operator": "gteq", "rightOperand": 3}));
    assert!(validate_json(&text, &open(), &o).conforms());
}

#[test]
fn a_referenced_value_on_a_closed_operand_is_unchecked() {
    let text = with_constraint(json!({
        "leftOperand": "ex:researchPurpose", "operator": "eq",
        "rightOperandReference": "https://example.net/current-purpose"
    }));
    let r = strict(&text, &closed());
    assert_eq!(summary(&r), [row("input.unchecked", C, Severity::Warning)]);
    assert_eq!(
        r.findings[0].message,
        "the right operand is only referenced (rightOperandReference), so the values the profile allows for \"ex:researchPurpose\" could not be checked"
    );
    assert_eq!(r.verdict, Verdict::Incomplete);
    // On an open operand nothing binds, so nothing is left unchecked.
    assert!(strict(&text, &open()).conforms());
}
