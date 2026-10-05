//! Events and the one generic reducer that applies them.
use std::fmt;

use super::model::*;
use super::path::*;
use super::rules::{EditRules, odrl_position};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NewItem {
    #[default]
    Default,
    Logical(LogicalOp),
}

/// One committed edit or structural action. `expect` is the id of the node
/// the editor saw when it made the event; a mismatch means the document has
/// moved on and the event is refused as [`EditError::Stale`].
///
/// For `Remove` and `Move` on lists of nodes, `expect` is the id of the
/// item; on lists of plain values (right operand values, profiles,
/// `inheritFrom`) it is the id of the list's owner.
#[derive(Debug, Clone, PartialEq)]
pub enum EditEvent {
    /// Committed free text, stored verbatim (never trimmed).
    SetText {
        slot: SlotPath,
        expect: NodeId,
        value: String,
    },
    /// A pick from a closed list: Operator, LogicalOp or Conflict ("" = absent).
    SetChoice {
        slot: SlotPath,
        expect: NodeId,
        value: String,
    },
    /// Insert at `index`, 0..=len.
    Add {
        list: ListPath,
        index: usize,
        item: NewItem,
    },
    Remove {
        list: ListPath,
        index: usize,
        expect: NodeId,
    },
    Move {
        list: ListPath,
        from: usize,
        to: usize,
        expect: NodeId,
    },
    /// Top-level rules only; appended at the end of the new list.
    ChangeRuleKind {
        rule: NodePath,
        expect: NodeId,
        to: RuleList,
    },
    Wrap {
        constraint: NodePath,
        expect: NodeId,
        op: LogicalOp,
    },
    /// Only a logical node with exactly one child.
    Unwrap {
        constraint: NodePath,
        expect: NodeId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditError {
    NoSuchPath(String),
    Stale { expected: NodeId, found: NodeId },
    NotAllowed(String),
    Locked(String),
    Invalid(String),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::NoSuchPath(p) => write!(f, "nothing at {p}"),
            EditError::Stale { expected, found } => write!(
                f,
                "the document changed under this edit (expected node {}, found {})",
                expected.0, found.0
            ),
            EditError::NotAllowed(why) => write!(f, "not allowed: {why}"),
            EditError::Locked(why) => write!(f, "kept as written: {why}"),
            EditError::Invalid(why) => write!(f, "invalid: {why}"),
        }
    }
}

impl std::error::Error for EditError {}

fn no_node(p: &NodePath) -> EditError {
    EditError::NoSuchPath(p.to_string())
}

fn no_list(l: &ListPath) -> EditError {
    EditError::NoSuchPath(l.to_string())
}

/// Resolve `path`, check `expect` and (unless `allow_locked`) that neither
/// the node nor an ancestor is locked.
fn check_node(
    doc: &EditDoc,
    path: &NodePath,
    expect: NodeId,
    allow_locked: bool,
) -> Result<(), EditError> {
    let (node, locked) = resolve_locked(doc, path).ok_or_else(|| no_node(path))?;
    if node.id() != expect {
        return Err(EditError::Stale {
            expected: expect,
            found: node.id(),
        });
    }
    match locked {
        Some(reason) if !allow_locked => Err(EditError::Locked(reason)),
        _ => Ok(()),
    }
}

fn check_owner_unlocked(doc: &EditDoc, list: &ListPath) -> Result<(), EditError> {
    if let Some(owner) = &list.owner {
        match resolve_locked(doc, owner) {
            None => return Err(no_list(list)),
            Some((_, Some(reason))) => return Err(EditError::Locked(reason)),
            Some(_) => {}
        }
    }
    Ok(())
}

