//! The edit layer with a loaded profile: slots show the profile's labels
//! while their raw text stays as written, a right-operand slot finds its
//! left operand, and the written JSON-LD does not depend on the profile.
mod common;
use common::*;
use prose_core::edit::*;
use prose_core::profile::{ProfileVocab, Wording};
use serde_json::json;

fn profile_text(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/profiles/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn research() -> Wording {
    Wording::new().with_profile(ProfileVocab::parse(&profile_text("research.jsonld")).unwrap())
}

fn research_doc() -> EditDoc {
    read_model(&profile_text("research-policy.json")).unwrap()
}

/// Every slot and choice in `p`, as (path, raw, display), in reading order.
fn slots(p: &PolicyProse) -> Vec<(String, String, String)> {
    fn sentence(s: &Sentence, out: &mut Vec<(String, String, String)>) {
        for seg in &s.segments {
            match seg {
                Segment::Slot(s) => {
                    out.push((s.path.to_string(), s.raw.clone(), s.display.clone()))
                }
                Segment::Choice(c) => {
                    out.push((c.path.to_string(), c.raw.clone(), c.display.clone()))
                }
                Segment::List(l) => {
                    for i in &l.items {
                        let s = &i.slot;
                        out.push((s.path.to_string(), s.raw.clone(), s.display.clone()));
                        sentence(
                            &Sentence {
                                segments: i.extra.clone(),
                            },
                            out,
                        );
                    }
                }
                _ => {}
            }
        }
    }
    fn conditions(l: &ConditionList, out: &mut Vec<(String, String, String)>) {
        for c in &l.items {
            sentence(&c.sentence, out);
            if let Some(kids) = &c.children {
                conditions(kids, out);
            }
        }
    }
    fn rule(r: &RuleProse, out: &mut Vec<(String, String, String)>) {
        sentence(&r.sentence, out);
        for g in &r.refinements {
            conditions(&g.conditions, out);
        }
        conditions(&r.conditions, out);
        for f in &r.follow_ups {
            for r in &f.rules {
                rule(r, out);
            }
        }
    }
    let mut out = Vec::new();
    sentence(&p.heading, &mut out);
    sentence(&p.intro, &mut out);
    for n in &p.notes {
        sentence(n, &mut out);
    }
    for g in &p.refinements {
        conditions(&g.conditions, &mut out);
    }
    for r in &p.rules {
        rule(r, &mut out);
    }
    out
}

fn slot_at<'a>(all: &'a [(String, String, String)], path: &str) -> &'a (String, String, String) {
    all.iter()
        .find(|(p, _, _)| p == path)
        .unwrap_or_else(|| panic!("no slot {path} in {all:?}"))
}

fn sp(s: &str) -> SlotPath {
    s.parse().unwrap()
}

#[test]
fn sentences_with_default_equals_sentences_research_policy() {
    // The v0.1 fixtures are covered in render_with.rs; this is the profile
    // policy, read without a profile.
    let doc = research_doc();
    assert_eq!(
        format!("{:#?}", sentences_with(&doc, &Wording::default())),
        format!("{:#?}", sentences(&doc))
    );
}

#[test]
fn slot_display_uses_label_raw_unchanged() {
    let doc = research_doc();
    let all = slots(&sentences_with(&doc, &research())[0]);
    let action = slot_at(&all, "policy[0].permission[0].action[0]#name");
    assert_eq!(
        (action.1.as_str(), action.2.as_str()),
        ("ex:fineTune", "fine-tune")
    );
    let left = slot_at(&all, "policy[0].permission[0].constraint[0]#leftOperand");
    assert_eq!(
        (left.1.as_str(), left.2.as_str()),
        ("ex:researchPurpose", "the research purpose")
    );
    let clinical = slot_at(
        &all,
        "policy[0].permission[0].constraint[0]#rightOperand[0]",
    );
    assert_eq!(
        (clinical.1.as_str(), clinical.2.as_str()),
        ("ex:clinical", "clinical research")
    );
    let academic = slot_at(
        &all,
        "policy[0].permission[0].constraint[0]#rightOperand[1]",
    );
    assert_eq!(
        (academic.1.as_str(), academic.2.as_str()),
        ("ex:academic", "academic research")
    );
    // A typed literal is not a profile term and reads as before.
    let period = slot_at(
        &all,
        "policy[0].permission[0].constraint[1]#rightOperand[0]",
    );
    assert_eq!((period.1.as_str(), period.2.as_str()), ("P2Y", "P2Y"));
    // An undeclared term and an ODRL term keep the built-in wording.
    let distill = slot_at(&all, "policy[0].permission[1].action[0]#name");
    assert_eq!(distill.2, "distill");
    let core = slot_at(&all, "policy[0].prohibition[0].action[0]#name");
    assert_eq!(core.2, "commercialize");
    // Without the profile, the IRI value reads as written.
    let plain = slots(&sentences(&doc)[0]);
    let clinical = slot_at(
        &plain,
        "policy[0].permission[0].constraint[0]#rightOperand[0]",
    );
    assert_eq!(clinical.2, "ex:clinical");
}

