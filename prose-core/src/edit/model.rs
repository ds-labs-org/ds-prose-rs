//! The editable model: policies as trees of addressable, numbered nodes.
use serde_json::Value;
use std::sync::Arc;

use super::apply::{EditError, EditEvent};
use super::path::{self, NodePath};
use super::rules::EditRules;
use crate::read::fmt_str;

/// Identity of a node inside one [`EditDoc`]. 0 means "not numbered yet".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NodeId(pub u32);

/// The JSON a reader built a node from. It travels with the node through
/// every edit, including moves, so a writer can merge into it.
#[derive(Debug, Clone)]
pub struct Origin(pub Arc<Value>);

impl Origin {
    pub fn new(v: Value) -> Origin {
        Origin(Arc::new(v))
    }
    pub fn value(&self) -> &Value {
        &self.0
    }
}

impl PartialEq for Origin {
    fn eq(&self, other: &Origin) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocShape {
    #[default]
    Single,
    Graph,
    Array,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EditDoc {
    pub shape: DocShape,
    /// The `@graph` wrapper object (Graph) or the whole top-level array
    /// (Array) a JSON-LD reader built the document from. Hosts with their
    /// own encoding leave it `None`.
    pub origin: Option<Origin>,
    pub policies: Vec<PolicyNode>,
    /// Reader notes (unknown keys, skipped nodes).
    pub warnings: Vec<String>,
    next_id: u32,
}

impl EditDoc {
    pub fn new(policies: Vec<PolicyNode>) -> EditDoc {
        let mut doc = EditDoc {
            policies,
            ..EditDoc::default()
        };
        doc.renumber();
        doc
    }

    /// Give every node an id: depth-first, in document order, 1..=n.
    pub fn renumber(&mut self) {
        let mut n = Numberer {
            next: 1,
            force: true,
        };
        for p in &mut self.policies {
            number_policy(p, &mut n);
        }
        self.next_id = n.next;
    }

    /// Number only the nodes that have no id yet (new nodes), keeping all
    /// existing ids.
    pub(crate) fn number_unset(&mut self) {
        let mut n = Numberer {
            next: self.next_id.max(1),
            force: false,
        };
        for p in &mut self.policies {
            number_policy(p, &mut n);
        }
        self.next_id = n.next;
    }

    pub(crate) fn fresh_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id.max(1));
        self.next_id = id.0 + 1;
        id
    }

    /// Structure only: ids zeroed, origins dropped, warnings cleared. Two
    /// documents with the same meaning compare equal after this.
    pub fn normalized(&self) -> EditDoc {
        let mut d = self.clone();
        d.origin = None;
        d.warnings.clear();
        d.next_id = 0;
        for p in &mut d.policies {
            p.normalize();
        }
        d
    }

