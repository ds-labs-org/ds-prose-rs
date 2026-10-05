//! Addresses: where a node, a slot or a list lives in an [`EditDoc`].
//!
//! The segment names are the ones ODRL uses for the properties, and the
//! ones a host engine is likely to report rules under, e.g.
//! `policy[0].permission[1].duty[0].consequence[0]#rightOperand[2]`.
use std::fmt;
use std::str::FromStr;

use super::model::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleList {
    Permission,
    Prohibition,
    Obligation,
    Duty,
    Remedy,
    Consequence,
}

impl RuleList {
    pub const ALL: [RuleList; 6] = [
        RuleList::Permission,
        RuleList::Prohibition,
        RuleList::Obligation,
        RuleList::Duty,
        RuleList::Remedy,
        RuleList::Consequence,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            RuleList::Permission => "permission",
            RuleList::Prohibition => "prohibition",
            RuleList::Obligation => "obligation",
            RuleList::Duty => "duty",
            RuleList::Remedy => "remedy",
            RuleList::Consequence => "consequence",
        }
    }

    pub fn parse(s: &str) -> Option<RuleList> {
        RuleList::ALL.into_iter().find(|l| l.as_str() == s)
    }

    pub fn is_top_level(self) -> bool {
        matches!(
            self,
            RuleList::Permission | RuleList::Prohibition | RuleList::Obligation
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityRole {
    Assigner,
    Assignee,
    Target,
}

impl EntityRole {
    pub const ALL: [EntityRole; 3] = [
        EntityRole::Assigner,
        EntityRole::Assignee,
        EntityRole::Target,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EntityRole::Assigner => "assigner",
            EntityRole::Assignee => "assignee",
            EntityRole::Target => "target",
        }
    }

    pub fn parse(s: &str) -> Option<EntityRole> {
        EntityRole::ALL.into_iter().find(|r| r.as_str() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    Rule(RuleList, usize),
    Entity(EntityRole, usize),
    Action(usize),
    Refinement(usize),
    Constraint(usize),
    Child(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodePath {
    pub policy: usize,
    pub steps: Vec<Step>,
}

impl NodePath {
    pub fn policy(index: usize) -> NodePath {
        NodePath {
            policy: index,
            steps: Vec::new(),
        }
    }

    pub fn child(&self, step: Step) -> NodePath {
        let mut steps = self.steps.clone();
        steps.push(step);
        NodePath {
            policy: self.policy,
            steps,
        }
    }

    pub fn parent(&self) -> Option<NodePath> {
        let mut steps = self.steps.clone();
        steps.pop()?;
        Some(NodePath {
            policy: self.policy,
            steps,
        })
    }

    pub fn last(&self) -> Option<Step> {
        self.steps.last().copied()
    }

    /// The rule lists along the path, outermost first.
    pub(crate) fn rule_chain(&self) -> Vec<RuleList> {
        self.steps
            .iter()
            .filter_map(|s| match s {
                Step::Rule(l, _) => Some(*l),
                _ => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Kind,
    Uid,
    Conflict,
    Profile(usize),
    InheritFrom(usize),
    Reference,
    Iri,
    PartOf,
    Name,
    LeftOperand,
    Operator,
    RightOperand(usize),
    OperandReference,
    Unit,
    LogicalOp,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SlotPath {
    pub node: NodePath,
    pub field: Field,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListKind {
    Policies,
    Rules(RuleList),
    Entities(EntityRole),
    Actions,
    Refinements,
    Constraints,
    Children,
    RightOperand,
    Profile,
    InheritFrom,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListPath {
    /// `None` exactly when `kind` is [`ListKind::Policies`].
    pub owner: Option<NodePath>,
    pub kind: ListKind,
}

impl ListPath {
    pub fn policies() -> ListPath {
        ListPath {
            owner: None,
            kind: ListKind::Policies,
        }
    }

    pub fn of(owner: &NodePath, kind: ListKind) -> ListPath {
        ListPath {
            owner: Some(owner.clone()),
            kind,
        }
    }

    /// The path of the node at `index` in this list; `None` for lists of
    /// plain values.
    pub fn item_path(&self, index: usize) -> Option<NodePath> {
        let owner = match (&self.owner, self.kind) {
            (None, ListKind::Policies) => return Some(NodePath::policy(index)),
            (Some(o), _) => o,
            (None, _) => return None,
        };
        let step = match self.kind {
            ListKind::Rules(l) => Step::Rule(l, index),
            ListKind::Entities(r) => Step::Entity(r, index),
            ListKind::Actions => Step::Action(index),
            ListKind::Refinements => Step::Refinement(index),
            ListKind::Constraints => Step::Constraint(index),
            ListKind::Children => Step::Child(index),
            _ => return None,
        };
        Some(owner.child(step))
    }

    /// The slot of the value at `index` for lists of plain values.
    pub fn value_slot(&self, index: usize) -> Option<SlotPath> {
        let field = match self.kind {
            ListKind::RightOperand => Field::RightOperand(index),
            ListKind::Profile => Field::Profile(index),
            ListKind::InheritFrom => Field::InheritFrom(index),
            _ => return None,
        };
        Some(SlotPath {
            node: self.owner.clone()?,
            field,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError(pub String);

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bad path: {}", self.0)
    }
}

impl std::error::Error for PathError {}

// --- Display ----------------------------------------------------------------

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Step::Rule(l, i) => write!(f, "{}[{i}]", l.as_str()),
            Step::Entity(r, i) => write!(f, "{}[{i}]", r.as_str()),
            Step::Action(i) => write!(f, "action[{i}]"),
            Step::Refinement(i) => write!(f, "refinement[{i}]"),
            Step::Constraint(i) => write!(f, "constraint[{i}]"),
            Step::Child(i) => write!(f, "child[{i}]"),
        }
    }
}

impl fmt::Display for NodePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "policy[{}]", self.policy)?;
        for s in &self.steps {
            write!(f, ".{s}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Field::Kind => f.write_str("kind"),
            Field::Uid => f.write_str("uid"),
            Field::Conflict => f.write_str("conflict"),
            Field::Profile(i) => write!(f, "profile[{i}]"),
            Field::InheritFrom(i) => write!(f, "inheritFrom[{i}]"),
            Field::Reference => f.write_str("reference"),
            Field::Iri => f.write_str("iri"),
            Field::PartOf => f.write_str("partOf"),
            Field::Name => f.write_str("name"),
            Field::LeftOperand => f.write_str("leftOperand"),
            Field::Operator => f.write_str("operator"),
            Field::RightOperand(i) => write!(f, "rightOperand[{i}]"),
            Field::OperandReference => f.write_str("rightOperandReference"),
            Field::Unit => f.write_str("unit"),
            Field::LogicalOp => f.write_str("logicalOp"),
        }
    }
}

impl fmt::Display for SlotPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.node, self.field)
    }
}

fn list_kind_name(kind: ListKind) -> &'static str {
    match kind {
        ListKind::Policies => "policies",
        ListKind::Rules(l) => l.as_str(),
        ListKind::Entities(r) => r.as_str(),
        ListKind::Actions => "action",
        ListKind::Refinements => "refinement",
        ListKind::Constraints => "constraint",
        ListKind::Children => "child",
        ListKind::RightOperand => "rightOperand",
        ListKind::Profile => "profile",
        ListKind::InheritFrom => "inheritFrom",
    }
}

impl fmt::Display for ListPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.owner {
            None => f.write_str("policies"),
            Some(o) => write!(f, "{o}@{}", list_kind_name(self.kind)),
        }
    }
}

// --- FromStr ----------------------------------------------------------------

fn err(s: &str) -> PathError {
    PathError(s.to_string())
}

/// `name[3]` -> ("name", 3).
fn indexed(seg: &str) -> Result<(&str, usize), PathError> {
    let open = seg.find('[').ok_or_else(|| err(seg))?;
    let inner = seg[open + 1..].strip_suffix(']').ok_or_else(|| err(seg))?;
    if inner.is_empty() || !inner.bytes().all(|b| b.is_ascii_digit()) {
        return Err(err(seg));
    }
    let n = inner.parse::<usize>().map_err(|_| err(seg))?;
    Ok((&seg[..open], n))
}

impl FromStr for Step {
    type Err = PathError;
    fn from_str(seg: &str) -> Result<Step, PathError> {
        let (name, n) = indexed(seg)?;
        if let Some(l) = RuleList::parse(name) {
            return Ok(Step::Rule(l, n));
        }
        if let Some(r) = EntityRole::parse(name) {
            return Ok(Step::Entity(r, n));
        }
        match name {
            "action" => Ok(Step::Action(n)),
            "refinement" => Ok(Step::Refinement(n)),
            "constraint" => Ok(Step::Constraint(n)),
            "child" => Ok(Step::Child(n)),
            _ => Err(err(seg)),
        }
    }
}

impl FromStr for NodePath {
    type Err = PathError;
    fn from_str(s: &str) -> Result<NodePath, PathError> {
        let mut parts = s.split('.');
        let first = parts.next().ok_or_else(|| err(s))?;
        let (name, policy) = indexed(first).map_err(|_| err(s))?;
        if name != "policy" {
            return Err(err(s));
        }
        let steps = parts
            .map(|p| p.parse::<Step>().map_err(|_| err(s)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(NodePath { policy, steps })
    }
}

impl FromStr for Field {
    type Err = PathError;
    fn from_str(s: &str) -> Result<Field, PathError> {
        Ok(match s {
            "kind" => Field::Kind,
            "uid" => Field::Uid,
            "conflict" => Field::Conflict,
            "reference" => Field::Reference,
            "iri" => Field::Iri,
            "partOf" => Field::PartOf,
            "name" => Field::Name,
            "leftOperand" => Field::LeftOperand,
            "operator" => Field::Operator,
            "rightOperandReference" => Field::OperandReference,
            "unit" => Field::Unit,
            "logicalOp" => Field::LogicalOp,
            _ => {
                let (name, n) = indexed(s)?;
                match name {
                    "profile" => Field::Profile(n),
                    "inheritFrom" => Field::InheritFrom(n),
                    "rightOperand" => Field::RightOperand(n),
                    _ => return Err(err(s)),
                }
            }
        })
    }
}

impl FromStr for SlotPath {
    type Err = PathError;
    fn from_str(s: &str) -> Result<SlotPath, PathError> {
        let (node, field) = s.split_once('#').ok_or_else(|| err(s))?;
        Ok(SlotPath {
            node: node.parse().map_err(|_| err(s))?,
            field: field.parse().map_err(|_| err(s))?,
        })
    }
}

impl FromStr for ListPath {
    type Err = PathError;
    fn from_str(s: &str) -> Result<ListPath, PathError> {
        if s == "policies" {
            return Ok(ListPath::policies());
        }
        let (node, kind) = s.split_once('@').ok_or_else(|| err(s))?;
        let kind = match kind {
            "action" => ListKind::Actions,
            "refinement" => ListKind::Refinements,
            "constraint" => ListKind::Constraints,
            "child" => ListKind::Children,
            "rightOperand" => ListKind::RightOperand,
            "profile" => ListKind::Profile,
            "inheritFrom" => ListKind::InheritFrom,
            other => match (RuleList::parse(other), EntityRole::parse(other)) {
                (Some(l), _) => ListKind::Rules(l),
                (_, Some(r)) => ListKind::Entities(r),
                _ => return Err(err(s)),
            },
        };
        Ok(ListPath {
            owner: Some(node.parse().map_err(|_| err(s))?),
            kind,
        })
    }
}

// --- resolution --------------------------------------------------------------

fn rules_of(p: &PolicyNode, l: RuleList) -> Option<&Vec<RuleNode>> {
    match l {
        RuleList::Permission => Some(&p.permission),
        RuleList::Prohibition => Some(&p.prohibition),
        RuleList::Obligation => Some(&p.obligation),
        _ => None,
    }
}

fn rules_of_mut(p: &mut PolicyNode, l: RuleList) -> Option<&mut Vec<RuleNode>> {
    match l {
        RuleList::Permission => Some(&mut p.permission),
        RuleList::Prohibition => Some(&mut p.prohibition),
        RuleList::Obligation => Some(&mut p.obligation),
        _ => None,
    }
}

fn follow_ups_of(r: &RuleNode, l: RuleList) -> Option<&Vec<RuleNode>> {
    match l {
        RuleList::Duty => Some(&r.duty),
        RuleList::Remedy => Some(&r.remedy),
        RuleList::Consequence => Some(&r.consequence),
        _ => None,
    }
}

fn follow_ups_of_mut(r: &mut RuleNode, l: RuleList) -> Option<&mut Vec<RuleNode>> {
    match l {
        RuleList::Duty => Some(&mut r.duty),
        RuleList::Remedy => Some(&mut r.remedy),
        RuleList::Consequence => Some(&mut r.consequence),
        _ => None,
    }
}

fn entities_of<'a>(
    v: (&'a Vec<Entity>, &'a Vec<Entity>, &'a Vec<Entity>),
    role: EntityRole,
) -> &'a Vec<Entity> {
    match role {
        EntityRole::Assigner => v.0,
        EntityRole::Assignee => v.1,
        EntityRole::Target => v.2,
    }
}

fn entities_mut<'a>(
    v: (
        &'a mut Vec<Entity>,
        &'a mut Vec<Entity>,
        &'a mut Vec<Entity>,
    ),
    role: EntityRole,
) -> &'a mut Vec<Entity> {
    match role {
        EntityRole::Assigner => v.0,
        EntityRole::Assignee => v.1,
        EntityRole::Target => v.2,
    }
}

pub(crate) fn step_ref<'a>(cur: NodeRef<'a>, s: Step) -> Option<NodeRef<'a>> {
    match (cur, s) {
        (NodeRef::Policy(p), Step::Rule(l, i)) => rules_of(p, l)?.get(i).map(NodeRef::Rule),
        (NodeRef::Policy(p), Step::Entity(role, i)) => {
            entities_of((&p.assigner, &p.assignee, &p.target), role)
                .get(i)
                .map(NodeRef::Entity)
        }
        (NodeRef::Policy(p), Step::Action(i)) => p.action.get(i).map(NodeRef::Action),
        (NodeRef::Rule(r), Step::Rule(l, i)) => follow_ups_of(r, l)?.get(i).map(NodeRef::Rule),
        (NodeRef::Rule(r), Step::Entity(role, i)) => {
            entities_of((&r.assigner, &r.assignee, &r.target), role)
                .get(i)
                .map(NodeRef::Entity)
        }
        (NodeRef::Rule(r), Step::Action(i)) => r.action.get(i).map(NodeRef::Action),
        (NodeRef::Rule(r), Step::Constraint(i)) => r.constraint.get(i).map(NodeRef::Constraint),
        (NodeRef::Action(a), Step::Refinement(i)) => a.refinement.get(i).map(NodeRef::Constraint),
        (NodeRef::Entity(e), Step::Refinement(i)) => e.refinement.get(i).map(NodeRef::Constraint),
        (NodeRef::Constraint(ConstraintNode::Logical(l)), Step::Child(i)) => {
            l.children.get(i).map(NodeRef::Constraint)
        }
        _ => None,
    }
}