#[test]
fn right_operand_label_in_the_sentence() {
    let doc = research_doc();
    let p = &sentences_with(&doc, &research())[0];
    assert_eq!(
        p.rules[0].conditions.items[0].sentence.plain(),
        "the research purpose is any of clinical research or academic research"
    );
}

#[test]
fn operator_choice_display_label() {
    let doc = read_model(
        &json!({
            "@context": ["http://www.w3.org/ns/odrl.jsonld",
                         {"ex": "https://example.org/odrl/profile/research/"}],
            "@type": "Set", "uid": "urn:policy:op",
            "permission": [{"target": "urn:asset:1", "action": "use",
                "constraint": [{"leftOperand": "spatial", "operator": "ex:withinRegion",
                                "rightOperand": "EU"}]}]
        })
        .to_string(),
    )
    .unwrap();
    let all = slots(&sentences_with(&doc, &research())[0]);
    let op = slot_at(&all, "policy[0].permission[0].constraint[0]#operator");
    assert_eq!(
        (op.1.as_str(), op.2.as_str()),
        ("ex:withinRegion", "is within region")
    );
    let plain = slots(&sentences(&doc)[0]);
    assert_eq!(
        slot_at(&plain, "policy[0].permission[0].constraint[0]#operator").2,
        "is related by within region to"
    );
}

#[test]
fn refinement_group_uses_profile_action_label() {
    let doc = research_doc();
    let p = &sentences_with(&doc, &research())[0];
    let group = &p.rules[0].refinements[0];
    assert_eq!(group.action_name, "fine-tune");
    assert_eq!(
        group.conditions.items[0].sentence.plain(),
        "the number of data subjects is at least 50"
    );
    assert_eq!(
        sentences(&doc)[0].rules[0].refinements[0].action_name,
        "fine tune"
    );
}

#[test]
fn left_operand_at_for_right_operand_unit_reference() {
    let doc = research_doc();
    assert_eq!(
        left_operand_at(
            &doc,
            &sp("policy[0].permission[0].constraint[0]#rightOperand[1]")
        ),
        Some("ex:researchPurpose")
    );
    assert_eq!(
        left_operand_at(
            &doc,
            &sp("policy[0].permission[0].action[0].refinement[0]#rightOperand[0]")
        ),
        Some("ex:dataSubjectCount")
    );
    let doc = read_model(
        &json!({
            "@context": "http://www.w3.org/ns/odrl.jsonld",
            "@type": "Set", "uid": "urn:policy:u",
            "permission": [{"target": "urn:asset:1", "action": "use",
                "constraint": [
                    {"leftOperand": "payAmount", "operator": "eq", "rightOperand": 5,
                     "unit": "http://dbpedia.org/resource/Euro"},
                    {"or": [{"leftOperand": "dateTime", "operator": "lt",
                             "rightOperandReference": "http://example.com/deadline"}]}
                ]}]
        })
        .to_string(),
    )
    .unwrap();
    assert_eq!(
        left_operand_at(&doc, &sp("policy[0].permission[0].constraint[0]#unit")),
        Some("payAmount")
    );
    assert_eq!(
        left_operand_at(
            &doc,
            &sp("policy[0].permission[0].constraint[1].child[0]#rightOperandReference")
        ),
        Some("dateTime")
    );
}