pub(crate) fn apply_event(
    doc: &mut EditDoc,
    ev: &EditEvent,
    rules: &EditRules,
) -> Result<(), EditError> {
    match ev {
        EditEvent::SetText {
            slot,
            expect,
            value,
        } => set_text(doc, slot, *expect, value),
        EditEvent::SetChoice {
            slot,
            expect,
            value,
        } => set_choice(doc, slot, *expect, value),
        EditEvent::Add { list, index, item } => add(doc, rules, list, *index, *item),
        EditEvent::Remove {
            list,
            index,
            expect,
        } => remove(doc, rules, list, *index, *expect),
        EditEvent::Move {
            list,
            from,
            to,
            expect,
        } => move_item(doc, rules, list, *from, *to, *expect),
        EditEvent::ChangeRuleKind { rule, expect, to } => {
            change_rule_kind(doc, rules, rule, *expect, *to)
        }
        EditEvent::Wrap {
            constraint,
            expect,
            op,
        } => wrap(doc, rules, constraint, *expect, *op),
        EditEvent::Unwrap { constraint, expect } => unwrap_constraint(doc, constraint, *expect),
    }
}

fn opt(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn set_text(
    doc: &mut EditDoc,
    slot: &SlotPath,
    expect: NodeId,
    value: &str,
) -> Result<(), EditError> {
    check_node(doc, &slot.node, expect, false)?;
    let bad = || EditError::NoSuchPath(slot.to_string());
    let node = resolve_mut(doc, &slot.node).ok_or_else(|| no_node(&slot.node))?;
    match (node, slot.field) {
        (NodeMut::Policy(p), Field::Kind) => p.kind = value.to_string(),
        (NodeMut::Policy(p), Field::Uid) => p.uid = Some(value.to_string()),
        (NodeMut::Rule(r), Field::Uid) => r.uid = Some(value.to_string()),
        (NodeMut::Rule(r), Field::Reference) => r.reference = Some(value.to_string()),
        (NodeMut::Constraint(ConstraintNode::Reference { iri, .. }), Field::Reference) => {
            *iri = value.to_string()
        }
        (NodeMut::Entity(e), Field::Iri) => e.iri = Some(value.to_string()),
        (NodeMut::Entity(e), Field::PartOf) => e.part_of = opt(value),
        (NodeMut::Action(a), Field::Name) => a.name = value.to_string(),
        (NodeMut::Constraint(ConstraintNode::Atomic(a)), Field::LeftOperand) => {
            a.left = value.to_string()
        }
        (NodeMut::Constraint(ConstraintNode::Atomic(a)), Field::RightOperand(k)) => {
            match &mut a.right {
                RightOperand::Values(v) => {
                    let lit = v.get_mut(k).ok_or_else(bad)?;
                    *lit = lit.with_text(value);
                }
                _ => return Err(bad()),
            }
        }
        (NodeMut::Constraint(ConstraintNode::Atomic(a)), Field::OperandReference) => {
            a.right = RightOperand::Reference(value.to_string())
        }
        (NodeMut::Constraint(ConstraintNode::Atomic(a)), Field::Unit) => a.unit = opt(value),
        (NodeMut::Policy(p), Field::Profile(k)) => {
            *p.profile.get_mut(k).ok_or_else(bad)? = value.to_string()
        }
        (NodeMut::Policy(p), Field::InheritFrom(k)) => {
            *p.inherit_from.get_mut(k).ok_or_else(bad)? = value.to_string()
        }
        _ => return Err(bad()),
    }
    Ok(())
}

fn set_choice(
    doc: &mut EditDoc,
    slot: &SlotPath,
    expect: NodeId,
    value: &str,
) -> Result<(), EditError> {
    check_node(doc, &slot.node, expect, false)?;
    let bad = || EditError::NoSuchPath(slot.to_string());
    let node = resolve_mut(doc, &slot.node).ok_or_else(|| no_node(&slot.node))?;
    match (node, slot.field) {
        (NodeMut::Constraint(ConstraintNode::Atomic(a)), Field::Operator) => {
            a.operator = value.to_string()
        }
        (NodeMut::Constraint(ConstraintNode::Logical(l)), Field::LogicalOp) => {
            l.op = LogicalOp::parse(value)
                .ok_or_else(|| EditError::Invalid(format!("{value} is not a logical operator")))?
        }
        (NodeMut::Policy(p), Field::Conflict) => p.conflict = opt(value),
        _ => return Err(bad()),
    }
    Ok(())
}

// --- add ----------------------------------------------------------------------

enum New {
    Policy(PolicyNode),
    Rule(RuleNode),
    Entity(Entity),
    Action(ActionNode),
    Constraint(ConstraintNode),
    Str,
    Literal,
}

fn default_atomic(rules: &EditRules) -> ConstraintNode {
    ConstraintNode::Atomic(AtomicConstraint {
        left: rules.defaults.left_operand.clone(),
        operator: rules.defaults.operator.clone(),
        right: RightOperand::Values(vec![Literal::Str(String::new())]),
        ..AtomicConstraint::default()
    })
}

fn new_node(
    doc: &EditDoc,
    rules: &EditRules,
    kind: ListKind,
    item: NewItem,
) -> Result<New, EditError> {
    let constraint_list = matches!(
        kind,
        ListKind::Refinements | ListKind::Constraints | ListKind::Children
    );
    if let NewItem::Logical(op) = item {
        if !constraint_list {
            return Err(EditError::Invalid(
                "only constraint lists take a logical group".into(),
            ));
        }
        if !rules.allow_logical {
            return Err(EditError::NotAllowed("logical groups are disabled".into()));
        }
        return Ok(New::Constraint(ConstraintNode::Logical(
            LogicalConstraint {
                op,
                children: vec![default_atomic(rules)],
                ..LogicalConstraint::default()
            },
        )));
    }
    Ok(match kind {
        ListKind::Policies => {
            let used: Vec<&str> = doc
                .policies
                .iter()
                .filter_map(|p| p.uid.as_deref())
                .collect();
            let n = (1..)
                .find(|n| {
                    let candidate = format!("{}{n}", rules.defaults.uid_prefix);
                    !used.contains(&candidate.as_str())
                })
                .unwrap_or(1);
            New::Policy(PolicyNode {
                kind: rules.defaults.policy_kind.clone(),
                uid: Some(format!("{}{n}", rules.defaults.uid_prefix)),
                ..PolicyNode::default()
            })
        }
        ListKind::Rules(_) => New::Rule(RuleNode {
            action: if rules.rule_action.max == Some(0) {
                Vec::new()
            } else {
                vec![ActionNode {
                    name: rules.defaults.action.clone(),
                    ..ActionNode::default()
                }]
            },
            ..RuleNode::default()
        }),
        ListKind::Entities(_) => New::Entity(Entity {
            iri: Some(String::new()),
            ..Entity::default()
        }),
        ListKind::Actions => New::Action(ActionNode {
            name: rules.defaults.action.clone(),
            ..ActionNode::default()
        }),
        ListKind::Refinements | ListKind::Constraints | ListKind::Children => {
            New::Constraint(default_atomic(rules))
        }
        ListKind::RightOperand => New::Literal,
        ListKind::Profile | ListKind::InheritFrom => New::Str,
    })
}

/// Insert without checking limits; used by `Add` after its checks and to
/// fill a new node's required lists.
fn insert_new(doc: &mut EditDoc, list: &ListPath, index: usize, new: New) -> Result<(), EditError> {
    let mut lm = list_mut(doc, list).ok_or_else(|| no_list(list))?;
    if index > lm.len() {
        return Err(no_list(list));
    }
    match (&mut lm, new) {
        (ListMut::Policies(v), New::Policy(n)) => v.insert(index, n),
        (ListMut::Rules(v), New::Rule(n)) => v.insert(index, n),
        (ListMut::Entities(v), New::Entity(n)) => v.insert(index, n),
        (ListMut::Actions(v), New::Action(n)) => v.insert(index, n),
        (ListMut::Constraints(v), New::Constraint(n)) => v.insert(index, n),
        (ListMut::Strings(v), New::Str) => v.insert(index, String::new()),
        (ListMut::Literals(v), New::Literal) => v.insert(index, Literal::Str(String::new())),
        _ => return Err(EditError::Invalid("item does not fit this list".into())),
    }
    doc.number_unset();
    Ok(())
}

/// The lists a node has that an editor may add to.
fn node_lists(doc: &EditDoc, path: &NodePath) -> Vec<ListPath> {
    let Some(node) = doc.node(path) else {
        return vec![];
    };
    let mut out = Vec::new();
    let roles = || EntityRole::ALL.into_iter().map(ListKind::Entities);
    match node {
        NodeRef::Policy(p) if p.locked.is_none() => {
            out.extend(roles());
            out.push(ListKind::Actions);
            out.extend(
                [
                    RuleList::Permission,
                    RuleList::Prohibition,
                    RuleList::Obligation,
                ]
                .map(ListKind::Rules),
            );
            out.push(ListKind::Profile);
            out.push(ListKind::InheritFrom);
        }
        NodeRef::Rule(r) if r.locked.is_none() && r.reference.is_none() => {
            out.extend(roles());
            out.push(ListKind::Actions);
            out.push(ListKind::Constraints);
            out.extend(
                [RuleList::Duty, RuleList::Remedy, RuleList::Consequence].map(ListKind::Rules),
            );
        }
        NodeRef::Action(a) if a.locked.is_none() => out.push(ListKind::Refinements),
        NodeRef::Entity(e) if e.locked.is_none() => out.push(ListKind::Refinements),
        NodeRef::Constraint(ConstraintNode::Atomic(_)) => out.push(ListKind::RightOperand),
        NodeRef::Constraint(ConstraintNode::Logical(_)) => out.push(ListKind::Children),
        _ => {}
    }
    out.into_iter().map(|k| ListPath::of(path, k)).collect()
}

/// Add default items until every list of the node at `path` meets its
/// minimum, recursing into what it adds.
fn fill_min(doc: &mut EditDoc, rules: &EditRules, path: &NodePath, depth: usize) {
    if depth > 6 {
        return;
    }
    for lp in node_lists(doc, path) {
        if let ListKind::Rules(kind) = lp.kind {
            let parent = path.rule_chain().last().copied();
            if !odrl_position(if kind.is_top_level() { None } else { parent }, kind) {
                continue;
            }
        }
        let limit = rules.limit(doc, &lp);
        while let Some((_, len)) = list_info(doc, &lp) {
            if len >= limit.min || limit.max.is_some_and(|m| len >= m) {
                break;
            }
            let Ok(new) = new_node(doc, rules, lp.kind, NewItem::Default) else {
                break;
            };
            if insert_new(doc, &lp, len, new).is_err() {
                break;
            }
            if let Some(child) = lp.item_path(len) {
                fill_min(doc, rules, &child, depth + 1);
            }
        }
    }
}

fn add(
    doc: &mut EditDoc,
    rules: &EditRules,
    list: &ListPath,
    index: usize,
    item: NewItem,
) -> Result<(), EditError> {
    let (_, len) = list_info(doc, list).ok_or_else(|| no_list(list))?;
    check_owner_unlocked(doc, list)?;
    if index > len {
        return Err(no_list(list));
    }
    if let Some(why) = rules.add_refusal(doc, list) {
        return Err(EditError::NotAllowed(why));
    }
    let new = new_node(doc, rules, list.kind, item)?;
    insert_new(doc, list, index, new)?;
    if let Some(path) = list.item_path(index) {
        fill_min(doc, rules, &path, 0);
    }
    Ok(())
}

// --- remove / move --------------------------------------------------------------

fn check_item(
    doc: &mut EditDoc,
    list: &ListPath,
    index: usize,
    expect: NodeId,
) -> Result<(), EditError> {
    let (owner_id, len) = list_info(doc, list).ok_or_else(|| no_list(list))?;
    if index >= len {
        return Err(EditError::NoSuchPath(format!("{list}[{index}]")));
    }
    let found = {
        let lm = list_mut(doc, list).ok_or_else(|| no_list(list))?;
        lm.id_at(index).unwrap_or(owner_id)
    };
    if found != expect {
        return Err(EditError::Stale {
            expected: expect,
            found,
        });
    }
    Ok(())
}

fn remove(
    doc: &mut EditDoc,
    rules: &EditRules,
    list: &ListPath,
    index: usize,
    expect: NodeId,
) -> Result<(), EditError> {
    check_item(doc, list, index, expect)?;
    check_owner_unlocked(doc, list)?;
    if let Some(why) = rules.remove_refusal(doc, list) {
        return Err(EditError::NotAllowed(why));
    }
    list_mut(doc, list)
        .ok_or_else(|| no_list(list))?
        .remove(index);
    Ok(())
}

fn move_item(
    doc: &mut EditDoc,
    rules: &EditRules,
    list: &ListPath,
    from: usize,
    to: usize,
    expect: NodeId,
) -> Result<(), EditError> {
    if !rules.allow_reorder {
        return Err(EditError::NotAllowed("reordering is disabled".into()));
    }
    check_item(doc, list, from, expect)?;
    check_owner_unlocked(doc, list)?;
    let mut lm = list_mut(doc, list).ok_or_else(|| no_list(list))?;
    if to >= lm.len() {
        return Err(EditError::NoSuchPath(format!("{list}[{to}]")));
    }
    lm.relocate(from, to);
    Ok(())
}

fn change_rule_kind(
    doc: &mut EditDoc,
    rules: &EditRules,
    rule: &NodePath,
    expect: NodeId,
    to: RuleList,
) -> Result<(), EditError> {
    check_node(doc, rule, expect, true)?;
    if !rules.allow_rule_kind_change {
        return Err(EditError::NotAllowed(
            "changing a rule's kind is disabled".into(),
        ));
    }
    let (from, index) = match rule.steps.as_slice() {
        [Step::Rule(l, i)] if l.is_top_level() => (*l, *i),
        _ => {
            return Err(EditError::Invalid(
                "only a top-level rule can change kind".into(),
            ));
        }
    };
    if !to.is_top_level() || to == from {
        return Err(EditError::Invalid(format!(
            "cannot change {} to {}",
            from.as_str(),
            to.as_str()
        )));
    }
    let owner = NodePath::policy(rule.policy);
    let dest = ListPath::of(&owner, ListKind::Rules(to));
    if let Some((_, Some(reason))) = resolve_locked(doc, &owner) {
        return Err(EditError::Locked(reason));
    }
    if let Some(why) = rules.add_refusal(doc, &dest) {
        return Err(EditError::NotAllowed(why));
    }
    let src = ListPath::of(&owner, ListKind::Rules(from));
    let Some(ListMut::Rules(v)) = list_mut(doc, &src) else {
        return Err(no_node(rule));
    };
    let node = v.remove(index);
    let Some(ListMut::Rules(d)) = list_mut(doc, &dest) else {
        return Err(no_node(rule));
    };
    d.push(node);
    Ok(())
}

fn wrap(
    doc: &mut EditDoc,
    rules: &EditRules,
    path: &NodePath,
    expect: NodeId,
    op: LogicalOp,
) -> Result<(), EditError> {
    check_node(doc, path, expect, false)?;
    if !rules.allow_logical {
        return Err(EditError::NotAllowed("logical groups are disabled".into()));
    }
    let id = doc.fresh_id();
    match resolve_mut(doc, path) {
        Some(NodeMut::Constraint(c)) => {
            let old = std::mem::take(c);
            *c = ConstraintNode::Logical(LogicalConstraint {
                id,
                origin: None,
                op,
                children: vec![old],
            });
            Ok(())
        }
        Some(_) => Err(EditError::Invalid(
            "only a constraint can be grouped".into(),
        )),
        None => Err(no_node(path)),
    }
}

fn unwrap_constraint(doc: &mut EditDoc, path: &NodePath, expect: NodeId) -> Result<(), EditError> {
    check_node(doc, path, expect, false)?;
    match resolve_mut(doc, path) {
        Some(NodeMut::Constraint(c)) => {
            let one = matches!(c, ConstraintNode::Logical(l) if l.children.len() == 1);
            if !one {
                return Err(EditError::Invalid(
                    "only a group with exactly one condition can be ungrouped".into(),
                ));
            }
            if let ConstraintNode::Logical(l) = std::mem::take(c) {
                *c = l.children.into_iter().next().unwrap_or_default();
            }
            Ok(())
        }
        Some(_) => Err(EditError::Invalid(
            "only a constraint can be ungrouped".into(),
        )),
        None => Err(no_node(path)),
    }
}

// --- focus --------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum FocusTarget {
    Slot(SlotPath),
    AddButton(ListPath),
    Node(NodePath),
}

