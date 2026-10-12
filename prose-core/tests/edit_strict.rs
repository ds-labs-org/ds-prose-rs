//! The strict reducer gate: [`Validator::admissible`], [`EditDoc::apply_gated`]
//! and [`EditDoc::apply_all`].
//!
//! A refusal is opt-in ([`StrictEdit::RefuseTerms`]), slot-local and keyed on
//! the written value only. Keystrokes, right-operand literals, the empty
//! string, structural events and undo are never refused, and nothing is
//! refused while the vocabulary is incomplete.
mod common;
mod corpus;
use common::*;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::Wording;
use prose_core::validate::*;
use serde_json::{Value, json};
use std::sync::Arc;

const RESEARCH: &str = "https://example.org/odrl/profile/research";
const EX: &str = "https://example.org/odrl/profile/research/";

const ACTION: &str = "policy[0].permission[0].action[0]#name";
const PURPOSE: &str = "policy[0].permission[0].constraint[0]";
const COUNT: &str = "policy[0].permission[0].constraint[1]";

fn closed() -> Wording {
    wording_of(&["research-closed.jsonld"])
}

fn strict_gate(w: Wording) -> Validator {
    Validator::new(Arc::new(w), Options::strict())
}

/// A Set under the research profile, with the ex and foo prefixes bound:
/// `use` constrained by the closed `researchPurpose` and `dataSubjectCount`
/// operands. It conforms in strict mode with the closed research profile.
fn base_value() -> Value {
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld",
                     {"ex": EX, "foo": "https://example.net/foo/"}],
        "@type": "Set", "uid": "urn:policy:s",
        "profile": RESEARCH,
        "conflict": "perm",
        "permission": [{
            "target": "urn:asset:1", "assigner": "urn:party:a", "action": "use",
            "constraint": [
                {"leftOperand": "ex:researchPurpose", "operator": "isAnyOf",
                 "rightOperand": [{"@id": "ex:academic"}, {"@id": "ex:clinical"}]},
                {"leftOperand": "ex:dataSubjectCount", "operator": "gteq",
                 "rightOperand": 50}
            ]
        }]
    })
}

fn base() -> EditDoc {
    read_model(&base_value().to_string()).unwrap()
}

/// [`base`], declaring a second profile that is not loaded.
fn incomplete() -> EditDoc {
    let mut v = base_value();
    v["profile"] = json!([RESEARCH, "https://example.org/odrl/profile/missing"]);
    read_model(&v.to_string()).unwrap()
}

fn sp(s: &str) -> SlotPath {
    s.parse().unwrap()
}

fn expect_at(doc: &EditDoc, slot: &SlotPath) -> NodeId {
    doc.node(&slot.node).map(|n| n.id()).unwrap_or_default()
}

fn text(doc: &EditDoc, slot: &str, value: &str) -> EditEvent {
    let slot = sp(slot);
    EditEvent::SetText {
        expect: expect_at(doc, &slot),
        slot,
        value: value.to_string(),
    }
}

fn choice(doc: &EditDoc, slot: &str, value: &str) -> EditEvent {
    let slot = sp(slot);
    EditEvent::SetChoice {
        expect: expect_at(doc, &slot),
        slot,
        value: value.to_string(),
    }
}

fn iri(doc: &EditDoc, slot: &str, value: &str) -> EditEvent {
    let slot = sp(slot);
    EditEvent::SetIri {
        expect: expect_at(doc, &slot),
        slot,
        value: value.to_string(),
    }
}

