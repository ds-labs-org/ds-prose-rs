mod common;
use common::*;
use prose_core::edit::*;

fn prose(name: &str) -> Vec<PolicyProse> {
    sentences(&read_model(&fixture(name)).unwrap())
}

fn list_of(seg: &Segment) -> &SlotList {
    match seg {
        Segment::List(l) => l,
        other => panic!("{other:?}"),
    }
}

#[test]
fn offer_reads_as_in_v01() {
    let p = &prose("offer")[0];
    assert_eq!(p.heading.plain(), "Offer http://example.com/policy:1010");
    let rule = &p.rules[0];
    assert_eq!(
        rule.sentence.plain(),
        "Anyone may display http://example.com/asset:9898.movie, as permitted by http://example.com/org:abc."
    );
    assert_eq!(
        rule.conditions.items[0].sentence.plain(),
        "the date and time is before 2026-12-31"
    );
    let duty = &rule.follow_ups[0].rules[0];
    assert_eq!(
        duty.sentence.plain(),
        "The assignee must attribute http://example.com/asset:9898.movie, as required by http://example.com/org:abc."
    );
}

#[test]
fn policy_intro_and_notes_always_show_every_slot() {
    let p = &prose("agreement")[0];
    assert_eq!(
        p.intro.plain(),
        "Between http://example.com/party:alice (assigner) and http://example.com/party:bob (assignee)."
    );
    let notes: Vec<String> = p.notes.iter().map(Sentence::plain).collect();
    assert_eq!(
        notes,
        [
            "Applies to .",
            "Covers the action .",
            "Written to the profile no profile.",
            "If a permission and a prohibition conflict: the prohibition wins.",
            "Inherits the rules of no other policy.",
        ]
    );
    let none = &prose("offer")[0];
    assert_eq!(
        none.intro.plain(),
        "Between http://example.com/org:abc (assigner) and anyone (assignee)."
    );
    assert!(none.notes[3].plain().ends_with("not stated."));
}

#[test]
fn inherited_items_are_populated_and_shown_when_own_is_empty() {
    let p = &prose("set")[0];
    let rule = &p.rules[0];
    assert_eq!(
        rule.sentence.plain(),
        "Anyone must not commercialize http://example.com/data:set1."
    );
    let target = list_of(&rule.sentence.segments[6]);
    assert!(target.items.is_empty());
    assert_eq!(target.inherited.len(), 1);
    assert_eq!(target.inherited[0].display, "http://example.com/data:set1");
    assert_eq!(rule.list, RuleList::Prohibition);
    // The remedy has no target of its own and inherits none.
    let remedy = &rule.follow_ups[0].rules[0];
    assert_eq!(remedy.sentence.plain(), "The assignee must compensate.");
    assert_eq!(
        remedy.conditions.items[0].sentence.plain(),
        "the payment amount is equal to 100.00 euro"
    );
}

#[test]
fn a_rule_that_says_nothing_inherits_the_policy_action_and_target() {
    let p = &prose("policy-action")[0];
    assert_eq!(p.rules[0].sentence.plain(), "Anyone may use urn:asset:8.");
    assert_eq!(p.refinements.len(), 1);
    assert_eq!(p.refinements[0].action_name, "use");
    assert_eq!(
        p.refinements[0].conditions.items[0].sentence.plain(),
        "the location is equal to urn:geo:EU"
    );
}

#[test]
fn set_operators_join_with_or() {
    let p = &prose("agreement")[0];
    let rule = &p.rules[0];
    let or = &rule.conditions.items[0];
    assert_eq!(or.sentence.plain(), "at least one of the following holds");
    let kids = or.children.as_ref().unwrap();
    assert_eq!(
        kids.items[0].sentence.plain(),
        "the purpose is any of \"research\" or \"education\""
    );
    let values = list_of(&kids.items[0].sentence.segments[4]);
    assert_eq!(values.conj, Conj::Or);
    assert_eq!(values.items.len(), 2);
    assert_eq!(
        kids.items[1].sentence.plain(),
        "the location is equal to https://example.com/geo/EU"
    );
    assert_eq!(list_of(&kids.items[1].sentence.segments[4]).conj, Conj::And);
    // The refinement of the action, with its own group.
    assert_eq!(
        rule.refinements[0].conditions.items[0].sentence.plain(),
        "the number of times used is at most 5"
    );
}

#[test]
fn odrl_position_is_reported_per_follow_up() {
    let doc = rich();
    let p = &sentences(&doc)[0];
    let perm = &p.rules[0];
    let kinds: Vec<(RuleList, bool, usize)> = perm
        .follow_ups
        .iter()
        .map(|f| (f.kind, f.in_odrl_position, f.rules.len()))
        .collect();
    // A permission offers duties; remedies and consequences only if present.
    assert_eq!(kinds, [(RuleList::Duty, true, 1)]);
    let proh = &p.rules[1];
    assert_eq!(proh.list, RuleList::Prohibition);
    let kinds: Vec<(RuleList, bool)> = proh
        .follow_ups
        .iter()
        .map(|f| (f.kind, f.in_odrl_position))
        .collect();
    assert_eq!(kinds, [(RuleList::Remedy, true)]);
    let duty = &perm.follow_ups[0].rules[0];
    assert_eq!(duty.follow_ups.len(), 1);
    assert_eq!(duty.follow_ups[0].kind, RuleList::Consequence);
    assert!(duty.follow_ups[0].in_odrl_position);
    assert_eq!(duty.follow_ups[0].parent, RuleList::Duty);

    // A duty under a prohibition is carried, outside an ODRL position.
    let v = serde_json::json!({"@type": "Set", "prohibition": [{"action": "copy", "duty": [{"action": "inform"}]}]});
    let doc = read_model_value(&v).unwrap();
    let p = &sentences(&doc)[0];
    let kinds: Vec<(RuleList, bool)> = p.rules[0]
        .follow_ups
        .iter()
        .map(|f| (f.kind, f.in_odrl_position))
        .collect();
    assert_eq!(kinds, [(RuleList::Duty, false), (RuleList::Remedy, true)]);
}