    pub fn node(&self, p: &NodePath) -> Option<NodeRef<'_>> {
        path::resolve(self, p)
    }

    /// Apply one event. Atomic: on `Err` the document is unchanged.
    pub fn apply(&mut self, ev: &EditEvent, rules: &EditRules) -> Result<(), EditError> {
        let mut work = self.clone();
        super::apply::apply_event(&mut work, ev, rules)?;
        *self = work;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NodeRef<'a> {
    Policy(&'a PolicyNode),
    Rule(&'a RuleNode),
    Entity(&'a Entity),
    Action(&'a ActionNode),
    Constraint(&'a ConstraintNode),
}

impl NodeRef<'_> {
    pub fn id(&self) -> NodeId {
        match self {
            NodeRef::Policy(n) => n.id,
            NodeRef::Rule(n) => n.id,
            NodeRef::Entity(n) => n.id,
            NodeRef::Action(n) => n.id,
            NodeRef::Constraint(n) => n.id(),
        }
    }

    pub fn locked(&self) -> Option<&str> {
        match self {
            NodeRef::Policy(n) => n.locked.as_deref(),
            NodeRef::Rule(n) => n.locked.as_deref(),
            NodeRef::Entity(n) => n.locked.as_deref(),
            NodeRef::Action(n) => n.locked.as_deref(),
            NodeRef::Constraint(ConstraintNode::Opaque { reason, .. }) => Some(reason),
            NodeRef::Constraint(_) => None,
        }
    }

    pub fn origin(&self) -> Option<&Origin> {
        match self {
            NodeRef::Policy(n) => n.origin.as_ref(),
            NodeRef::Rule(n) => n.origin.as_ref(),
            NodeRef::Entity(n) => n.origin.as_ref(),
            NodeRef::Action(n) => n.origin.as_ref(),
            NodeRef::Constraint(n) => n.origin(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PolicyNode {
    pub id: NodeId,
    pub origin: Option<Origin>,
    /// `Some(reason)`: read-only, written back verbatim.
    pub locked: Option<String>,
    /// Raw as written ("Offer", "odrl:Agreement", ...).
    pub kind: String,
    pub uid: Option<String>,
    pub assigner: Vec<Entity>,
    pub assignee: Vec<Entity>,
    pub target: Vec<Entity>,
    pub action: Vec<ActionNode>,
    pub profile: Vec<String>,
    /// Raw; `None` = key absent.
    pub conflict: Option<String>,
    pub inherit_from: Vec<String>,
    pub permission: Vec<RuleNode>,
    pub prohibition: Vec<RuleNode>,
    pub obligation: Vec<RuleNode>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RuleNode {
    pub id: NodeId,
    pub origin: Option<Origin>,
    pub locked: Option<String>,
    /// A rule given only as an IRI string.
    pub reference: Option<String>,
    pub uid: Option<String>,
    pub action: Vec<ActionNode>,
    pub target: Vec<Entity>,
    pub assigner: Vec<Entity>,
    pub assignee: Vec<Entity>,
    pub constraint: Vec<ConstraintNode>,
    pub duty: Vec<RuleNode>,
    pub remedy: Vec<RuleNode>,
    pub consequence: Vec<RuleNode>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActionNode {
    pub id: NodeId,
    pub origin: Option<Origin>,
    pub locked: Option<String>,
    /// Raw, never humanised.
    pub name: String,
    pub refinement: Vec<ConstraintNode>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entity {
    pub id: NodeId,
    pub origin: Option<Origin>,
    pub locked: Option<String>,
    pub iri: Option<String>,
    pub part_of: Option<String>,
    pub refinement: Vec<ConstraintNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintNode {
    Atomic(AtomicConstraint),
    Logical(LogicalConstraint),
    /// A constraint given as an IRI string.
    Reference {
        id: NodeId,
        origin: Option<Origin>,
        iri: String,
    },
    /// Not modelled: read-only, written back verbatim.
    Opaque {
        id: NodeId,
        origin: Origin,
        reason: String,
    },
}

impl Default for ConstraintNode {
    fn default() -> Self {
        ConstraintNode::Atomic(AtomicConstraint::default())
    }
}

impl ConstraintNode {
    pub fn id(&self) -> NodeId {
        match self {
            ConstraintNode::Atomic(a) => a.id,
            ConstraintNode::Logical(l) => l.id,
            ConstraintNode::Reference { id, .. } | ConstraintNode::Opaque { id, .. } => *id,
        }
    }

    pub fn origin(&self) -> Option<&Origin> {
        match self {
            ConstraintNode::Atomic(a) => a.origin.as_ref(),
            ConstraintNode::Logical(l) => l.origin.as_ref(),
            ConstraintNode::Reference { origin, .. } => origin.as_ref(),
            ConstraintNode::Opaque { origin, .. } => Some(origin),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AtomicConstraint {
    pub id: NodeId,
    pub origin: Option<Origin>,
    pub left: String,
    /// Raw.
    pub operator: String,
    pub right: RightOperand,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LogicalConstraint {
    pub id: NodeId,
    pub origin: Option<Origin>,
    pub op: LogicalOp,
    pub children: Vec<ConstraintNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LogicalOp {
    #[default]
    And,
    Or,
    Xone,
    AndSequence,
}

impl LogicalOp {
    pub const ALL: [LogicalOp; 4] = [
        LogicalOp::And,
        LogicalOp::Or,
        LogicalOp::Xone,
        LogicalOp::AndSequence,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            LogicalOp::And => "and",
            LogicalOp::Or => "or",
            LogicalOp::Xone => "xone",
            LogicalOp::AndSequence => "andSequence",
        }
    }

    pub fn parse(term: &str) -> Option<LogicalOp> {
        let local = crate::words::local_name(term);
        LogicalOp::ALL.into_iter().find(|o| o.as_str() == local)
    }

    pub fn phrase(self) -> &'static str {
        match self {
            LogicalOp::And => "all of the following hold",
            LogicalOp::Or => "at least one of the following holds",
            LogicalOp::Xone => "exactly one of the following holds",
            LogicalOp::AndSequence => "the following hold, in this order",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RightOperand {
    Values(Vec<Literal>),
    Reference(String),
    Missing,
}

impl Default for RightOperand {
    fn default() -> Self {
        RightOperand::Values(vec![Literal::Str(String::new())])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Str(String),
    Num(serde_json::Number),
    Bool(bool),
    Typed { value: String, datatype: String },
    Iri(String),
}

impl Literal {
    /// The text a person edits.
    pub fn raw(&self) -> String {
        match self {
            Literal::Str(s) | Literal::Iri(s) => s.clone(),
            Literal::Typed { value, .. } => value.clone(),
            Literal::Num(n) => n.to_string(),
            Literal::Bool(b) => b.to_string(),
        }
    }

    /// How the text reads inside a sentence (the v0.1 rules).
    pub fn display(&self) -> String {
        match self {
            Literal::Str(s) => fmt_str(s),
            Literal::Num(n) => n.to_string(),
            Literal::Bool(b) => b.to_string(),
            Literal::Typed { value, .. } => value.clone(),
            Literal::Iri(s) => s.clone(),
        }
    }

    /// The literal with new text: the variant is kept when the text still
    /// fits it, otherwise it becomes a plain string.
    pub fn with_text(&self, text: &str) -> Literal {
        match self {
            Literal::Num(_) => match text.parse::<serde_json::Number>() {
                Ok(n) if text.trim() == text => Literal::Num(n),
                _ => Literal::Str(text.to_string()),
            },
            Literal::Bool(_) => match text {
                "true" => Literal::Bool(true),
                "false" => Literal::Bool(false),
                _ => Literal::Str(text.to_string()),
            },
            Literal::Typed { datatype, .. } => Literal::Typed {
                value: text.to_string(),
                datatype: datatype.clone(),
            },
            Literal::Iri(_) => Literal::Iri(text.to_string()),
            Literal::Str(_) => Literal::Str(text.to_string()),
        }
    }
}

// --- numbering ------------------------------------------------------------

struct Numberer {
    next: u32,
    force: bool,
}

impl Numberer {
    fn give(&mut self, id: &mut NodeId) {
        if self.force || id.0 == 0 {
            *id = NodeId(self.next);
            self.next += 1;
        }
    }
}

fn number_entities(v: &mut [Entity], n: &mut Numberer) {
    for e in v {
        n.give(&mut e.id);
        number_constraints(&mut e.refinement, n);
    }
}

fn number_actions(v: &mut [ActionNode], n: &mut Numberer) {
    for a in v {
        n.give(&mut a.id);
        number_constraints(&mut a.refinement, n);
    }
}

fn number_constraints(v: &mut [ConstraintNode], n: &mut Numberer) {
    for c in v {
        match c {
            ConstraintNode::Atomic(a) => n.give(&mut a.id),
            ConstraintNode::Logical(l) => {
                n.give(&mut l.id);
                number_constraints(&mut l.children, n);
            }
            ConstraintNode::Reference { id, .. } | ConstraintNode::Opaque { id, .. } => n.give(id),
        }
    }
}

fn number_rules(v: &mut [RuleNode], n: &mut Numberer) {
    for r in v {
        n.give(&mut r.id);
        number_actions(&mut r.action, n);
        number_entities(&mut r.target, n);
        number_entities(&mut r.assigner, n);
        number_entities(&mut r.assignee, n);
        number_constraints(&mut r.constraint, n);
        number_rules(&mut r.duty, n);
        number_rules(&mut r.remedy, n);
        number_rules(&mut r.consequence, n);
    }
}

fn number_policy(p: &mut PolicyNode, n: &mut Numberer) {
    n.give(&mut p.id);
    number_entities(&mut p.assigner, n);
    number_entities(&mut p.assignee, n);
    number_entities(&mut p.target, n);
    number_actions(&mut p.action, n);
    number_rules(&mut p.permission, n);
    number_rules(&mut p.prohibition, n);
    number_rules(&mut p.obligation, n);
}

// --- normalisation --------------------------------------------------------

pub(crate) fn normalize_constraint(c: &mut ConstraintNode) {
    match c {
        ConstraintNode::Atomic(a) => {
            a.id = NodeId(0);
            a.origin = None;
        }
        ConstraintNode::Logical(l) => {
            l.id = NodeId(0);
            l.origin = None;
            l.children.iter_mut().for_each(normalize_constraint);
        }
        ConstraintNode::Reference { id, origin, .. } => {
            *id = NodeId(0);
            *origin = None;
        }
        ConstraintNode::Opaque { id, origin, .. } => {
            *id = NodeId(0);
            *origin = Origin::new(Value::Null);
        }
    }
}

pub(crate) fn normalize_entity(e: &mut Entity) {
    e.id = NodeId(0);
    e.origin = None;
    e.refinement.iter_mut().for_each(normalize_constraint);
}

pub(crate) fn normalize_action(a: &mut ActionNode) {
    a.id = NodeId(0);
    a.origin = None;
    a.refinement.iter_mut().for_each(normalize_constraint);
}

pub(crate) fn normalize_rule(r: &mut RuleNode) {
    r.id = NodeId(0);
    r.origin = None;
    r.action.iter_mut().for_each(normalize_action);
    for list in [&mut r.target, &mut r.assigner, &mut r.assignee] {
        list.iter_mut().for_each(normalize_entity);
    }
    r.constraint.iter_mut().for_each(normalize_constraint);
    for list in [&mut r.duty, &mut r.remedy, &mut r.consequence] {
        list.iter_mut().for_each(normalize_rule);
    }
}

impl PolicyNode {
    pub(crate) fn normalize(&mut self) {
        self.id = NodeId(0);
        self.origin = None;
        for list in [&mut self.assigner, &mut self.assignee, &mut self.target] {
            list.iter_mut().for_each(normalize_entity);
        }
        self.action.iter_mut().for_each(normalize_action);
        for list in [
            &mut self.permission,
            &mut self.prohibition,
            &mut self.obligation,
        ] {
            list.iter_mut().for_each(normalize_rule);
        }
    }
}