#[test]
fn left_operand_at_none_elsewhere() {
    let doc = research_doc();
    for s in [
        "policy[0].permission[0].constraint[0]#leftOperand",
        "policy[0].permission[0].constraint[0]#operator",
        "policy[0].permission[0].action[0]#name",
        "policy[0]#profile[0]",
        "policy[0]#kind",
        "policy[0].permission[9].constraint[0]#rightOperand[0]",
        "policy[3].permission[0].constraint[0]#rightOperand[0]",
    ] {
        assert_eq!(left_operand_at(&doc, &sp(s)), None, "{s}");
    }
    // A logical constraint has no left operand of its own.
    let doc = read_model(
        &json!({
            "@context": "http://www.w3.org/ns/odrl.jsonld",
            "@type": "Set", "uid": "urn:policy:l",
            "permission": [{"target": "urn:asset:1", "action": "use",
                "constraint": [{"and": [{"leftOperand": "count", "operator": "lt",
                                         "rightOperand": 3}]}]}]
        })
        .to_string(),
    )
    .unwrap();
    assert_eq!(
        left_operand_at(
            &doc,
            &sp("policy[0].permission[0].constraint[0]#rightOperand[0]")
        ),
        None
    );
    // An empty left operand is no left operand.
    let mut doc = research_doc();
    set_text(
        &mut doc,
        "policy[0].permission[0].constraint[0]#leftOperand",
        "",
    )
    .unwrap();
    assert_eq!(
        left_operand_at(
            &doc,
            &sp("policy[0].permission[0].constraint[0]#rightOperand[0]")
        ),
        None
    );
}

#[test]
fn write_jsonld_identical_with_or_without_profile() {
    let mut doc = research_doc();
    let before = write_jsonld(&doc);
    let _ = sentences_with(&doc, &research());
    assert_eq!(write_jsonld(&doc), before);
    // An edit made while a profile is loaded writes what was typed.
    set_text(
        &mut doc,
        "policy[0].permission[1].action[0]#name",
        "ex:train",
    )
    .unwrap();
    let written = write_jsonld(&doc);
    assert_eq!(written["permission"][1]["action"], json!("ex:train"));
}

// --- check_profile -------------------------------------------------------------

const RESEARCH: &str = "https://example.org/odrl/profile/research";

fn doc_of(v: serde_json::Value) -> EditDoc {
    read_model(&v.to_string()).unwrap()
}

/// A Set with the research prefix bound and one permission holding `rule`.
fn policy_with(rule: serde_json::Value) -> EditDoc {
    doc_of(json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld",
                     {"ex": "https://example.org/odrl/profile/research/",
                      "other": "https://example.com/other/"}],
        "@type": "Set", "uid": "urn:policy:c",
        "permission": [rule]
    }))
}

fn at(issues: &[Issue], slot: &str) -> Vec<(Severity, String)> {
    let want = IssueTarget::Slot(sp(slot));
    issues
        .iter()
        .filter(|i| i.target == want)
        .map(|i| (i.severity, i.message.clone()))
        .collect()
}