pub(crate) fn resolve<'a>(doc: &'a EditDoc, path: &NodePath) -> Option<NodeRef<'a>> {
    let mut cur = NodeRef::Policy(doc.policies.get(path.policy)?);
    for s in &path.steps {
        cur = step_ref(cur, *s)?;
    }
    Some(cur)
}

/// The node plus the reason of the first locked node among it and its
/// ancestors, if any.
pub(crate) fn resolve_locked<'a>(
    doc: &'a EditDoc,
    path: &NodePath,
) -> Option<(NodeRef<'a>, Option<String>)> {
    let mut cur = NodeRef::Policy(doc.policies.get(path.policy)?);
    let mut locked = cur.locked().map(str::to_string);
    for s in &path.steps {
        cur = step_ref(cur, *s)?;
        if locked.is_none() {
            locked = cur.locked().map(str::to_string);
        }
    }
    Some((cur, locked))
}

pub(crate) enum NodeMut<'a> {
    Policy(&'a mut PolicyNode),
    Rule(&'a mut RuleNode),
    Entity(&'a mut Entity),
    Action(&'a mut ActionNode),
    Constraint(&'a mut ConstraintNode),
}

pub(crate) fn step_mut<'a>(cur: NodeMut<'a>, s: Step) -> Option<NodeMut<'a>> {
    match (cur, s) {
        (NodeMut::Policy(p), Step::Rule(l, i)) => rules_of_mut(p, l)?.get_mut(i).map(NodeMut::Rule),
        (NodeMut::Policy(p), Step::Entity(role, i)) => {
            entities_mut((&mut p.assigner, &mut p.assignee, &mut p.target), role)
                .get_mut(i)
                .map(NodeMut::Entity)
        }
        (NodeMut::Policy(p), Step::Action(i)) => p.action.get_mut(i).map(NodeMut::Action),
        (NodeMut::Rule(r), Step::Rule(l, i)) => {
            follow_ups_of_mut(r, l)?.get_mut(i).map(NodeMut::Rule)
        }
        (NodeMut::Rule(r), Step::Entity(role, i)) => {
            entities_mut((&mut r.assigner, &mut r.assignee, &mut r.target), role)
                .get_mut(i)
                .map(NodeMut::Entity)
        }
        (NodeMut::Rule(r), Step::Action(i)) => r.action.get_mut(i).map(NodeMut::Action),
        (NodeMut::Rule(r), Step::Constraint(i)) => r.constraint.get_mut(i).map(NodeMut::Constraint),
        (NodeMut::Action(a), Step::Refinement(i)) => {
            a.refinement.get_mut(i).map(NodeMut::Constraint)
        }
        (NodeMut::Entity(e), Step::Refinement(i)) => {
            e.refinement.get_mut(i).map(NodeMut::Constraint)
        }
        (NodeMut::Constraint(ConstraintNode::Logical(l)), Step::Child(i)) => {
            l.children.get_mut(i).map(NodeMut::Constraint)
        }
        _ => None,
    }
}

pub(crate) fn resolve_mut<'a>(doc: &'a mut EditDoc, path: &NodePath) -> Option<NodeMut<'a>> {
    let mut cur = NodeMut::Policy(doc.policies.get_mut(path.policy)?);
    for s in &path.steps {
        cur = step_mut(cur, *s)?;
    }
    Some(cur)
}

