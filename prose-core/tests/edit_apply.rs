mod common;
use common::*;
use prose_core::edit::*;

fn rules() -> EditRules {
    EditRules::default()
}

#[test]
fn add_then_remove_is_the_identity_for_every_list_kind() {
    let lists = [
        "policies",
        "policy[0]@permission",
        "policy[0]@prohibition",
        "policy[0]@obligation",
        "policy[0]@assigner",
        "policy[0]@assignee",
        "policy[0]@target",
        "policy[0]@action",
        "policy[0]@profile",
        "policy[0]@inheritFrom",
        "policy[0].action[0]@refinement",
        "policy[0].target[0]@refinement",
        "policy[0].permission[0]@assigner",
        "policy[0].permission[0]@assignee",
        "policy[0].permission[0]@target",
        "policy[0].permission[0]@action",
        "policy[0].permission[0]@constraint",
        "policy[0].permission[0]@duty",
        "policy[0].prohibition[0]@remedy",
        "policy[0].permission[0].duty[0]@consequence",
        "policy[0].permission[0].constraint[0]@child",
        "policy[0].permission[0].constraint[0].child[0]@rightOperand",
    ];
    for l in lists {
        let list = lp(l);
        let mut doc = rich();
        let before = doc.normalized();
        let len = list_len(&doc, &list);
        doc.apply(&add_ev(&doc, list.clone(), len, NewItem::Default), &rules())
            .unwrap_or_else(|e| panic!("add {l}: {e}"));
        assert_eq!(list_len(&doc, &list), len + 1, "{l}");
        assert_ne!(doc.normalized(), before, "{l}");
        let expect = item_id(&doc, &list, len);
        doc.apply(
            &EditEvent::Remove {
                list: list.clone(),
                index: len,
                expect,
            },
            &rules(),
        )
        .unwrap_or_else(|e| panic!("remove {l}: {e}"));
        assert_eq!(doc.normalized(), before, "{l}");
    }
}

#[test]
fn add_in_the_middle_and_the_new_node_is_numbered_without_disturbing_others() {
    let mut doc = rich();
    let old_id = id_of(&doc, "policy[0].permission[0]");
    doc.apply(
        &add_ev(&doc, lp("policy[0]@permission"), 0, NewItem::Default),
        &rules(),
    )
    .unwrap();
    let new_rule = id_of(&doc, "policy[0].permission[0]");
    let moved = id_of(&doc, "policy[0].permission[1]");
    assert_eq!(moved, old_id);
    assert_ne!(new_rule, old_id);
    assert_ne!(new_rule, NodeId(0));
    // Every id is unique.
    let mut ids = Vec::new();
    fn walk(r: &RuleNode, ids: &mut Vec<NodeId>) {
        ids.push(r.id);
        r.action.iter().for_each(|a| ids.push(a.id));
        r.duty
            .iter()
            .chain(&r.remedy)
            .chain(&r.consequence)
            .for_each(|c| walk(c, ids));
    }
    for r in &doc.policies[0].permission {
        walk(r, &mut ids);
    }
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n);
    assert!(ids.iter().all(|i| i.0 != 0));
}

#[test]
fn stale_and_missing_paths_leave_the_document_unchanged() {
    let mut doc = rich();
    let before = doc.clone();
    let slot: SlotPath = "policy[0]#uid".parse().unwrap();
    let id = doc.policies[0].id;
    let err = doc
        .apply(
            &EditEvent::SetText {
                slot: slot.clone(),
                expect: NodeId(id.0 + 99),
                value: "x".into(),
            },
            &rules(),
        )
        .unwrap_err();
    assert!(matches!(err, EditError::Stale { .. }), "{err}");
    assert_eq!(doc, before);

    let bad: SlotPath = "policy[7]#uid".parse().unwrap();
    let err = doc
        .apply(
            &EditEvent::SetText {
                slot: bad,
                expect: id,
                value: "x".into(),
            },
            &rules(),
        )
        .unwrap_err();
    assert!(matches!(err, EditError::NoSuchPath(_)), "{err}");
    assert_eq!(doc, before);

    // Wrong field for the node, index out of range, add past the end.
    for ev in [
        EditEvent::SetText {
            slot: "policy[0]#iri".parse().unwrap(),
            expect: id,
            value: "x".into(),
        },
        EditEvent::SetText {
            slot: "policy[0]#profile[9]".parse().unwrap(),
            expect: id,
            value: "x".into(),
        },
        add_ev(&doc, lp("policy[0]@permission"), 9, NewItem::Default),
        EditEvent::Remove {
            list: lp("policy[0]@permission"),
            index: 9,
            expect: id,
        },
        EditEvent::Move {
            list: lp("policy[0]@permission"),
            from: 0,
            to: 9,
            expect: id_of(&before, "policy[0].permission[0]"),
        },
    ] {
        let err = doc.apply(&ev, &rules()).unwrap_err();
        assert!(matches!(err, EditError::NoSuchPath(_)), "{ev:?}: {err}");
        assert_eq!(doc, before);
    }
    // A stale remove.
    let err = doc
        .apply(
            &EditEvent::Remove {
                list: lp("policy[0]@permission"),
                index: 0,
                expect: NodeId(9999),
            },
            &rules(),
        )
        .unwrap_err();
    assert!(matches!(err, EditError::Stale { .. }));
    assert_eq!(doc, before);
}