fn first_slot(doc: &EditDoc, path: &NodePath) -> Option<FocusTarget> {
    let node = doc.node(path)?;
    let slot = |node: NodePath, field| Some(FocusTarget::Slot(SlotPath { node, field }));
    match node {
        NodeRef::Policy(_) => slot(path.clone(), Field::Uid),
        NodeRef::Rule(r) if r.reference.is_some() => slot(path.clone(), Field::Reference),
        NodeRef::Rule(r) if !r.action.is_empty() => slot(path.child(Step::Action(0)), Field::Name),
        NodeRef::Rule(_) => Some(FocusTarget::Node(path.clone())),
        NodeRef::Entity(_) => slot(path.clone(), Field::Iri),
        NodeRef::Action(_) => slot(path.clone(), Field::Name),
        NodeRef::Constraint(ConstraintNode::Atomic(_)) => slot(path.clone(), Field::LeftOperand),
        NodeRef::Constraint(ConstraintNode::Logical(_)) => slot(path.clone(), Field::LogicalOp),
        NodeRef::Constraint(ConstraintNode::Reference { .. }) => {
            slot(path.clone(), Field::Reference)
        }
        NodeRef::Constraint(ConstraintNode::Opaque { .. }) => Some(FocusTarget::Node(path.clone())),
    }
}