/// A mutable view of one list of a document.
pub(crate) enum ListMut<'a> {
    Policies(&'a mut Vec<PolicyNode>),
    Rules(&'a mut Vec<RuleNode>),
    Entities(&'a mut Vec<Entity>),
    Actions(&'a mut Vec<ActionNode>),
    Constraints(&'a mut Vec<ConstraintNode>),
    Strings(&'a mut Vec<String>),
    Literals(&'a mut Vec<Literal>),
}

pub(crate) fn list_mut<'a>(doc: &'a mut EditDoc, lp: &ListPath) -> Option<ListMut<'a>> {
    if lp.kind == ListKind::Policies {
        return lp
            .owner
            .is_none()
            .then_some(ListMut::Policies(&mut doc.policies));
    }
    let owner = resolve_mut(doc, lp.owner.as_ref()?)?;
    match (owner, lp.kind) {
        (NodeMut::Policy(p), ListKind::Rules(l)) => rules_of_mut(p, l).map(ListMut::Rules),
        (NodeMut::Rule(r), ListKind::Rules(l)) => follow_ups_of_mut(r, l).map(ListMut::Rules),
        (NodeMut::Policy(p), ListKind::Entities(role)) => Some(ListMut::Entities(entities_mut(
            (&mut p.assigner, &mut p.assignee, &mut p.target),
            role,
        ))),
        (NodeMut::Rule(r), ListKind::Entities(role)) => Some(ListMut::Entities(entities_mut(
            (&mut r.assigner, &mut r.assignee, &mut r.target),
            role,
        ))),
        (NodeMut::Policy(p), ListKind::Actions) => Some(ListMut::Actions(&mut p.action)),
        (NodeMut::Rule(r), ListKind::Actions) => Some(ListMut::Actions(&mut r.action)),
        (NodeMut::Action(a), ListKind::Refinements) => {
            Some(ListMut::Constraints(&mut a.refinement))
        }
        (NodeMut::Entity(e), ListKind::Refinements) => {
            Some(ListMut::Constraints(&mut e.refinement))
        }
        (NodeMut::Rule(r), ListKind::Constraints) => Some(ListMut::Constraints(&mut r.constraint)),
        (NodeMut::Constraint(ConstraintNode::Logical(l)), ListKind::Children) => {
            Some(ListMut::Constraints(&mut l.children))
        }
        (NodeMut::Constraint(ConstraintNode::Atomic(a)), ListKind::RightOperand) => {
            match &mut a.right {
                RightOperand::Values(v) => Some(ListMut::Literals(v)),
                _ => None,
            }
        }
        (NodeMut::Policy(p), ListKind::Profile) => Some(ListMut::Strings(&mut p.profile)),
        (NodeMut::Policy(p), ListKind::InheritFrom) => Some(ListMut::Strings(&mut p.inherit_from)),
        _ => None,
    }
}

impl ListMut<'_> {
    pub(crate) fn len(&self) -> usize {
        match self {
            ListMut::Policies(v) => v.len(),
            ListMut::Rules(v) => v.len(),
            ListMut::Entities(v) => v.len(),
            ListMut::Actions(v) => v.len(),
            ListMut::Constraints(v) => v.len(),
            ListMut::Strings(v) => v.len(),
            ListMut::Literals(v) => v.len(),
        }
    }

    /// Id of the item at `i` for lists of nodes; `None` for plain values.
    pub(crate) fn id_at(&self, i: usize) -> Option<NodeId> {
        match self {
            ListMut::Policies(v) => v.get(i).map(|n| n.id),
            ListMut::Rules(v) => v.get(i).map(|n| n.id),
            ListMut::Entities(v) => v.get(i).map(|n| n.id),
            ListMut::Actions(v) => v.get(i).map(|n| n.id),
            ListMut::Constraints(v) => v.get(i).map(|n| n.id()),
            ListMut::Strings(_) | ListMut::Literals(_) => None,
        }
    }

    pub(crate) fn remove(&mut self, i: usize) {
        match self {
            ListMut::Policies(v) => drop(v.remove(i)),
            ListMut::Rules(v) => drop(v.remove(i)),
            ListMut::Entities(v) => drop(v.remove(i)),
            ListMut::Actions(v) => drop(v.remove(i)),
            ListMut::Constraints(v) => drop(v.remove(i)),
            ListMut::Strings(v) => drop(v.remove(i)),
            ListMut::Literals(v) => drop(v.remove(i)),
        }
    }

    /// Move the item at `from` so that it ends up at `to`.
    pub(crate) fn relocate(&mut self, from: usize, to: usize) {
        fn go<T>(v: &mut Vec<T>, from: usize, to: usize) {
            let item = v.remove(from);
            v.insert(to, item);
        }
        match self {
            ListMut::Policies(v) => go(v, from, to),
            ListMut::Rules(v) => go(v, from, to),
            ListMut::Entities(v) => go(v, from, to),
            ListMut::Actions(v) => go(v, from, to),
            ListMut::Constraints(v) => go(v, from, to),
            ListMut::Strings(v) => go(v, from, to),
            ListMut::Literals(v) => go(v, from, to),
        }
    }
}

/// The owner id and current length of a list, or `None` when the list does
/// not exist in the document.
pub(crate) fn list_info(doc: &EditDoc, lp: &ListPath) -> Option<(NodeId, usize)> {
    if lp.kind == ListKind::Policies {
        return lp
            .owner
            .is_none()
            .then_some((NodeId(0), doc.policies.len()));
    }
    let owner = resolve(doc, lp.owner.as_ref()?)?;
    let len = match (owner, lp.kind) {
        (NodeRef::Policy(p), ListKind::Rules(l)) => rules_of(p, l)?.len(),
        (NodeRef::Rule(r), ListKind::Rules(l)) => follow_ups_of(r, l)?.len(),
        (NodeRef::Policy(p), ListKind::Entities(role)) => {
            entities_of((&p.assigner, &p.assignee, &p.target), role).len()
        }
        (NodeRef::Rule(r), ListKind::Entities(role)) => {
            entities_of((&r.assigner, &r.assignee, &r.target), role).len()
        }
        (NodeRef::Policy(p), ListKind::Actions) => p.action.len(),
        (NodeRef::Rule(r), ListKind::Actions) => r.action.len(),
        (NodeRef::Action(a), ListKind::Refinements) => a.refinement.len(),
        (NodeRef::Entity(e), ListKind::Refinements) => e.refinement.len(),
        (NodeRef::Rule(r), ListKind::Constraints) => r.constraint.len(),
        (NodeRef::Constraint(ConstraintNode::Logical(l)), ListKind::Children) => l.children.len(),
        (NodeRef::Constraint(ConstraintNode::Atomic(a)), ListKind::RightOperand) => {
            match &a.right {
                RightOperand::Values(v) => v.len(),
                _ => 0,
            }
        }
        (NodeRef::Policy(p), ListKind::Profile) => p.profile.len(),
        (NodeRef::Policy(p), ListKind::InheritFrom) => p.inherit_from.len(),
        _ => return None,
    };
    Some((owner.id(), len))
}
