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