#[test]
fn limits_are_enforced_at_max_and_min() {
    let mut r = rules();
    r.policy_assigner = Limit::exactly(1);
    r.rule_action = Limit::exactly(1);
    r.consequence = Limit::at_most(1);
    let mut doc = rich();
    let before = doc.clone();
    // At max.
    let e = doc
        .apply(
            &add_ev(&doc, lp("policy[0]@assigner"), 1, NewItem::Default),
            &r,
        )
        .unwrap_err();
    assert!(matches!(e, EditError::NotAllowed(_)), "{e}");
    // At min.
    let e = doc
        .apply(
            &EditEvent::Remove {
                list: lp("policy[0]@assigner"),
                index: 0,
                expect: id_of(&doc, "policy[0].assigner[0]"),
            },
            &r,
        )
        .unwrap_err();
    assert!(matches!(e, EditError::NotAllowed(_)), "{e}");
    let e = doc
        .apply(
            &EditEvent::Remove {
                list: lp("policy[0].permission[0]@action"),
                index: 0,
                expect: id_of(&doc, "policy[0].permission[0].action[0]"),
            },
            &r,
        )
        .unwrap_err();
    assert!(matches!(e, EditError::NotAllowed(_)), "{e}");
    assert_eq!(doc, before);
    assert!(!r.can_add(&doc, &lp("policy[0].permission[0].duty[0]@consequence")));
    assert!(r.can_add(&doc, &lp("policy[0].permission[0]@constraint")));
    assert!(!r.can_remove(&doc, &lp("policy[0]@assigner")));
    assert!(r.can_remove(&doc, &lp("policy[0]@assignee")));
    // The reducer and the predicate agree.
    assert!(!r.can_add(&doc, &lp("policy[0]@assigner")));
}

#[test]
fn follow_ups_are_only_allowed_in_odrl_positions() {
    let mut doc = rich();
    let r = rules();
    for (list, ok) in [
        ("policy[0].permission[0]@duty", true),
        ("policy[0].permission[0]@remedy", false),
        ("policy[0].permission[0]@consequence", false),
        ("policy[0].prohibition[0]@remedy", true),
        ("policy[0].prohibition[0]@duty", false),
        ("policy[0].obligation[0]@consequence", true),
        ("policy[0].obligation[0]@duty", false),
        ("policy[0].permission[0].duty[0]@consequence", true),
        ("policy[0].permission[0].duty[0]@duty", false),
        (
            "policy[0].permission[0].duty[0].consequence[0]@consequence",
            true,
        ),
        (
            "policy[0].permission[0].duty[0].consequence[0]@remedy",
            false,
        ),
    ] {
        let l = lp(list);
        assert_eq!(r.can_add(&doc, &l), ok, "{list}");
        let res = doc.apply(&add_ev(&doc, l, 0, NewItem::Default), &r);
        assert_eq!(res.is_ok(), ok, "{list}: {res:?}");
        if !ok {
            assert!(matches!(res, Err(EditError::NotAllowed(_))));
        }
    }
    assert!(odrl_position(Some(RuleList::Permission), RuleList::Duty));
    assert!(!odrl_position(Some(RuleList::Prohibition), RuleList::Duty));
    assert!(odrl_position(None, RuleList::Obligation));
    assert!(!odrl_position(None, RuleList::Duty));
}