#[test]
fn entities_references_and_units() {
    let p = &prose("entity-collection")[0];
    assert_eq!(
        p.rules[0].sentence.plain(),
        "urn:party:bob (a member of urn:collection:staff) may use any asset in the collection urn:collection:movies where the file format is equal to \"mp4\"."
    );
    let p = &prose("rule-reference")[0];
    assert_eq!(
        p.rules[0].sentence.plain(),
        "The rule urn:rule:external-1 is referenced here but not defined."
    );
    assert_eq!(p.rules[0].path.to_string(), "policy[0].permission[0]");
    let p = &prose("operand-reference")[0];
    let c = &p.rules[0].conditions.items;
    assert_eq!(
        c[0].sentence.plain(),
        "the payment amount is at least 10.50 euro"
    );
    assert_eq!(
        c[1].sentence.plain(),
        "the number of times used is less than the value at urn:ref:limit"
    );
    let p = &prose("inheritance")[0];
    assert_eq!(
        p.notes[2].plain(),
        "Written to the profile urn:profile:one and urn:profile:two."
    );
    assert_eq!(
        p.notes[3].plain(),
        "If a permission and a prohibition conflict: the permission wins."
    );
    assert_eq!(
        p.notes[4].plain(),
        "Inherits the rules of urn:policy:parent."
    );
}

#[test]
fn locked_nodes_have_a_reason_and_a_cut_excerpt() {
    let long = "x".repeat(300);
    let v = serde_json::json!({"@type": "Set", "permission": [7, {"action": "use", "constraint": [{"and": [], "or": []}, {"leftOperand": long}]}]});
    let doc = read_model_value(&v).unwrap();
    let p = &sentences(&doc)[0];
    let locked = p.rules[0].locked.as_ref().unwrap();
    assert_eq!(locked.reason, "not a rule");
    assert_eq!(locked.json, "7");
    assert_eq!(p.rules[0].sentence, Sentence::default());
    let c = &p.rules[1].conditions.items[0];
    assert_eq!(
        c.locked.as_ref().unwrap().reason,
        "several logical operators"
    );
    // Only a read-through of the whole origin is cut.
    let v = serde_json::json!({"@type": "Set", "permission": [{"action": "use", "constraint": [{"and": [], "or": [], "k": "y".repeat(400)}]}]});
    let doc = read_model_value(&v).unwrap();
    let c = &sentences(&doc)[0].rules[0].conditions.items[0];
    let json = &c.locked.as_ref().unwrap().json;
    assert_eq!(json.chars().count(), 203);
    assert!(json.ends_with("..."));
}

#[test]
fn slots_carry_paths_that_resolve_and_owner_ids_that_match() {
    for name in FIXTURES {
        let doc = read_model(&fixture(name)).unwrap();
        for p in sentences(&doc) {
            check_policy(&doc, &p);
        }
    }
}

fn check_slot(doc: &EditDoc, s: &Slot) {
    let node = doc
        .node(&s.path.node)
        .unwrap_or_else(|| panic!("{}", s.path));
    assert_eq!(node.id(), s.owner, "{}", s.path);
}

fn check_segments(doc: &EditDoc, segs: &[Segment]) {
    for seg in segs {
        match seg {
            Segment::Text(_) => {}
            Segment::Slot(s) => check_slot(doc, s),
            Segment::Choice(c) => {
                assert_eq!(doc.node(&c.path.node).unwrap().id(), c.owner);
            }
            Segment::RuleKind(k) => assert_eq!(doc.node(&k.rule).unwrap().id(), k.owner),
            Segment::List(l) => {
                if let Some(o) = &l.path.owner {
                    assert_eq!(doc.node(o).unwrap().id(), l.owner);
                }
                for i in &l.items {
                    check_slot(doc, &i.slot);
                    check_segments(doc, &i.extra);
                }
                l.inherited.iter().for_each(|s| check_slot(doc, s));
            }
            // `Segment` is non_exhaustive: a new variant needs its own check.
            other => panic!("no ownership check for {other:?}"),
        }
    }
}

fn check_conditions(doc: &EditDoc, c: &ConditionList) {
    for item in &c.items {
        assert_eq!(doc.node(&item.path).unwrap().id(), item.node);
        check_segments(doc, &item.sentence.segments);
        if let Some(k) = &item.children {
            check_conditions(doc, k);
        }
    }
}

fn check_rule(doc: &EditDoc, r: &RuleProse) {
    assert_eq!(doc.node(&r.path).unwrap().id(), r.node);
    check_segments(doc, &r.sentence.segments);
    check_conditions(doc, &r.conditions);
    for g in &r.refinements {
        check_conditions(doc, &g.conditions);
    }
    for f in &r.follow_ups {
        f.rules.iter().for_each(|c| check_rule(doc, c));
    }
}

fn check_policy(doc: &EditDoc, p: &PolicyProse) {
    check_segments(doc, &p.heading.segments);
    check_segments(doc, &p.intro.segments);
    for n in &p.notes {
        check_segments(doc, &n.segments);
    }
    for g in &p.refinements {
        check_conditions(doc, &g.conditions);
    }
    p.rules.iter().for_each(|r| check_rule(doc, r));
    assert_eq!(p.add_rule_lists.len(), 3);
}