fn on_slots(issues: &[Issue]) -> Vec<String> {
    issues
        .iter()
        .filter_map(|i| match &i.target {
            IssueTarget::Slot(s) => Some(s.to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn undeclared_term_warning_on_slot() {
    let issues = check_profile(&research_doc(), &research());
    assert_eq!(
        at(&issues, "policy[0].permission[1].action[0]#name"),
        [(
            Severity::Warning,
            format!("the action \"ex:distill\" is not declared by the loaded profile <{RESEARCH}>")
        )]
    );
    // Declared and ODRL terms are fine; the research policy says nothing else.
    assert_eq!(issues.len(), 1, "{issues:?}");

    let doc = policy_with(json!({
        "target": "urn:asset:1", "action": "ex:pseudonymise",
        "constraint": [{"leftOperand": "ex:nope", "operator": "ex:nopeOp",
                        "rightOperand": [{"@id": "ex:nopeVal"}, {"@id": "ex:academic"}]},
                       {"leftOperand": "other:x", "operator": "other:op",
                        "rightOperand": {"@id": "other:v"}}]
    }));
    let issues = check_profile(&doc, &research());
    let c = "policy[0].permission[0].constraint[0]";
    for (field, what, term) in [
        ("leftOperand", "left operand", "ex:nope"),
        ("operator", "operator", "ex:nopeOp"),
        ("rightOperand[0]", "right operand", "ex:nopeVal"),
    ] {
        assert_eq!(
            at(&issues, &format!("{c}#{field}")),
            [(
                Severity::Warning,
                format!("the {what} \"{term}\" is not declared by the loaded profile <{RESEARCH}>")
            )],
            "{field}"
        );
    }
    // A term in a namespace no loaded profile uses is silent: mixing
    // profiles is legal.
    assert_eq!(
        on_slots(&issues),
        [
            format!("{c}#leftOperand"),
            format!("{c}#operator"),
            format!("{c}#rightOperand[0]")
        ]
    );
}

#[test]
fn undeclared_terms_found_everywhere_in_the_tree() {
    let doc = doc_of(json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld",
                     {"ex": "https://example.org/odrl/profile/research/"}],
        "@type": "Set", "uid": "urn:policy:t",
        "action": "ex:p0",
        "permission": [{
            "target": {"@type": "AssetCollection", "source": "urn:c",
                       "refinement": [{"leftOperand": "ex:p1", "operator": "eq",
                                       "rightOperand": 1}]},
            "action": "use",
            "constraint": [{"or": [{"leftOperand": "ex:p2", "operator": "eq",
                                    "rightOperand": 1}]}],
            "duty": [{"action": "ex:p3",
                      "consequence": [{"action": "ex:p4"}]}]
        }]
    }));
    let issues = check_profile(&doc, &research());
    let slots = on_slots(&issues);
    for s in [
        "policy[0].action[0]#name",
        "policy[0].permission[0].constraint[0].child[0]#leftOperand",
        "policy[0].permission[0].duty[0].action[0]#name",
        "policy[0].permission[0].duty[0].consequence[0].action[0]#name",
    ] {
        assert!(slots.iter().any(|x| x == s), "{s} not in {slots:?}");
    }
    assert!(
        slots
            .iter()
            .any(|x| x.ends_with("refinement[0]#leftOperand") && x.contains("target[0]")),
        "{slots:?}"
    );
}

#[test]
fn deprecated_term_warning() {
    let w = Wording::new().with_profile(
        ProfileVocab::from_value(
            &json!({
                "@context": {"odrl": "http://www.w3.org/ns/odrl/2/",
                             "owl": "http://www.w3.org/2002/07/owl#",
                             "ex": "https://example.org/odrl/profile/research/"},
                "@graph": [
                    {"@id": "https://example.org/odrl/profile/research", "@type": "owl:Ontology"},
                    {"@id": "ex:old", "@type": "odrl:Action", "owl:deprecated": true}
                ]
            }),
            &Default::default(),
        )
        .unwrap(),
    );
    let doc = policy_with(json!({"target": "urn:asset:1", "action": "ex:old"}));
    assert_eq!(
        at(
            &check_profile(&doc, &w),
            "policy[0].permission[0].action[0]#name"
        ),
        [(
            Severity::Warning,
            format!("\"ex:old\" is deprecated in the profile <{RESEARCH}>")
        )]
    );
}

#[test]
fn declared_profile_not_loaded_on_node() {
    let doc = doc_of(json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set", "uid": "urn:policy:p",
        "profile": ["https://example.org/other", RESEARCH,
                    "http://www.w3.org/ns/odrl/2/core"],
        "permission": [{"target": "urn:asset:1", "action": "use"}]
    }));
    let issues = check_profile(&doc, &research());
    assert_eq!(
        issues,
        [Issue {
            target: IssueTarget::Node(NodePath::policy(0)),
            severity: Severity::Warning,
            message:
                "the policy declares the profile <https://example.org/other>, which is not loaded"
                    .to_string(),
        }]
    );
}

#[test]
fn operator_outside_hints_is_info() {
    let mut doc = research_doc();
    let op = "policy[0].permission[0].action[0].refinement[0]#operator";
    let expect = doc.node(&sp(op).node).unwrap().id();
    doc.apply(
        &EditEvent::SetChoice {
            slot: sp(op),
            expect,
            value: "eq".to_string(),
        },
        &EditRules::default(),
    )
    .unwrap();
    let issues = check_profile(&doc, &research());
    let got = at(&issues, op);
    assert_eq!(got.len(), 1, "{issues:?}");
    assert_eq!(got[0].0, Severity::Info);
    assert_eq!(
        got[0].1,
        "the profile suggests the operators gteq or gt for \"ex:dataSubjectCount\""
    );
    // The same operator written as odrl:gteq or in full is a suggested one.
    for op_text in ["odrl:gteq", "http://www.w3.org/ns/odrl/2/gteq", "gt"] {
        let doc = policy_with(json!({
            "target": "urn:asset:1", "action": "use",
            "constraint": [{"leftOperand": "ex:dataSubjectCount", "operator": op_text,
                            "rightOperand": 50}]
        }));
        assert!(check_profile(&doc, &research()).is_empty(), "{op_text}");
    }
}