fn item_target(doc: &EditDoc, list: &ListPath, index: usize) -> Option<FocusTarget> {
    match list.item_path(index) {
        Some(p) => doc.node(&p).map(|_| FocusTarget::Node(p)),
        None => {
            let slot = list.value_slot(index)?;
            let (_, len) = list_info(doc, list)?;
            (index < len).then_some(FocusTarget::Slot(slot))
        }
    }
}

/// Where keyboard focus should go once `ev` has been applied to produce
/// `doc_after`. `None` when no move is needed or the target is gone.
pub fn focus_after(ev: &EditEvent, doc_after: &EditDoc) -> Option<FocusTarget> {
    match ev {
        EditEvent::SetText { .. } | EditEvent::SetChoice { .. } => None,
        EditEvent::Add { list, index, .. } => match list.item_path(*index) {
            Some(p) => first_slot(doc_after, &p),
            None => {
                let slot = list.value_slot(*index)?;
                let (_, len) = list_info(doc_after, list)?;
                (*index < len).then_some(FocusTarget::Slot(slot))
            }
        },
        EditEvent::Remove { list, index, .. } => {
            let (_, len) = list_info(doc_after, list)?;
            if *index >= 1 && index - 1 < len {
                item_target(doc_after, list, index - 1)
            } else if *index < len {
                item_target(doc_after, list, *index)
            } else {
                Some(FocusTarget::AddButton(list.clone()))
            }
        }
        EditEvent::Move { list, to, .. } => item_target(doc_after, list, *to),
        EditEvent::ChangeRuleKind { rule, to, .. } => {
            let (_, len) = list_info(
                doc_after,
                &ListPath::of(&NodePath::policy(rule.policy), ListKind::Rules(*to)),
            )?;
            let p = NodePath::policy(rule.policy).child(Step::Rule(*to, len.checked_sub(1)?));
            doc_after.node(&p).map(|_| FocusTarget::Node(p))
        }
        EditEvent::Wrap { constraint, .. } => {
            doc_after.node(constraint)?;
            Some(FocusTarget::Slot(SlotPath {
                node: constraint.clone(),
                field: Field::LogicalOp,
            }))
        }
        EditEvent::Unwrap { constraint, .. } => doc_after
            .node(constraint)
            .map(|_| FocusTarget::Node(constraint.clone())),
    }
}