#[test]
fn locked_nodes_refuse_edits_but_can_be_removed_or_moved() {
    let v = serde_json::json!({
        "@type": "Set",
        "permission": [7, {"action": "use"}],
        "assignee": [{"x": 1}]
    });
    let mut doc = read_model_value(&v).unwrap();
    let r = rules();
    let locked_rule = id_of(&doc, "policy[0].permission[0]");
    let e = doc
        .apply(
            &add_ev(
                &doc,
                lp("policy[0].permission[0]@constraint"),
                0,
                NewItem::Default,
            ),
            &r,
        )
        .unwrap_err();
    assert!(matches!(e, EditError::Locked(_)), "{e}");
    assert!(!r.can_add(&doc, &lp("policy[0].permission[0]@constraint")));
    let e = doc
        .apply(
            &EditEvent::SetText {
                slot: "policy[0].assignee[0]#iri".parse().unwrap(),
                expect: id_of(&doc, "policy[0].assignee[0]"),
                value: "x".into(),
            },
            &r,
        )
        .unwrap_err();
    assert!(matches!(e, EditError::Locked(_)), "{e}");
    doc.apply(
        &EditEvent::Move {
            list: lp("policy[0]@permission"),
            from: 0,
            to: 1,
            expect: locked_rule,
        },
        &r,
    )
    .unwrap();
    assert_eq!(id_of(&doc, "policy[0].permission[1]"), locked_rule);
    doc.apply(
        &EditEvent::ChangeRuleKind {
            rule: np("policy[0].permission[1]"),
            expect: locked_rule,
            to: RuleList::Prohibition,
        },
        &r,
    )
    .unwrap();
    assert!(doc.policies[0].prohibition[0].locked.is_some());
    doc.apply(
        &EditEvent::Remove {
            list: lp("policy[0]@assignee"),
            index: 0,
            expect: id_of(&doc, "policy[0].assignee[0]"),
        },
        &r,
    )
    .unwrap();
    assert!(doc.policies[0].assignee.is_empty());
}

