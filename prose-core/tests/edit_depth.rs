//! `EditRules::logical_depth`: how deep `and`/`or`/`xone` groups may nest.
mod common;
use common::*;
use prose_core::edit::*;

fn doc(constraint: &str) -> EditDoc {
    read_model(&format!(
        r#"{{"@type": "Set", "uid": "urn:p",
            "permission": [{{"target": "urn:a", "action": "use", "constraint": [{constraint}]}}]}}"#
    ))
    .unwrap()
}

const ATOM: &str = r#"{"leftOperand": "count", "operator": "lteq", "rightOperand": 3}"#;

fn depth(n: usize) -> EditRules {
    EditRules {
        logical_depth: Limit::at_most(n),
        ..EditRules::default()
    }
}

fn add_group(doc: &mut EditDoc, list: &str, rules: &EditRules) -> Result<(), EditError> {
    let list = lp(list);
    let len = list_len(doc, &list);
    let ev = add_ev(doc, list, len, NewItem::Logical(LogicalOp::And));
    doc.apply(&ev, rules)
}

fn wrap(doc: &mut EditDoc, path: &str, rules: &EditRules) -> Result<(), EditError> {
    let ev = EditEvent::Wrap {
        constraint: np(path),
        expect: id_of(doc, path),
        op: LogicalOp::And,
    };
    doc.apply(&ev, rules)
}

#[test]
fn the_default_nests_without_a_limit() {
    assert_eq!(EditRules::default().logical_depth, Limit::ANY);
    let mut d = doc(ATOM);
    let r = EditRules::default();
    for _ in 0..5 {
        wrap(&mut d, "policy[0].permission[0].constraint[0]", &r).unwrap();
    }
}

#[test]
fn a_group_may_not_nest_deeper_than_the_limit() {
    let r = depth(2);
    let mut d = doc(ATOM);
    add_group(&mut d, "policy[0].permission[0]@constraint", &r).unwrap();
    // The group at constraint[1] holds one atom; a group in it is depth 2.
    add_group(&mut d, "policy[0].permission[0].constraint[1]@child", &r).unwrap();
    // A third level is refused.
    let err = add_group(
        &mut d,
        "policy[0].permission[0].constraint[1].child[1]@child",
        &r,
    )
    .unwrap_err();
    assert!(matches!(err, EditError::NotAllowed(_)), "{err:?}");
    assert!(err.to_string().contains("nest"), "{err}");
}

#[test]
fn a_depth_of_zero_allows_no_group_at_all() {
    let r = depth(0);
    let mut d = doc(ATOM);
    assert!(add_group(&mut d, "policy[0].permission[0]@constraint", &r).is_err());
    assert!(wrap(&mut d, "policy[0].permission[0].constraint[0]", &r).is_err());
    assert!(!r.can_add_item(
        &d,
        &lp("policy[0].permission[0]@constraint"),
        NewItem::Logical(LogicalOp::Or)
    ));
    assert!(r.can_add_item(
        &d,
        &lp("policy[0].permission[0]@constraint"),
        NewItem::Default
    ));
}

#[test]
fn wrapping_counts_the_height_of_what_it_wraps() {
    let r = depth(2);
    let mut d = doc(&format!(r#"{{"and": [{{"or": [{ATOM}]}}]}}"#));
    // An and holding an or is two deep already: wrapping it makes three.
    assert!(!r.can_wrap(&d, &np("policy[0].permission[0].constraint[0]")));
    assert!(wrap(&mut d, "policy[0].permission[0].constraint[0]", &r).is_err());
    // Wrapping the atom inside the or would make three as well.
    assert!(!r.can_wrap(
        &d,
        &np("policy[0].permission[0].constraint[0].child[0].child[0]")
    ));
    // An atom beside it may be wrapped: depth 1.
    let mut d = doc(&format!(r#"{ATOM}, {{"or": [{ATOM}]}}"#));
    assert!(r.can_wrap(&d, &np("policy[0].permission[0].constraint[0]")));
    wrap(&mut d, "policy[0].permission[0].constraint[0]", &r).unwrap();
    // The or is now depth 1; wrapping it gives 2, which is allowed.
    wrap(&mut d, "policy[0].permission[0].constraint[1]", &r).unwrap();
}

#[test]
fn a_document_already_too_deep_is_read_and_unwrapped() {
    // The limit is for adding, not for reading: nothing is dropped, and an
    // ungroup is always allowed.
    let r = depth(1);
    let mut d = doc(&format!(r#"{{"and": [{{"or": [{{"xone": [{ATOM}]}}]}}]}}"#));
    let path = "policy[0].permission[0].constraint[0]";
    d.apply(
        &EditEvent::Unwrap {
            constraint: np(path),
            expect: id_of(&d, path),
        },
        &r,
    )
    .unwrap();
}

#[test]
fn a_new_group_in_a_refinement_counts_from_the_refinement() {
    let r = depth(1);
    let mut d = rich();
    add_group(&mut d, "policy[0].action[0]@refinement", &r).unwrap();
}