/// The rule a refusal names, or panics when `r` is not a refusal.
fn refused_by(r: Result<(), EditError>) -> String {
    match r {
        Err(EditError::OutsideProfile(f)) => f.rule.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// Applies `ev` to a copy of `doc` through the gate.
fn gated(doc: &EditDoc, ev: &EditEvent, gate: Option<&Validator>) -> Result<EditDoc, EditError> {
    let mut d = doc.clone();
    d.apply_gated(ev, &EditRules::default(), gate).map(|()| d)
}

fn plain(doc: &EditDoc, ev: &EditEvent) -> Result<EditDoc, EditError> {
    let mut d = doc.clone();
    d.apply(ev, &EditRules::default()).map(|()| d)
}

#[test]
fn the_base_policy_conforms() {
    let r = strict_gate(closed()).validate(&base());
    assert_eq!(r.verdict, Verdict::Conforms, "{:?}", r.findings);
    assert_eq!(
        strict_gate(closed()).validate(&incomplete()).verdict,
        Verdict::Incomplete
    );
}

#[test]
fn refuse_foreign_action() {
    let doc = base();
    let v = strict_gate(closed());
    let cases = [
        // Resolves nowhere: neither ODRL nor a loaded profile.
        ("foo:bar", "profile.unknown-term"),
        // A left operand written as an action.
        ("ex:researchPurpose", "profile.wrong-kind"),
        // In the profile's namespace, but not declared by it.
        ("ex:distil", "profile.undeclared-in-namespace"),
    ];
    for (value, rule) in cases {
        let ev = text(&doc, ACTION, value);
        assert_eq!(refused_by(gated(&doc, &ev, Some(&v)).map(|_| ())), rule);
        assert!(v.admissible(&doc, &ev).is_err(), "{value}");
        // The document is unchanged by a refusal.
        let mut d = doc.clone();
        let _ = d.apply_gated(&ev, &EditRules::default(), Some(&v));
        assert_eq!(d, doc);
    }
    // ODRL's own actions and the profile's are admissible.
    for value in ["use", "read", "ex:train", &format!("{EX}fineTune")] {
        let ev = text(&doc, ACTION, value);
        assert!(v.admissible(&doc, &ev).is_ok(), "{value}");
        assert_eq!(gated(&doc, &ev, Some(&v)), plain(&doc, &ev), "{value}");
    }
}

#[test]
fn refuse_other_vocabulary_slots() {
    let doc = base();
    let v = strict_gate(closed());
    let refused = [
        (
            text(&doc, "policy[0]#kind", "ex:fineTune"),
            "profile.wrong-kind",
        ),
        (
            choice(&doc, "policy[0]#conflict", "foo:x"),
            "profile.conflict-term",
        ),
        (
            text(&doc, &format!("{PURPOSE}#leftOperand"), "ex:fineTune"),
            "profile.wrong-kind",
        ),
        (
            choice(&doc, &format!("{COUNT}#operator"), "foo:op"),
            "profile.unknown-term",
        ),
        // The operand is closed: only eq, isAnyOf and isNoneOf.
        (
            choice(&doc, &format!("{COUNT}#operator"), "eq"),
            "prose.operator-not-allowed",
        ),
    ];
    for (ev, rule) in refused {
        assert_eq!(
            refused_by(gated(&doc, &ev, Some(&v)).map(|_| ())),
            rule,
            "{ev:?}"
        );
    }
    let admitted = [
        text(&doc, "policy[0]#kind", "Offer"),
        choice(&doc, "policy[0]#conflict", "prohibit"),
        text(&doc, &format!("{PURPOSE}#leftOperand"), "purpose"),
        choice(&doc, &format!("{PURPOSE}#operator"), "isNoneOf"),
        choice(&doc, &format!("{COUNT}#operator"), "gt"),
    ];
    for ev in admitted {
        assert_eq!(gated(&doc, &ev, Some(&v)), plain(&doc, &ev), "{ev:?}");
    }
}

#[test]
fn an_unbound_prefix_is_refused_and_its_fix_is_not() {
    // The policy does not bind ex: only the profile's own prefixes read it.
    let mut v = base_value();
    v["@context"] = json!("http://www.w3.org/ns/odrl.jsonld");
    v["permission"][0]["constraint"] = json!([]);
    let doc = read_model(&v.to_string()).unwrap();
    let gate = strict_gate(closed());
    let ev = text(&doc, ACTION, "ex:train");
    assert_eq!(
        refused_by(gated(&doc, &ev, Some(&gate)).map(|_| ())),
        "profile.unbound-prefix"
    );
    let ev = text(&doc, ACTION, &format!("{EX}train"));
    assert!(gated(&doc, &ev, Some(&gate)).is_ok());
}

#[test]
fn refusal_after_stale_locked_notallowed() {
    let doc = base();
    let v = strict_gate(closed());
    let rules = EditRules::default();
    // Stale: the event was made against another node.
    let ev = EditEvent::SetText {
        slot: sp(ACTION),
        expect: NodeId(9999),
        value: "foo:bar".into(),
    };
    assert!(matches!(
        gated(&doc, &ev, Some(&v)),
        Err(EditError::Stale { .. })
    ));
    // Nothing there.
    let ev = EditEvent::SetText {
        slot: sp("policy[0].permission[7].action[0]#name"),
        expect: NodeId(1),
        value: "foo:bar".into(),
    };
    assert!(matches!(
        gated(&doc, &ev, Some(&v)),
        Err(EditError::NoSuchPath(_))
    ));
    // Not allowed: two values, and an operator that takes one; also
    // outside the closed operand's operators.
    let ev = choice(&doc, &format!("{PURPOSE}#operator"), "lteq");
    assert!(matches!(
        gated(&doc, &ev, Some(&v)),
        Err(EditError::NotAllowed(_))
    ));
    // Locked: an action the reader kept as written.
    let locked = read_model(
        &json!({"@context": {"foo": "https://example.net/foo/"},
                "@type": "Set", "uid": "urn:policy:l",
                "permission": [{"target": "urn:a", "action": {"rdf:value": 42}}]})
        .to_string(),
    )
    .unwrap();
    let ev = text(&locked, ACTION, "foo:bar");
    let want = locked.clone().apply(&ev, &rules);
    assert!(matches!(want, Err(EditError::Locked(_))), "{want:?}");
    assert_eq!(gated(&locked, &ev, Some(&v)).map(|_| ()), want);
}

#[test]
fn empty_always_admissible() {
    let doc = base();
    let v = strict_gate(closed());
    let events = [
        text(&doc, ACTION, ""),
        text(&doc, &format!("{PURPOSE}#leftOperand"), ""),
        choice(&doc, &format!("{PURPOSE}#operator"), ""),
        choice(&doc, &format!("{COUNT}#operator"), ""),
        choice(&doc, "policy[0]#conflict", ""),
    ];
    for ev in events {
        assert!(v.admissible(&doc, &ev).is_ok(), "{ev:?}");
        assert_eq!(gated(&doc, &ev, Some(&v)), plain(&doc, &ev), "{ev:?}");
    }
    // The reducer's own refusal of an empty policy type is unchanged.
    let ev = text(&doc, "policy[0]#kind", "");
    assert!(v.admissible(&doc, &ev).is_ok());
    assert!(matches!(
        gated(&doc, &ev, Some(&v)),
        Err(EditError::Invalid(_))
    ));
}

#[test]
fn free_text_slots_are_never_refused() {
    // What a person types into an identifier slot is data, not a term.
    let doc = base();
    let v = strict_gate(closed());
    for value in [
        "foo:bar",
        "ex:distil",
        "ex:researchPurpose",
        "zz:nope",
        "x y",
    ] {
        for ev in [
            text(&doc, "policy[0]#uid", value),
            text(&doc, "policy[0].permission[0].target[0]#iri", value),
            text(&doc, "policy[0].permission[0].assigner[0]#iri", value),
            text(&doc, "policy[0]#profile[0]", value),
        ] {
            assert!(v.admissible(&doc, &ev).is_ok(), "{ev:?}");
            assert_eq!(gated(&doc, &ev, Some(&v)), plain(&doc, &ev), "{ev:?}");
        }
    }
}

#[test]
fn right_operand_never_refused() {
    let doc = base();
    let v = strict_gate(closed());
    // Values the closed operands do not allow: flagged, never refused.
    for ev in [
        text(&doc, &format!("{PURPOSE}#rightOperand[0]"), "foo:bar"),
        iri(&doc, &format!("{PURPOSE}#rightOperand[1]"), "ex:commercial"),
        iri(&doc, &format!("{PURPOSE}#rightOperand[1]"), "foo:bar"),
        text(&doc, &format!("{COUNT}#rightOperand[0]"), "ten"),
    ] {
        assert!(v.admissible(&doc, &ev).is_ok(), "{ev:?}");
        let after = gated(&doc, &ev, Some(&v)).unwrap();
        assert_eq!(Ok(after.clone()), plain(&doc, &ev));
        // And the validator says so afterwards.
        assert_eq!(v.validate(&after).verdict, Verdict::Violations, "{ev:?}");
    }
}

#[test]
fn left_operand_change_never_refused_for_dependents() {
    // dataSubjectCount allows gteq and gt; the constraint keeps isAnyOf and
    // two IRIs. The left operand is admissible on its own, so it commits,
    // and the dependents are flagged.
    let doc = base();
    let v = strict_gate(closed());
    let ev = text(
        &doc,
        &format!("{PURPOSE}#leftOperand"),
        "ex:dataSubjectCount",
    );
    let after = gated(&doc, &ev, Some(&v)).unwrap();
    let r = v.validate(&after);
    assert_eq!(r.verdict, Verdict::Violations);
    assert!(
        r.findings
            .iter()
            .any(|f| f.rule == RuleId::ProseOperatorNotAllowed),
        "{:?}",
        r.findings
    );
}

#[test]
fn an_already_violating_document_stays_editable() {
    let doc = read_model(&strict_text("violating.json")).unwrap();
    let v = strict_gate(closed());
    assert_eq!(v.validate(&doc).verdict, Verdict::Violations);
    // Fixing one violation commits, while the others stay.
    let ev = text(&doc, "policy[0].permission[1].action[0]#name", "ex:train");
    let after = gated(&doc, &ev, Some(&v)).unwrap();
    assert_eq!(v.validate(&after).verdict, Verdict::Violations);
    // And an unrelated admissible edit commits too.
    let ev = text(&doc, "policy[0]#uid", "urn:policy:renamed");
    assert!(gated(&doc, &ev, Some(&v)).is_ok());
}

fn strict_text(name: &str) -> String {
    std::fs::read_to_string(format!("{}/strict/{name}", tests_dir())).unwrap()
}

#[test]
fn nothing_refused_while_incomplete() {
    let doc = incomplete();
    let v = strict_gate(closed());
    for ev in [
        text(&doc, ACTION, "foo:bar"),
        text(&doc, ACTION, "ex:researchPurpose"),
        choice(&doc, &format!("{COUNT}#operator"), "eq"),
        choice(&doc, "policy[0]#conflict", "foo:x"),
        text(&doc, "policy[0]#kind", "ex:fineTune"),
    ] {
        assert!(v.admissible(&doc, &ev).is_ok(), "{ev:?}");
        assert_eq!(gated(&doc, &ev, Some(&v)), plain(&doc, &ev), "{ev:?}");
    }
    // A loaded profile that builds on one that is not loaded: incomplete too.
    let w = wording_of(&["research-imports.jsonld", "research-closed.jsonld"]);
    let v = strict_gate(w);
    let doc = base();
    let ev = text(&doc, ACTION, "foo:bar");
    assert!(v.admissible(&doc, &ev).is_ok());
}

#[test]
fn advisory_and_flag_never_refuse() {
    let doc = base();
    let advisory = Validator::new(Arc::new(closed()), Options::advisory());
    let strict = strict_gate(closed());
    let ev = text(&doc, ACTION, "foo:bar");
    assert!(advisory.admissible(&doc, &ev).is_ok());
    assert_eq!(gated(&doc, &ev, Some(&advisory)), plain(&doc, &ev));
    // Flag is the default and hands the reducer no gate.
    assert_eq!(StrictEdit::default(), StrictEdit::Flag);
    assert!(StrictEdit::Flag.gate(Some(&strict)).is_none());
    assert_eq!(StrictEdit::RefuseTerms.gate(Some(&strict)), Some(&strict));
    assert!(StrictEdit::RefuseTerms.gate(None).is_none());
    // A level lowered by hand is not refused either.
    let lowered = Validator::new(
        Arc::new(closed()),
        Options::strict().level(RuleId::ProfileUnknownTerm, Level::Warning),
    );
    assert!(lowered.admissible(&doc, &ev).is_ok());
}

/// Every node of `doc`, depth first.
fn nodes(doc: &EditDoc) -> Vec<NodePath> {
    let mut out = Vec::new();
    let mut todo: Vec<NodePath> = (0..doc.policies.len())
        .rev()
        .map(NodePath::policy)
        .collect();
    while let Some(p) = todo.pop() {
        let mut kids = Vec::new();
        let steps: Vec<fn(usize) -> Step> = vec![
            Step::Action,
            Step::Refinement,
            Step::Constraint,
            Step::Child,
            |i| Step::Entity(EntityRole::Assigner, i),
            |i| Step::Entity(EntityRole::Assignee, i),
            |i| Step::Entity(EntityRole::Target, i),
            |i| Step::Rule(RuleList::Permission, i),
            |i| Step::Rule(RuleList::Prohibition, i),
            |i| Step::Rule(RuleList::Obligation, i),
            |i| Step::Rule(RuleList::Duty, i),
            |i| Step::Rule(RuleList::Remedy, i),
            |i| Step::Rule(RuleList::Consequence, i),
        ];
        for step in steps {
            let mut i = 0;
            while doc.node(&p.child(step(i))).is_some() {
                kids.push(p.child(step(i)));
                i += 1;
            }
        }
        out.push(p);
        todo.extend(kids.into_iter().rev());
    }
    out
}

const LIST_KINDS: &[ListKind] = &[
    ListKind::Rules(RuleList::Permission),
    ListKind::Rules(RuleList::Prohibition),
    ListKind::Rules(RuleList::Obligation),
    ListKind::Rules(RuleList::Duty),
    ListKind::Rules(RuleList::Remedy),
    ListKind::Rules(RuleList::Consequence),
    ListKind::Entities(EntityRole::Assigner),
    ListKind::Entities(EntityRole::Assignee),
    ListKind::Entities(EntityRole::Target),
    ListKind::Actions,
    ListKind::Refinements,
    ListKind::Constraints,
    ListKind::Children,
    ListKind::RightOperand,
    ListKind::Profile,
    ListKind::InheritFrom,
];

/// Every structural event that can be made on `doc`: an add at each end of
/// every list, a removal and a move of its first item, a group and an
/// ungroup of each constraint, and a change of each rule's kind.
fn structural_events(doc: &EditDoc) -> Vec<EditEvent> {
    let mut out = vec![add_ev(
        doc,
        ListPath::policies(),
        doc.policies.len(),
        NewItem::Default,
    )];
    for p in nodes(doc) {
        let id = doc.node(&p).unwrap().id();
        for kind in LIST_KINDS {
            let list = ListPath::of(&p, *kind);
            if doc.list_owner_id(&list).is_none() {
                continue;
            }
            let len = list_len(doc, &list);
            out.push(add_ev(doc, list.clone(), len, NewItem::Default));
            out.push(add_ev(
                doc,
                list.clone(),
                0,
                NewItem::Logical(LogicalOp::And),
            ));
            if len > 0 {
                out.push(EditEvent::Remove {
                    expect: item_id(doc, &list, 0),
                    list: list.clone(),
                    index: 0,
                });
            }
            if len > 1 {
                out.push(EditEvent::Move {
                    expect: item_id(doc, &list, 0),
                    list: list.clone(),
                    from: 0,
                    to: len - 1,
                });
            }
        }
        if let Some(NodeRef::Constraint(c)) = doc.node(&p) {
            out.push(EditEvent::Wrap {
                constraint: p.clone(),
                expect: id,
                op: LogicalOp::Or,
            });
            if matches!(c, ConstraintNode::Logical(_)) {
                out.push(EditEvent::Unwrap {
                    constraint: p.clone(),
                    expect: id,
                });
            }
        }
        if let Some(NodeRef::Rule(_)) = doc.node(&p) {
            for to in [
                RuleList::Permission,
                RuleList::Prohibition,
                RuleList::Obligation,
            ] {
                out.push(EditEvent::ChangeRuleKind {
                    rule: p.clone(),
                    expect: id,
                    to,
                });
            }
        }
    }
    out
}

/// Every slot and choice of `doc` that sentences show, with its owner.
fn slots_of(doc: &EditDoc) -> Vec<(SlotPath, NodeId, bool)> {
    fn seg(segs: &[Segment], out: &mut Vec<(SlotPath, NodeId, bool)>) {
        for s in segs {
            match s {
                Segment::Slot(s) => out.push((s.path.clone(), s.owner, false)),
                Segment::Choice(c) => out.push((c.path.clone(), c.owner, true)),
                Segment::List(l) => {
                    for i in &l.items {
                        out.push((i.slot.path.clone(), i.slot.owner, false));
                        seg(&i.extra, out);
                    }
                }
                _ => {}
            }
        }
    }
    fn conds(c: &ConditionList, out: &mut Vec<(SlotPath, NodeId, bool)>) {
        for i in &c.items {
            seg(&i.sentence.segments, out);
            if let Some(k) = &i.children {
                conds(k, out);
            }
        }
    }
    fn rule(r: &RuleProse, out: &mut Vec<(SlotPath, NodeId, bool)>) {
        seg(&r.sentence.segments, out);
        conds(&r.conditions, out);
        for g in &r.refinements {
            conds(&g.conditions, out);
        }
        for f in &r.follow_ups {
            f.rules.iter().for_each(|c| rule(c, out));
        }
    }
    let mut out = Vec::new();
    for p in sentences(doc) {
        seg(&p.heading.segments, &mut out);
        seg(&p.intro.segments, &mut out);
        for n in &p.notes {
            seg(&n.segments, &mut out);
        }
        for g in &p.refinements {
            conds(&g.conditions, &mut out);
        }
        p.rules.iter().for_each(|r| rule(r, &mut out));
    }
    out
}

/// Every slot event a battery of values makes on `doc`, including stale
/// and locked ones.
fn slot_events(doc: &EditDoc) -> Vec<EditEvent> {
    let mut out = Vec::new();
    for (slot, owner, is_choice) in slots_of(doc) {
        for value in ["", "x", "foo:bar", "ex:distil", "use", "eq", "and"] {
            for expect in [owner, NodeId(owner.0 + 1000)] {
                let value = value.to_string();
                let slot = slot.clone();
                out.push(if is_choice {
                    EditEvent::SetChoice {
                        slot,
                        expect,
                        value,
                    }
                } else {
                    EditEvent::SetText {
                        slot,
                        expect,
                        value,
                    }
                });
            }
        }
        if matches!(slot.field, Field::RightOperand(_)) {
            out.push(EditEvent::SetIri {
                slot,
                expect: owner,
                value: "foo:bar".into(),
            });
        }
    }
    out
}

/// The documents the equivalence tests run over: every policy of the
/// corpus that reads, and [`common::rich`].
fn docs() -> Vec<(String, EditDoc)> {
    let mut out: Vec<(String, EditDoc)> = policy_corpus()
        .into_iter()
        .filter_map(|(name, text)| read_model(&text).ok().map(|d| (name, d)))
        .collect();
    out.push(("rich".into(), rich()));
    out.push(("base".into(), base()));
    out
}

#[test]
fn gate_none_identical_to_apply() {
    let advisory = Validator::new(Arc::new(closed()), Options::advisory());
    let mut checked = 0;
    for (name, doc) in docs() {
        let mut events = structural_events(&doc);
        events.extend(slot_events(&doc));
        for ev in events {
            let want = plain(&doc, &ev);
            assert_eq!(gated(&doc, &ev, None), want, "{name}: {ev:?}");
            // Advisory never refuses.
            assert_eq!(gated(&doc, &ev, Some(&advisory)), want, "{name}: {ev:?}");
            checked += 1;
        }
    }
    assert!(checked > 1000, "{checked}");
}

#[test]
fn structural_events_never_refused() {
    let v = strict_gate(closed());
    for (name, doc) in docs() {
        for ev in structural_events(&doc) {
            assert!(v.admissible(&doc, &ev).is_ok(), "{name}: {ev:?}");
            assert_eq!(
                gated(&doc, &ev, Some(&v)),
                plain(&doc, &ev),
                "{name}: {ev:?}"
            );
        }
    }
}

#[test]
fn a_strict_refusal_comes_only_after_the_reducer_accepts() {
    // Over the whole battery: the strict gate either agrees with apply, or
    // refuses an event apply accepts, and only on a refusable rule at
    // error level, on the event's own slot.
    let v = strict_gate(closed());
    let mut refusals = 0;
    for (name, doc) in docs() {
        for ev in slot_events(&doc) {
            let want = plain(&doc, &ev);
            match gated(&doc, &ev, Some(&v)) {
                Err(EditError::OutsideProfile(f)) => {
                    assert!(want.is_ok(), "{name}: {ev:?}: {want:?}");
                    assert!(f.rule.info().refusable, "{name}: {}", f.rule);
                    assert_eq!(f.severity, Severity::Error);
                    assert!(!f.gap);
                    let (EditEvent::SetText { slot, .. } | EditEvent::SetChoice { slot, .. }) = &ev
                    else {
                        panic!("{name}: refused {ev:?}");
                    };
                    assert_eq!(f.target, IssueTarget::Slot(slot.clone()));
                    assert!(!matches!(slot.field, Field::RightOperand(_)));
                    refusals += 1;
                }
                got => assert_eq!(got, want, "{name}: {ev:?}"),
            }
        }
    }
    assert!(refusals > 0);
}

#[test]
fn apply_all_atomic_on_failure() {
    let doc = base();
    let v = strict_gate(closed());
    let rules = EditRules::default();
    let ok = [
        text(&doc, ACTION, "ex:train"),
        text(&doc, "policy[0]#uid", "urn:policy:t"),
    ];
    let mut d = doc.clone();
    d.apply_all(&ok, &rules, Some(&v)).unwrap();
    let mut want = doc.clone();
    for ev in &ok {
        want.apply(ev, &rules).unwrap();
    }
    assert_eq!(d, want);

    // A refusal at the third event leaves the document as it was.
    let mut evs = ok.to_vec();
    evs.push(text(&doc, ACTION, "foo:bar"));
    let mut d = doc.clone();
    let (at, e) = d.apply_all(&evs, &rules, Some(&v)).unwrap_err();
    assert_eq!(at, 2);
    assert!(matches!(e, EditError::OutsideProfile(_)));
    assert_eq!(d, doc);

    // So does a stale second event, without a gate too.
    let evs = [
        text(&doc, ACTION, "ex:train"),
        EditEvent::SetText {
            slot: sp("policy[0]#uid"),
            expect: NodeId(9999),
            value: "x".into(),
        },
    ];
    let mut d = doc.clone();
    let (at, e) = d.apply_all(&evs, &rules, None).unwrap_err();
    assert_eq!(at, 1);
    assert!(matches!(e, EditError::Stale { .. }));
    assert_eq!(d, doc);

    // Each event sees the document the ones before it made.
    let list = ListPath::of(&NodePath::policy(0), ListKind::Profile);
    let evs = [
        add_ev(&doc, list, 1, NewItem::Default),
        text(
            &doc,
            "policy[0]#profile[1]",
            "https://example.org/odrl/profile/other",
        ),
    ];
    let mut d = doc.clone();
    d.apply_all(&evs, &rules, Some(&v)).unwrap();
    assert_eq!(
        write_jsonld(&d)["profile"],
        json!([RESEARCH, "https://example.org/odrl/profile/other"])
    );
    // No events: nothing changes.
    let mut d = doc.clone();
    d.apply_all(&[], &rules, Some(&v)).unwrap();
    assert_eq!(d, doc);
}

#[test]
fn stale_fix_refused() {
    // ex is not bound: the fix writes the full IRI.
    let mut v = base_value();
    v["@context"] = json!("http://www.w3.org/ns/odrl.jsonld");
    v["permission"][0]["constraint"] = json!([]);
    v["permission"][0]["action"] = json!("ex:train");
    let doc = read_model(&v.to_string()).unwrap();
    let gate = strict_gate(closed());
    let report = gate.validate(&doc);
    let fix = report
        .findings
        .iter()
        .find(|f| f.rule == RuleId::ProfileUnboundPrefix)
        .and_then(|f| f.fixes.first())
        .cloned()
        .expect("a fix");
    // Applied to the document it was made for, through the gate: one step.
    let mut d = doc.clone();
    d.apply_all(&fix.events, &EditRules::default(), Some(&gate))
        .unwrap();
    assert!(
        gate.validate(&d).conforms(),
        "{:?}",
        gate.validate(&d).findings
    );

    // The action is replaced: the fix is stale, and nothing changes.
    let rules = EditRules::default();
    let actions = ListPath::of(&np("policy[0].permission[0]"), ListKind::Actions);
    let mut moved = doc.clone();
    moved
        .apply(
            &add_ev(&moved, actions.clone(), 1, NewItem::Default),
            &rules,
        )
        .unwrap();
    let first = item_id(&moved, &actions, 0);
    moved
        .apply(
            &EditEvent::Remove {
                list: actions,
                index: 0,
                expect: first,
            },
            &rules,
        )
        .unwrap();
    let before = moved.clone();
    let (at, e) = moved
        .apply_all(&fix.events, &rules, Some(&gate))
        .unwrap_err();
    assert_eq!(at, 0);
    assert!(matches!(e, EditError::Stale { .. }), "{e:?}");
    assert_eq!(moved, before);
}

#[test]
fn error_issue_targets_slot() {
    let doc = base();
    let v = strict_gate(closed());
    let ev = text(&doc, ACTION, "foo:bar");
    let e = gated(&doc, &ev, Some(&v)).unwrap_err();
    let issue = e.issue(&ev).expect("an issue");
    assert_eq!(issue.target, IssueTarget::Slot(sp(ACTION)));
    assert_eq!(issue.severity, Severity::Error);
    let EditError::OutsideProfile(f) = &e else {
        panic!("{e:?}")
    };
    assert_eq!(issue.message, f.message);
    assert!(issue.message.contains("foo:bar"), "{}", issue.message);
    assert!(e.to_string().contains(&f.message), "{e}");
    // Only a refusal is placed; the reducer's own errors are not.
    let stale = EditEvent::SetText {
        slot: sp(ACTION),
        expect: NodeId(9999),
        value: "x".into(),
    };
    let e = gated(&doc, &stale, Some(&v)).unwrap_err();
    assert_eq!(e.issue(&stale), None);
}

#[test]
fn a_refusal_names_its_finding() {
    let doc = base();
    let v = strict_gate(closed());
    let ev = choice(&doc, &format!("{COUNT}#operator"), "eq");
    let f = v.admissible(&doc, &ev).unwrap_err();
    assert_eq!(f.rule, RuleId::ProseOperatorNotAllowed);
    assert_eq!(f.node, Some(doc.node(&np(COUNT)).unwrap().id()));
    let expected = f.expected.as_ref().expect("what is allowed");
    assert!(expected.closed);
    assert_eq!(
        expected.operators,
        [
            "http://www.w3.org/ns/odrl/2/gteq",
            "http://www.w3.org/ns/odrl/2/gt"
        ]
    );
}

#[test]
fn hostile_events_do_not_panic() {
    let v = strict_gate(closed());
    let doc = base();
    let long = "x:".repeat(50_000);
    for value in [long.as_str(), ":", "::", "_:b0", "http://", "\u{0}", "ex:"] {
        for slot in [
            ACTION.to_string(),
            format!("{PURPOSE}#leftOperand"),
            format!("{PURPOSE}#unit"),
            "policy[0]#kind".to_string(),
        ] {
            let _ = v.admissible(&doc, &text(&doc, &slot, value));
            let _ = v.admissible(&doc, &choice(&doc, &slot, value));
        }
        let _ = v.admissible(&doc, &choice(&doc, &format!("{COUNT}#operator"), value));
        let _ = v.admissible(&doc, &choice(&doc, "policy[0]#conflict", value));
    }
    // An event on a path that is not there is admissible: apply refuses it.
    let ev = EditEvent::SetText {
        slot: sp("policy[9].permission[0].action[0]#name"),
        expect: NodeId(1),
        value: "foo:bar".into(),
    };
    assert!(v.admissible(&doc, &ev).is_ok());
}

#[test]
fn the_gate_follows_the_ruleset_pin_and_odrl_vocabulary() {
    let doc = base();
    // A term outside ODRL core is refused under the core vocabulary only.
    let common = strict_gate(closed());
    let core = Validator::new(
        Arc::new(closed()),
        Options::strict().odrl_vocabulary(prose_core::profile::OdrlVocab::Core),
    );
    let ev = text(&doc, ACTION, "anonymize");
    assert!(common.admissible(&doc, &ev).is_ok());
    assert_eq!(
        core.admissible(&doc, &ev).unwrap_err().rule,
        RuleId::ProfileUnknownTerm
    );
}

#[test]
fn a_ruleset_pinned_before_a_rule_does_not_refuse_by_it() {
    // A rule the pinned ruleset does not have is off, so it refuses nothing.
    let doc = base();
    let ev = text(&doc, ACTION, "foo:bar");
    assert!(strict_gate(closed()).admissible(&doc, &ev).is_err());
    let pinned = Validator::new(Arc::new(closed()), Options::strict().ruleset(0));
    assert!(pinned.admissible(&doc, &ev).is_ok());
    assert_eq!(gated(&doc, &ev, Some(&pinned)), plain(&doc, &ev));
}

const UNITS: &str = "https://example.org/odrl/profile/units";
const STORAGE: &str = "policy[0].permission[0].constraint[2]";

/// research-closed, and a profile whose `storage` operand suggests the
/// unit gigabyte or, when `closed`, allows only that.
fn with_units(is_closed: bool) -> Wording {
    let units = prose_core::profile::ProfileVocab::from_value(
        &json!({
            "@context": {"odrl": "http://www.w3.org/ns/odrl/2/",
                         "prose": "https://ds-labs-org.github.io/ds-prose-rs/profile#",
                         "owl": "http://www.w3.org/2002/07/owl#",
                         "u": "https://example.org/odrl/profile/units/"},
            "@graph": [{"@id": UNITS, "@type": "owl:Ontology"},
                       {"@id": "u:storage", "@type": "odrl:LeftOperand",
                        "prose:unit": [{"@id": "u:gigabyte"}], "prose:closed": is_closed}]
        }),
        &prose_core::profile::ProfileLimits::default(),
    )
    .unwrap();
    closed().with_profile(units)
}

/// [`base`], declaring the units profile too, with a third constraint on
/// `u:storage` in gigabytes.
fn base_with_units() -> EditDoc {
    let mut v = base_value();
    v["@context"][1]["u"] = json!("https://example.org/odrl/profile/units/");
    v["profile"] = json!([RESEARCH, UNITS]);
    v["permission"][0]["constraint"]
        .as_array_mut()
        .unwrap()
        .push(json!({"leftOperand": "u:storage", "operator": "lteq",
                     "rightOperand": 5, "unit": "u:gigabyte"}));
    read_model(&v.to_string()).unwrap()
}

#[test]
fn a_unit_outside_a_closed_operand_is_refused() {
    let doc = base_with_units();
    let unit = format!("{STORAGE}#unit");
    let megabyte = "http://dbpedia.org/resource/Megabyte";
    let closed = strict_gate(with_units(true));
    assert!(
        closed.validate(&doc).conforms(),
        "{:?}",
        closed.validate(&doc).findings
    );
    let ev = text(&doc, &unit, megabyte);
    assert_eq!(
        refused_by(gated(&doc, &ev, Some(&closed)).map(|_| ())),
        "prose.unit-not-allowed"
    );
    // The allowed unit, written either way, commits.
    for value in [
        "u:gigabyte",
        "https://example.org/odrl/profile/units/gigabyte",
    ] {
        let ev = text(&doc, &unit, value);
        assert_eq!(gated(&doc, &ev, Some(&closed)), plain(&doc, &ev), "{value}");
    }
    // On an open operand the unit is only a suggestion: flagged as Info.
    let open = strict_gate(with_units(false));
    let after = gated(&doc, &ev, Some(&open)).unwrap();
    assert!(
        open.validate(&after)
            .findings
            .iter()
            .any(|f| f.rule == RuleId::ProseUnitNotAllowed && f.severity == Severity::Info)
    );
}

#[test]
fn a_term_of_a_profile_the_policy_does_not_declare_is_flagged_not_refused() {
    // other.jsonld is loaded, but the policy declares only the research
    // profile: profile.not-declared is an error with a fix, never a refusal.
    let w = wording_of(&["research-closed.jsonld", "other.jsonld"]);
    let v = strict_gate(w);
    let doc = base();
    let ev = text(
        &doc,
        ACTION,
        "https://example.com/odrl/profile/other/archiveCopy",
    );
    assert!(v.admissible(&doc, &ev).is_ok());
    let after = gated(&doc, &ev, Some(&v)).unwrap();
    let r = v.validate(&after);
    let f = r
        .findings
        .iter()
        .find(|f| f.rule == RuleId::ProfileNotDeclared)
        .expect("not declared");
    assert_eq!(f.severity, Severity::Error);
    assert!(!f.fixes.is_empty());
}

#[test]
fn advisory_with_a_level_raised_by_hand_never_refuses() {
    // Advisory mode never refuses, whatever level a rule is given.
    let doc = base();
    let raised = Validator::new(
        Arc::new(closed()),
        Options::advisory()
            .level(RuleId::ProfileUnknownTerm, Level::Error)
            .level(RuleId::ProfileUndeclaredInNamespace, Level::Error),
    );
    for value in ["foo:bar", "ex:distil"] {
        let ev = text(&doc, ACTION, value);
        let after = gated(&doc, &ev, Some(&raised)).unwrap();
        assert!(
            raised
                .validate(&after)
                .findings
                .iter()
                .any(|f| f.severity == Severity::Error),
            "{value}"
        );
    }
}

#[test]
fn the_gate_refuses_exactly_what_validation_reports_at_the_slot() {
    // Soundness: a refusal is a finding the validator reports, at the same
    // slot, once the event is applied; and, while nothing is a gap, the
    // validator reports no refusable error at a slot the gate let through.
    let mut docs = docs();
    docs.push(("units".into(), base_with_units()));
    docs.push(("incomplete".into(), incomplete()));
    let wordings = [
        ("closed", closed()),
        (
            "closed+other",
            wording_of(&["research-closed.jsonld", "other.jsonld"]),
        ),
        ("units-closed", with_units(true)),
        ("units-open", with_units(false)),
        ("none", Wording::new()),
    ];
    let values = [
        "foo:bar",
        "ex:distil",
        "ex:train",
        "ex:researchPurpose",
        "ex:fineTune",
        "use",
        "eq",
        "gteq",
        "perm",
        "Offer",
        "u:gigabyte",
        "http://dbpedia.org/resource/Megabyte",
        "https://example.com/odrl/profile/other/archiveCopy",
    ];
    let (mut refused, mut admitted) = (0, 0);
    for (wname, w) in wordings {
        let v = strict_gate(w);
        for (name, doc) in &docs {
            for (slot, owner, is_choice) in slots_of(doc) {
                for value in values {
                    let value = value.to_string();
                    let ev = if is_choice {
                        EditEvent::SetChoice {
                            slot: slot.clone(),
                            expect: owner,
                            value,
                        }
                    } else {
                        EditEvent::SetText {
                            slot: slot.clone(),
                            expect: owner,
                            value,
                        }
                    };
                    let Ok(after) = plain(doc, &ev) else {
                        continue;
                    };
                    let r = v.validate(&after);
                    let here = IssueTarget::Slot(slot.clone());
                    let at_slot = |f: &&Finding| {
                        f.target == here
                            && f.severity == Severity::Error
                            && !f.gap
                            && f.rule.info().refusable
                    };
                    match v.admissible(doc, &ev) {
                        Err(f) => {
                            assert!(
                                r.findings
                                    .iter()
                                    .any(|g| g.rule == f.rule && g.target == f.target),
                                "{wname}/{name}: {ev:?} refused for {} ({}), not reported after: {:?}",
                                f.rule,
                                f.message,
                                r.findings
                            );
                            refused += 1;
                        }
                        Ok(()) => {
                            // Right operands are flagged, never refused.
                            if r.gaps.is_empty() && !matches!(slot.field, Field::RightOperand(_)) {
                                assert!(
                                    !r.findings.iter().any(|f| at_slot(&f)),
                                    "{wname}/{name}: {ev:?} admitted, reported after: {:?}",
                                    r.findings.iter().filter(at_slot).collect::<Vec<_>>()
                                );
                            }
                            admitted += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(refused > 100 && admitted > 100, "{refused} {admitted}");
}

/// R2: a closed list asks about every entry of one slot. The batch call
/// prepares the document once and must answer exactly as one call per value.
#[test]
fn admissible_values_agrees_with_admissible_per_value() {
    let v = strict_gate(closed());
    let values = [
        "",
        "use",
        "read",
        "ex:train",
        "ex:fineTune",
        "foo:bar",
        "ex:distil",
        "ex:researchPurpose",
        "gteq",
        "isAnyOf",
        "perm",
        "http://",
        "ex:",
    ];
    let slots = [
        ACTION.to_string(),
        format!("{PURPOSE}#leftOperand"),
        format!("{PURPOSE}#operator"),
        format!("{COUNT}#operator"),
        format!("{COUNT}#unit"),
        "policy[0]#kind".to_string(),
        "policy[0]#conflict".to_string(),
        // Not vocabulary slots, and one that is not there.
        format!("{COUNT}#rightOperand[0]"),
        "policy[9].permission[0].action[0]#name".to_string(),
    ];
    for doc in [base(), incomplete()] {
        for slot in &slots {
            let at = sp(slot);
            let batch = v.admissible_values(&doc, &at, &values);
            assert_eq!(batch.len(), values.len(), "{slot}");
            for (value, got) in values.iter().zip(&batch) {
                let one = v.admissible(&doc, &text(&doc, slot, value));
                assert_eq!(got.is_ok(), one.is_ok(), "{slot} <- {value:?}");
                if let (Err(a), Err(b)) = (got, &one) {
                    assert_eq!(a.rule, b.rule, "{slot} <- {value:?}");
                }
            }
        }
    }
    // Advisory: everything is admitted.
    let advisory = Validator::new(Arc::new(closed()), Options::advisory());
    let all = advisory.admissible_values(&base(), &sp(ACTION), &["foo:bar", "use"]);
    assert!(all.iter().all(Result::is_ok));
}

/// R2: the finding a document already has on the slot is not an answer for
/// the values asked about, and one value's finding is not another's.
#[test]
fn admissible_values_ignores_what_the_slot_already_violates() {
    let v = strict_gate(closed());
    let base = base();
    let doc = plain(&base, &text(&base, ACTION, "foo:bar")).unwrap();
    assert_eq!(v.validate(&doc).verdict, Verdict::Violations);
    let got = v.admissible_values(&doc, &sp(ACTION), &["use", "foo:bar", "ex:train", "use"]);
    let ok: Vec<bool> = got.iter().map(Result::is_ok).collect();
    assert_eq!(ok, [true, false, true, true]);
    for (value, got) in ["use", "foo:bar", "ex:train", "use"].iter().zip(&got) {
        let one = v.admissible(&doc, &text(&doc, ACTION, value));
        assert_eq!(got.is_ok(), one.is_ok(), "{value}");
    }
}