#[test]
fn datatype_integer_duration_checks() {
    // The research policy's literals fit their datatypes.
    assert!(
        at(
            &check_profile(&research_doc(), &research()),
            "policy[0].permission[0].action[0].refinement[0]#rightOperand[0]"
        )
        .is_empty()
    );
    let check = |left: &str, right: serde_json::Value| -> Vec<(Severity, String)> {
        let doc = policy_with(json!({
            "target": "urn:asset:1", "action": "use",
            "constraint": [{"leftOperand": left, "operator": "eq", "rightOperand": right}]
        }));
        let w = research();
        let issues = check_profile(&doc, &w);
        at(
            &issues,
            "policy[0].permission[0].constraint[0]#rightOperand[0]",
        )
    };
    let count = "ex:dataSubjectCount";
    for ok in [
        json!(50),
        json!("50"),
        json!("-3"),
        json!("+7"),
        json!({"@value": "50", "@type": "xsd:integer"}),
    ] {
        assert!(check(count, ok.clone()).is_empty(), "{ok}");
    }
    for bad in [
        json!("fifty"),
        json!(1.5),
        json!(""),
        json!("5 0"),
        json!("-"),
    ] {
        assert_eq!(
            check(count, bad.clone()),
            [(
                Severity::Info,
                format!(
                    "\"{}\" is not an integer, which the profile expects for \"ex:dataSubjectCount\"",
                    match &bad {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    }
                )
            )],
            "{bad}"
        );
    }
    let period = "ex:retentionPeriod";
    for ok in ["P2Y", "P1Y2M10DT2H30M", "PT0.5S", "-P3D", "PT1H"] {
        assert!(check(period, json!(ok)).is_empty(), "{ok}");
    }
    for bad in ["two years", "P", "PT", "P2", "2Y", "P1H", "PT1D", "P1.5Y"] {
        let got = check(period, json!(bad));
        assert_eq!(got.len(), 1, "{bad}");
        assert_eq!(got[0].0, Severity::Info);
        assert!(got[0].1.contains("not a duration"), "{}", got[0].1);
    }
    // An IRI value is not checked against a datatype.
    assert!(check(count, json!({"@id": "ex:academic"})).is_empty());
}

#[test]
fn datatype_decimal_boolean_date_datetime_checks() {
    let w = Wording::new().with_profile(
        ProfileVocab::from_value(
            &json!({
                "@context": {"odrl": "http://www.w3.org/ns/odrl/2/",
                             "xsd": "http://www.w3.org/2001/XMLSchema#",
                             "prose": "https://ds-labs-org.github.io/ds-prose-rs/profile#",
                             "ex": "https://example.org/odrl/profile/research/"},
                "@graph": [
                    {"@id": "ex:dec", "@type": "odrl:LeftOperand", "prose:datatype": "xsd:decimal"},
                    {"@id": "ex:flag", "@type": "odrl:LeftOperand", "prose:datatype": "xsd:boolean"},
                    {"@id": "ex:day", "@type": "odrl:LeftOperand", "prose:datatype": "xsd:date"},
                    {"@id": "ex:when", "@type": "odrl:LeftOperand", "prose:datatype": "xsd:dateTime"},
                    {"@id": "ex:any", "@type": "odrl:LeftOperand", "prose:datatype": "xsd:anyURI"}
                ]
            }),
            &Default::default(),
        )
        .unwrap(),
    );
    let check = |left: &str, right: serde_json::Value| -> usize {
        let doc = policy_with(json!({
            "target": "urn:asset:1", "action": "use",
            "constraint": [{"leftOperand": left, "operator": "eq", "rightOperand": right}]
        }));
        at(
            &check_profile(&doc, &w),
            "policy[0].permission[0].constraint[0]#rightOperand[0]",
        )
        .len()
    };
    for (left, ok, bad) in [
        (
            "ex:dec",
            vec![
                json!("1.5"),
                json!(".5"),
                json!("-2"),
                json!(3),
                json!(2.25),
            ],
            vec![json!("1,5"), json!("1e3"), json!("."), json!("abc")],
        ),
        (
            "ex:flag",
            vec![json!(true), json!("false"), json!("1"), json!("0")],
            vec![json!("yes"), json!("True")],
        ),
        (
            "ex:day",
            vec![
                json!("2026-01-31"),
                json!("2026-01-31Z"),
                json!("2026-01-31+02:00"),
            ],
            vec![
                json!("2026-1-31"),
                json!("2026-13-01"),
                json!("2026-01-32"),
                json!("31.01.2026"),
            ],
        ),
        (
            "ex:when",
            vec![
                json!("2026-01-31T10:00:00"),
                json!("2026-01-31T10:00:00.5Z"),
                json!("2026-01-31T10:00:00-05:00"),
            ],
            vec![
                json!("2026-01-31"),
                json!("2026-01-31 10:00:00"),
                json!("2026-01-31T25:00:00"),
            ],
        ),
        ("ex:any", vec![json!("anything at all")], vec![]),
    ] {
        for v in ok {
            assert_eq!(check(left, v.clone()), 0, "{left} {v}");
        }
        for v in bad {
            assert_eq!(check(left, v.clone()), 1, "{left} {v}");
        }
    }
}