#[test]
fn new_nodes_carry_the_defaults_and_unique_uids() {
    let mut r = rules();
    r.defaults.uid_prefix = "urn:p:".into();
    r.defaults.policy_kind = "Offer".into();
    r.defaults.action = "display".into();
    r.defaults.left_operand = "dateTime".into();
    r.defaults.operator = "lt".into();
    let mut doc = EditDoc::new(vec![PolicyNode {
        kind: "Set".into(),
        uid: Some("urn:p:2".into()),
        ..PolicyNode::default()
    }]);
    let add_policy = |doc: &mut EditDoc| {
        let n = doc.policies.len();
        doc.apply(&add_ev(doc, ListPath::policies(), n, NewItem::Default), &r)
            .unwrap();
    };
    add_policy(&mut doc);
    add_policy(&mut doc);
    assert_eq!(doc.policies[1].uid.as_deref(), Some("urn:p:1"));
    assert_eq!(doc.policies[2].uid.as_deref(), Some("urn:p:3"));
    assert_eq!(doc.policies[1].kind, "Offer");
    doc.apply(
        &add_ev(&doc, lp("policy[0]@permission"), 0, NewItem::Default),
        &r,
    )
    .unwrap();
    let rule = &doc.policies[0].permission[0];
    assert_eq!(rule.action.len(), 1);
    assert_eq!(rule.action[0].name, "display");
    doc.apply(
        &add_ev(
            &doc,
            lp("policy[0].permission[0]@constraint"),
            0,
            NewItem::Default,
        ),
        &r,
    )
    .unwrap();
    match &doc.policies[0].permission[0].constraint[0] {
        ConstraintNode::Atomic(a) => {
            assert_eq!(a.left, "dateTime");
            assert_eq!(a.operator, "lt");
            assert_eq!(
                a.right,
                RightOperand::Values(vec![Literal::Str(String::new())])
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn minimums_are_filled_in_for_a_new_node() {
    let mut r = rules();
    r.policy_assigner = Limit::exactly(1);
    r.rule_assigner = Limit::exactly(1);
    r.rule_target = Limit::at_least(1);
    r.duty = Limit::at_least(1);
    r.consequence = Limit::at_least(1);
    let mut doc = EditDoc::new(vec![]);
    doc.apply(&add_ev(&doc, ListPath::policies(), 0, NewItem::Default), &r)
        .unwrap();
    assert_eq!(doc.policies[0].assigner.len(), 1);
    assert_eq!(doc.policies[0].assigner[0].iri.as_deref(), Some(""));
    doc.apply(
        &add_ev(&doc, lp("policy[0]@permission"), 0, NewItem::Default),
        &r,
    )
    .unwrap();
    let rule = &doc.policies[0].permission[0];
    assert_eq!(rule.assigner.len(), 1);
    assert_eq!(rule.target.len(), 1);
    // A duty is required under a permission, and a consequence under that.
    assert_eq!(rule.duty.len(), 1);
    assert_eq!(rule.duty[0].consequence.len(), 1);
    assert_eq!(rule.duty[0].assigner.len(), 1);
    // No remedy: a permission has none in ODRL.
    assert!(rule.remedy.is_empty());
    // Every node has an id.
    assert!(doc.policies[0].permission[0].duty[0].consequence[0].id.0 != 0);
}

#[test]
fn change_rule_kind_keeps_children_and_origin() {
    let mut doc = rich();
    let rule = np("policy[0].permission[0]");
    let id = id_of(&doc, "policy[0].permission[0]");
    let origin = doc.policies[0].permission[0].origin.clone();
    doc.apply(
        &EditEvent::ChangeRuleKind {
            rule: rule.clone(),
            expect: id,
            to: RuleList::Obligation,
        },
        &rules(),
    )
    .unwrap();
    assert!(doc.policies[0].permission.is_empty());
    let moved = doc.policies[0].obligation.last().unwrap();
    assert_eq!(moved.id, id);
    assert_eq!(moved.origin, origin);
    assert_eq!(moved.duty.len(), 1);
    assert_eq!(doc.policies[0].obligation.len(), 2);
    // Invalid targets and nested rules.
    let e = doc
        .apply(
            &EditEvent::ChangeRuleKind {
                rule: np("policy[0].obligation[1]"),
                expect: id,
                to: RuleList::Obligation,
            },
            &rules(),
        )
        .unwrap_err();
    assert!(matches!(e, EditError::Invalid(_)));
    let e = doc
        .apply(
            &EditEvent::ChangeRuleKind {
                rule: np("policy[0].obligation[1].duty[0]"),
                expect: id_of(&doc, "policy[0].obligation[1].duty[0]"),
                to: RuleList::Prohibition,
            },
            &rules(),
        )
        .unwrap_err();
    assert!(matches!(e, EditError::Invalid(_)));
    let mut r = rules();
    r.prohibition = Limit::NONE;
    let e = doc
        .apply(
            &EditEvent::ChangeRuleKind {
                rule: np("policy[0].obligation[1]"),
                expect: id,
                to: RuleList::Prohibition,
            },
            &r,
        )
        .unwrap_err();
    assert!(matches!(e, EditError::NotAllowed(_)));
    r = rules();
    r.allow_rule_kind_change = false;
    assert!(matches!(
        doc.apply(
            &EditEvent::ChangeRuleKind {
                rule: np("policy[0].obligation[1]"),
                expect: id,
                to: RuleList::Prohibition,
            },
            &r,
        ),
        Err(EditError::NotAllowed(_))
    ));
}

#[test]
fn wrap_unwrap_and_move() {
    let mut doc = rich();
    let c = np("policy[0].permission[0].constraint[0].child[0]");
    let atomic_id = id_of(&doc, "policy[0].permission[0].constraint[0].child[0]");
    doc.apply(
        &EditEvent::Wrap {
            constraint: c.clone(),
            expect: atomic_id,
            op: LogicalOp::Xone,
        },
        &rules(),
    )
    .unwrap();
    let wrapper = id_of(&doc, "policy[0].permission[0].constraint[0].child[0]");
    assert_ne!(wrapper, atomic_id);
    assert_eq!(
        id_of(
            &doc,
            "policy[0].permission[0].constraint[0].child[0].child[0]"
        ),
        atomic_id
    );
    let e = doc
        .apply(
            &EditEvent::Wrap {
                constraint: c.clone(),
                expect: wrapper,
                op: LogicalOp::And,
            },
            &{
                let mut r = rules();
                r.allow_logical = false;
                r
            },
        )
        .unwrap_err();
    assert!(matches!(e, EditError::NotAllowed(_)));
    doc.apply(
        &EditEvent::Unwrap {
            constraint: c.clone(),
            expect: wrapper,
        },
        &rules(),
    )
    .unwrap();
    assert_eq!(
        id_of(&doc, "policy[0].permission[0].constraint[0].child[0]"),
        atomic_id
    );
    // Unwrap of an atomic constraint, and of a group with two children, is invalid.
    let e = doc
        .apply(
            &EditEvent::Unwrap {
                constraint: c.clone(),
                expect: atomic_id,
            },
            &rules(),
        )
        .unwrap_err();
    assert!(matches!(e, EditError::Invalid(_)));

    // Move: remove at `from`, insert at `to`.
    let mut doc = rich();
    for _ in 0..2 {
        let n = doc.policies[0].profile.len();
        doc.apply(
            &add_ev(&doc, lp("policy[0]@profile"), n, NewItem::Default),
            &rules(),
        )
        .unwrap();
    }
    for (k, v) in ["b", "c"].iter().enumerate() {
        set_text(&mut doc, &format!("policy[0]#profile[{}]", k + 1), v).unwrap();
    }
    let owner = doc.policies[0].id;
    doc.apply(
        &EditEvent::Move {
            list: lp("policy[0]@profile"),
            from: 0,
            to: 2,
            expect: owner,
        },
        &rules(),
    )
    .unwrap();
    assert_eq!(doc.policies[0].profile, ["b", "c", "urn:p1"]);
    let mut r = rules();
    r.allow_reorder = false;
    assert!(matches!(
        doc.apply(
            &EditEvent::Move {
                list: lp("policy[0]@profile"),
                from: 0,
                to: 1,
                expect: owner,
            },
            &r,
        ),
        Err(EditError::NotAllowed(_))
    ));
}

#[test]
fn set_text_variants_store_text_verbatim() {
    let mut doc = rich();
    // Never trimmed.
    set_text(&mut doc, "policy[0]#uid", "  urn:x \u{a0}").unwrap();
    assert_eq!(doc.policies[0].uid.as_deref(), Some("  urn:x \u{a0}"));
    // An empty uid is Some(""), an empty part-of is None.
    set_text(&mut doc, "policy[0]#uid", "").unwrap();
    assert_eq!(doc.policies[0].uid.as_deref(), Some(""));
    set_text(&mut doc, "policy[0].target[0]#partOf", "urn:c").unwrap();
    assert_eq!(doc.policies[0].target[0].part_of.as_deref(), Some("urn:c"));
    set_text(&mut doc, "policy[0].target[0]#partOf", "").unwrap();
    assert_eq!(doc.policies[0].target[0].part_of, None);
    // A number stays a number while the text fits.
    set_text(
        &mut doc,
        "policy[0].action[0].refinement[0]#rightOperand[0]",
        "9",
    )
    .unwrap();
    set_text(&mut doc, "policy[0].action[0].refinement[0]#unit", "urn:u").unwrap();
    match &doc.policies[0].action[0].refinement[0] {
        ConstraintNode::Atomic(a) => {
            assert_eq!(a.right, RightOperand::Values(vec![Literal::Num(9.into())]));
            assert_eq!(a.unit.as_deref(), Some("urn:u"));
        }
        _ => unreachable!(),
    }
    set_text(
        &mut doc,
        "policy[0].action[0].refinement[0]#rightOperand[0]",
        "lots",
    )
    .unwrap();
    match &doc.policies[0].action[0].refinement[0] {
        ConstraintNode::Atomic(a) => {
            assert_eq!(
                a.right,
                RightOperand::Values(vec![Literal::Str("lots".into())])
            );
        }
        _ => unreachable!(),
    }
    set_text(
        &mut doc,
        "policy[0].action[0].refinement[0]#rightOperandReference",
        "urn:ref",
    )
    .unwrap();
    match &doc.policies[0].action[0].refinement[0] {
        ConstraintNode::Atomic(a) => assert_eq!(a.right, RightOperand::Reference("urn:ref".into())),
        _ => unreachable!(),
    }
    // Choices.
    let slot: SlotPath = "policy[0].permission[0].constraint[0]#logicalOp"
        .parse()
        .unwrap();
    let expect = id_of(&doc, "policy[0].permission[0].constraint[0]");
    let bad = doc.apply(
        &EditEvent::SetChoice {
            slot: slot.clone(),
            expect,
            value: "nand".into(),
        },
        &rules(),
    );
    assert!(matches!(bad, Err(EditError::Invalid(_))));
    doc.apply(
        &EditEvent::SetChoice {
            slot,
            expect,
            value: "odrl:xone".into(),
        },
        &rules(),
    )
    .unwrap();
    match &doc.policies[0].permission[0].constraint[0] {
        ConstraintNode::Logical(l) => assert_eq!(l.op, LogicalOp::Xone),
        _ => unreachable!(),
    }
}

#[test]
fn right_operand_limits_follow_the_operator() {
    let doc = rich();
    let r = rules();
    let set = lp("policy[0].permission[0].constraint[0].child[0]@rightOperand");
    assert_eq!(r.limit(&doc, &set), Limit::at_least(1));
    let scalar = lp("policy[0].action[0].refinement[0]@rightOperand");
    assert_eq!(
        r.limit(&doc, &scalar),
        Limit {
            min: 1,
            max: Some(1)
        }
    );
    assert!(r.can_add(&doc, &set));
    assert!(!r.can_add(&doc, &scalar));
    assert!(r.is_set_operator("odrl:isAnyOf"));
    assert!(!r.is_set_operator("eq"));
}

#[test]
fn focus_after_every_variant() {
    let r = rules();
    let mut doc = rich();
    let check = |doc: &EditDoc, ev: &EditEvent| focus_after(ev, doc);

    // SetText / SetChoice: none.
    let ev = EditEvent::SetText {
        slot: "policy[0]#uid".parse().unwrap(),
        expect: doc.policies[0].id,
        value: "x".into(),
    };
    doc.apply(&ev, &r).unwrap();
    assert_eq!(check(&doc, &ev), None);

    // Add: the new item's first slot.
    let cases = [
        ("policies", 1usize, "policy[1]#uid"),
        (
            "policy[0]@permission",
            1,
            "policy[0].permission[1].action[0]#name",
        ),
        ("policy[0]@assignee", 1, "policy[0].assignee[1]#iri"),
        ("policy[0]@action", 1, "policy[0].action[1]#name"),
        (
            "policy[0].action[0]@refinement",
            1,
            "policy[0].action[0].refinement[1]#leftOperand",
        ),
        (
            "policy[0].permission[0]@constraint",
            1,
            "policy[0].permission[0].constraint[1]#leftOperand",
        ),
        (
            "policy[0].permission[0].constraint[0].child[0]@rightOperand",
            2,
            "policy[0].permission[0].constraint[0].child[0]#rightOperand[2]",
        ),
        ("policy[0]@profile", 1, "policy[0]#profile[1]"),
        ("policy[0]@inheritFrom", 1, "policy[0]#inheritFrom[1]"),
    ];
    for (list, index, slot) in cases {
        let mut d = rich();
        let ev = add_ev(&d, lp(list), index, NewItem::Default);
        d.apply(&ev, &r).unwrap_or_else(|e| panic!("{list}: {e}"));
        assert_eq!(
            focus_after(&ev, &d),
            Some(FocusTarget::Slot(slot.parse().unwrap())),
            "{list}"
        );
    }
    // A logical group focuses its operator.
    let mut d = rich();
    let ev = add_ev(
        &d,
        lp("policy[0].permission[0]@constraint"),
        1,
        NewItem::Logical(LogicalOp::Or),
    );
    d.apply(&ev, &r).unwrap();
    assert_eq!(
        focus_after(&ev, &d),
        Some(FocusTarget::Slot(
            "policy[0].permission[0].constraint[1]#logicalOp"
                .parse()
                .unwrap()
        ))
    );

    // Remove: the previous item, else the next, else the add button.
    let mut d = rich();
    for _ in 0..2 {
        let n = d.policies[0].obligation.len();
        d.apply(
            &add_ev(&d, lp("policy[0]@obligation"), n, NewItem::Default),
            &r,
        )
        .unwrap();
    }
    let list = lp("policy[0]@obligation");
    let ev = EditEvent::Remove {
        list: list.clone(),
        index: 1,
        expect: id_of(&d, "policy[0].obligation[1]"),
    };
    d.apply(&ev, &r).unwrap();
    assert_eq!(
        check(&d, &ev),
        Some(FocusTarget::Node(np("policy[0].obligation[0]")))
    );
    let ev = EditEvent::Remove {
        list: list.clone(),
        index: 0,
        expect: id_of(&d, "policy[0].obligation[0]"),
    };
    d.apply(&ev, &r).unwrap();
    assert_eq!(
        check(&d, &ev),
        Some(FocusTarget::Node(np("policy[0].obligation[0]")))
    );
    let ev = EditEvent::Remove {
        list: list.clone(),
        index: 0,
        expect: id_of(&d, "policy[0].obligation[0]"),
    };
    d.apply(&ev, &r).unwrap();
    assert!(d.policies[0].obligation.is_empty());
    assert_eq!(check(&d, &ev), Some(FocusTarget::AddButton(list.clone())));
    // Scalar lists use their slot.
    let mut d = rich();
    let ev = EditEvent::Remove {
        list: lp("policy[0]@profile"),
        index: 0,
        expect: d.policies[0].id,
    };
    d.apply(&ev, &r).unwrap();
    assert_eq!(
        check(&d, &ev),
        Some(FocusTarget::AddButton(lp("policy[0]@profile")))
    );

    // Move, ChangeRuleKind, Wrap, Unwrap.
    let mut d = rich();
    d.apply(
        &add_ev(&d, lp("policy[0]@permission"), 1, NewItem::Default),
        &r,
    )
    .unwrap();
    let ev = EditEvent::Move {
        list: lp("policy[0]@permission"),
        from: 0,
        to: 1,
        expect: id_of(&d, "policy[0].permission[0]"),
    };
    d.apply(&ev, &r).unwrap();
    assert_eq!(
        check(&d, &ev),
        Some(FocusTarget::Node(np("policy[0].permission[1]")))
    );
    let ev = EditEvent::ChangeRuleKind {
        rule: np("policy[0].permission[0]"),
        expect: id_of(&d, "policy[0].permission[0]"),
        to: RuleList::Prohibition,
    };
    d.apply(&ev, &r).unwrap();
    assert_eq!(
        check(&d, &ev),
        Some(FocusTarget::Node(np("policy[0].prohibition[1]")))
    );
    let c = np("policy[0].permission[0].constraint[0].child[0]");
    let ev = EditEvent::Wrap {
        constraint: c.clone(),
        expect: id_of(&d, "policy[0].permission[0].constraint[0].child[0]"),
        op: LogicalOp::And,
    };
    d.apply(&ev, &r).unwrap();
    assert_eq!(
        check(&d, &ev),
        Some(FocusTarget::Slot(SlotPath {
            node: c.clone(),
            field: Field::LogicalOp
        }))
    );
    let ev = EditEvent::Unwrap {
        constraint: c.clone(),
        expect: id_of(&d, "policy[0].permission[0].constraint[0].child[0]"),
    };
    d.apply(&ev, &r).unwrap();
    assert_eq!(check(&d, &ev), Some(FocusTarget::Node(c)));
    // A target that is gone gives none.
    let gone = EditEvent::Wrap {
        constraint: np("policy[5]"),
        expect: NodeId(1),
        op: LogicalOp::And,
    };
    assert_eq!(focus_after(&gone, &d), None);
}

#[test]
fn apply_is_atomic_when_an_add_fails_midway() {
    // A rule limit that cannot be met leaves the document as it was.
    let mut r = rules();
    r.rule_action = Limit::NONE;
    let mut doc = rich();
    let before = doc.clone();
    let e = doc
        .apply(
            &add_ev(
                &doc,
                lp("policy[0].permission[0]@action"),
                0,
                NewItem::Default,
            ),
            &r,
        )
        .unwrap_err();
    assert!(matches!(e, EditError::NotAllowed(_)));
    assert_eq!(doc, before);
}

#[test]
fn literal_text_rules() {
    assert_eq!(Literal::Str("a b".into()).display(), "\"a b\"");
    assert_eq!(Literal::Str("urn:x".into()).display(), "urn:x");
    assert_eq!(Literal::Str("2026-01-01".into()).display(), "2026-01-01");
    assert_eq!(Literal::Str("research".into()).display(), "\"research\"");
    assert_eq!(Literal::Bool(true).with_text("false"), Literal::Bool(false));
    assert_eq!(
        Literal::Bool(true).with_text("no"),
        Literal::Str("no".into())
    );
    assert_eq!(
        Literal::Typed {
            value: "1".into(),
            datatype: "xsd:int".into()
        }
        .with_text("2"),
        Literal::Typed {
            value: "2".into(),
            datatype: "xsd:int".into()
        }
    );
    assert_eq!(
        Literal::Iri("a".into()).with_text("b"),
        Literal::Iri("b".into())
    );
    assert_eq!(
        Literal::Num(3.into()).with_text(" 4"),
        Literal::Str(" 4".into())
    );
    assert_eq!(Literal::Num(3.into()).raw(), "3");
}
