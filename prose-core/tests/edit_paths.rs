use prose_core::edit::*;

#[test]
fn every_step_round_trips() {
    let mut steps = vec![
        Step::Action(3),
        Step::Refinement(0),
        Step::Constraint(12),
        Step::Child(1),
    ];
    for l in [
        RuleList::Permission,
        RuleList::Prohibition,
        RuleList::Obligation,
        RuleList::Duty,
        RuleList::Remedy,
        RuleList::Consequence,
    ] {
        steps.push(Step::Rule(l, 2));
    }
    for r in [
        EntityRole::Assigner,
        EntityRole::Assignee,
        EntityRole::Target,
    ] {
        steps.push(Step::Entity(r, 0));
    }
    for s in steps {
        let p = NodePath::policy(4).child(s);
        let text = p.to_string();
        assert_eq!(text.parse::<NodePath>().unwrap(), p, "{text}");
    }
}

#[test]
fn deep_paths_match_the_documented_examples() {
    let p = NodePath::policy(0)
        .child(Step::Rule(RuleList::Permission, 1))
        .child(Step::Rule(RuleList::Duty, 0))
        .child(Step::Rule(RuleList::Consequence, 0));
    assert_eq!(
        p.to_string(),
        "policy[0].permission[1].duty[0].consequence[0]"
    );
    assert_eq!(
        p.parent().unwrap().last(),
        Some(Step::Rule(RuleList::Duty, 0))
    );
    let s: SlotPath = "policy[0].permission[0].constraint[0].child[1]#rightOperand[2]"
        .parse()
        .unwrap();
    assert_eq!(s.field, Field::RightOperand(2));
}

#[test]
fn every_field_round_trips() {
    let node = NodePath::policy(1);
    for f in [
        Field::Kind,
        Field::Uid,
        Field::Conflict,
        Field::Profile(3),
        Field::InheritFrom(0),
        Field::Reference,
        Field::Iri,
        Field::PartOf,
        Field::Name,
        Field::LeftOperand,
        Field::Operator,
        Field::RightOperand(7),
        Field::OperandReference,
        Field::Unit,
        Field::LogicalOp,
    ] {
        let s = SlotPath {
            node: node.clone(),
            field: f,
        };
        assert_eq!(s.to_string().parse::<SlotPath>().unwrap(), s);
    }
}

#[test]
fn every_list_kind_round_trips() {
    let owner = NodePath::policy(0).child(Step::Rule(RuleList::Permission, 0));
    let mut kinds = vec![
        ListKind::Actions,
        ListKind::Refinements,
        ListKind::Constraints,
        ListKind::Children,
        ListKind::RightOperand,
        ListKind::Profile,
        ListKind::InheritFrom,
    ];
    kinds.extend(RuleList::ALL.map(ListKind::Rules));
    kinds.extend(EntityRole::ALL.map(ListKind::Entities));
    for k in kinds {
        let l = ListPath::of(&owner, k);
        assert_eq!(l.to_string().parse::<ListPath>().unwrap(), l, "{l}");
    }
    assert_eq!(
        "policies".parse::<ListPath>().unwrap(),
        ListPath::policies()
    );
    assert_eq!(
        "policy[1]@inheritFrom".parse::<ListPath>().unwrap().kind,
        ListKind::InheritFrom
    );
}

#[test]
fn malformed_strings_are_rejected() {
    for bad in [
        "",
        "policy",
        "policy[]",
        "policy[x]",
        "policy[-1]",
        "policy[0].",
        "policy[0].permission",
        "policy[0].permission[a]",
        "policy[0].unknown[0]",
        "rule[0]",
        "policy[0]]",
        "policy[0] .action[0]",
        "policy[0].duty[0]x",
    ] {
        assert!(bad.parse::<NodePath>().is_err(), "{bad:?} should fail");
    }
    for bad in [
        "policy[0]",
        "policy[0]#",
        "policy[0]#nope",
        "policy[0]#profile",
        "policy[0]#profile[x]",
    ] {
        assert!(bad.parse::<SlotPath>().is_err(), "{bad:?} should fail");
    }
    for bad in [
        "",
        "policy[0]",
        "policy[0]@",
        "policy[0]@nope",
        "policy[0]@policies",
        "@action",
    ] {
        assert!(bad.parse::<ListPath>().is_err(), "{bad:?} should fail");
    }
}