#[test]
fn skos_note_never_parsed() {
    // ODRL's own elapsedTime says in prose that its value is a duration;
    // no hint is read from that, so nothing is checked.
    let w = Wording::new()
        .with_profile(ProfileVocab::parse(&profile_text("odrl22-excerpt.json")).unwrap());
    let doc = policy_with(json!({
        "target": "urn:asset:1", "action": "use",
        "constraint": [{"leftOperand": "elapsedTime", "operator": "isA",
                        "rightOperand": "not a duration"}]
    }));
    assert_eq!(check_profile(&doc, &w), []);
}

#[test]
fn empty_wording_no_issues() {
    let w = Wording::default();
    for name in FIXTURES {
        let doc = read_model(&fixture(name)).unwrap();
        assert_eq!(check_profile(&doc, &w), [], "{name}");
    }
    assert_eq!(check_profile(&research_doc(), &w), []);
    let mut doc = research_doc();
    set_text(
        &mut doc,
        "policy[0].permission[0].action[0].refinement[0]#rightOperand[0]",
        "fifty",
    )
    .unwrap();
    assert_eq!(check_profile(&doc, &w), []);
}

#[test]
fn opaque_constraints_are_not_checked() {
    // Two logical operators: kept as written, with no slots to flag.
    let doc = policy_with(json!({
        "target": "urn:asset:1", "action": "use",
        "constraint": [{
            "and": [{"leftOperand": "ex:nope", "operator": "eq", "rightOperand": 1}],
            "or": [{"leftOperand": "ex:nope2", "operator": "eq", "rightOperand": 1}]
        }]
    }));
    assert!(
        doc.node(&"policy[0].permission[0].constraint[0]".parse().unwrap())
            .unwrap()
            .locked()
            .is_some()
    );
    assert_eq!(check_profile(&doc, &research()), []);
}

#[test]
fn picked_right_operand_writes_at_id() {
    let mut doc = research_doc();
    // A value picked from the profile is an IRI and is written as one.
    if let ConstraintNode::Atomic(a) = &mut doc.policies[0].permission[0].constraint[0]
        && let RightOperand::Values(v) = &mut a.right
    {
        v[1] = Literal::Iri("ex:commercialResearch".to_string());
    } else {
        panic!("not an atomic constraint");
    }
    let written = write_jsonld(&doc);
    assert_eq!(
        written["permission"][0]["constraint"][0]["rightOperand"],
        json!([{"@id": "ex:clinical"}, {"@id": "ex:commercialResearch"}])
    );
    // Typing over an IRI value keeps it an IRI.
    set_text(
        &mut doc,
        "policy[0].permission[0].constraint[0]#rightOperand[0]",
        "ex:academic",
    )
    .unwrap();
    assert_eq!(
        write_jsonld(&doc)["permission"][0]["constraint"][0]["rightOperand"][0],
        json!({"@id": "ex:academic"})
    );
}

#[test]
fn reducer_still_accepts_out_of_profile_text() {
    let mut doc = research_doc();
    let w = research();
    set_text(
        &mut doc,
        "policy[0].permission[0].action[0]#name",
        "ex:madeUp",
    )
    .unwrap();
    set_text(
        &mut doc,
        "policy[0].permission[0].constraint[1]#leftOperand",
        "ex:alsoMadeUp",
    )
    .unwrap();
    let op = "policy[0].permission[0].action[0].refinement[0]#operator";
    let expect = doc.node(&sp(op).node).unwrap().id();
    doc.apply(
        &EditEvent::SetChoice {
            slot: sp(op),
            expect,
            value: "ex:nopeOp".to_string(),
        },
        &EditRules::default(),
    )
    .unwrap();
    let issues = check_profile(&doc, &w);
    assert!(
        !at(&issues, "policy[0].permission[0].action[0]#name").is_empty(),
        "{issues:?}"
    );
    let written = write_jsonld(&doc);
    assert_eq!(
        written["permission"][0]["constraint"][1]["leftOperand"],
        json!("ex:alsoMadeUp")
    );
}
