//! The walk over a document: every term, constraint and literal, checked in
//! document order against the loaded profiles.
use super::lexical::lexical;
use super::{Finding, Mode, Options, RuleId, TermRef};
use crate::edit::*;
use crate::profile::{Prefixes, ProfileVocab, Term, TermKind, Wording};
use crate::read::join_with;
use crate::words::local_name;
use crate::words_ctx::{namespace, profile_name};
use std::collections::{HashMap, HashSet};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// The most profiles that are not loaded one policy's findings name; one
/// more finding counts the rest.
const MAX_NAMED: usize = 20;

/// What the validator works out once per wording.
#[derive(Debug, Default)]
pub(crate) struct Index {
    /// Namespace -> position of the first loaded profile that declares a
    /// term in it. ODRL's own namespace is left out.
    namespaces: HashMap<String, usize>,
}

impl Index {
    pub(crate) fn new(w: &Wording) -> Index {
        let mut namespaces = HashMap::new();
        for (i, p) in w.profiles().enumerate() {
            for t in p.terms() {
                let ns = namespace(&t.iri);
                if ns != ODRL && !namespaces.contains_key(ns) {
                    namespaces.insert(ns.to_string(), i);
                }
            }
        }
        Index { namespaces }
    }
}

/// Every finding about `doc`, in document order.
pub(crate) fn run(doc: &EditDoc, w: &Wording, index: &Index, o: &Options) -> Vec<Finding> {
    // Advisory with no profile says nothing, as check_profile always has.
    if o.mode() == Mode::Advisory && w.is_empty() {
        return Vec::new();
    }
    let mut c = Checker {
        w,
        profiles: w.profiles().collect(),
        index,
        options: o,
        prefixes: prefixes(doc),
        out: Vec::new(),
    };
    for (i, p) in doc.policies.iter().enumerate() {
        c.policy(NodePath::policy(i), p);
    }
    let mut out = c.out;
    for f in &mut out {
        if f.node.is_none() {
            let path = match &f.target {
                IssueTarget::Node(n) => Some(n),
                IssueTarget::Slot(s) => Some(&s.node),
                _ => None,
            };
            f.node = path.and_then(|p| doc.node(p)).map(|n| n.id());
        }
    }
    out
}

struct Checker<'a> {
    w: &'a Wording,
    profiles: Vec<&'a ProfileVocab>,
    index: &'a Index,
    options: &'a Options,
    prefixes: Prefixes,
    out: Vec<Finding>,
}

fn slot(node: &NodePath, field: Field) -> IssueTarget {
    IssueTarget::Slot(SlotPath {
        node: node.clone(),
        field,
    })
}

impl<'a> Checker<'a> {
    /// `written` as an expanded IRI.
    fn expand(&self, written: &str) -> String {
        self.w.expand(&self.prefixes, written)
    }

    /// The loaded term `written` names, and its profile.
    fn resolve(&self, written: &str) -> Option<(&'a Term, &'a ProfileVocab)> {
        self.w.lookup(&self.expand(written))
    }

    /// The loaded profile whose namespace `written` is in, when no loaded
    /// profile declares it.
    fn undeclared_in(&self, written: &str) -> Option<&'a ProfileVocab> {
        if self.resolve(written).is_some() {
            return None;
        }
        let iri = self.expand(written);
        let i = *self.index.namespaces.get(namespace(&iri))?;
        self.profiles.get(i).copied()
    }

    /// Whether the profile `id` is loaded (ODRL's core always is).
    fn covers(&self, id: &str) -> bool {
        self.w.covers(&self.expand(id))
    }

    /// Records a finding of `rule`, at its level for these options; nothing
    /// when the rule is off.
    fn emit(
        &mut self,
        rule: RuleId,
        closed: bool,
        target: IssueTarget,
        message: String,
    ) -> Option<&mut Finding> {
        let severity = match self.options.level_of(rule, closed) {
            super::Level::Off => return None,
            super::Level::Info => Severity::Info,
            super::Level::Warning => Severity::Warning,
            super::Level::Error => Severity::Error,
        };
        self.out.push(Finding::new(rule, severity, target, message));
        self.out.last_mut()
    }

    /// As [`Checker::emit`], about a term written in a slot.
    fn emit_term(
        &mut self,
        rule: RuleId,
        closed: bool,
        node: &NodePath,
        field: Field,
        written: &str,
        message: String,
    ) -> Option<&mut Finding> {
        let iri = self.expand(written);
        let f = self.emit(rule, closed, slot(node, field), message)?;
        f.term = Some(TermRef {
            written: written.to_string(),
            iri,
        });
        Some(f)
    }

    /// A term written in `field` of `node`: deprecated, or (when `what`
    /// names it) undeclared in a loaded profile's namespace.
    fn term(&mut self, node: &NodePath, field: Field, written: &str, what: Option<&str>) {
        if written.is_empty() {
            return;
        }
        if let Some((t, p)) = self.resolve(written) {
            if t.deprecated {
                let msg = format!(
                    "\"{written}\" is deprecated in the profile {}",
                    profile_name(p)
                );
                let profile = p.iri().map(str::to_string);
                if let Some(f) = self.emit_term(
                    RuleId::ProfileDeprecatedTerm,
                    false,
                    node,
                    field,
                    written,
                    msg,
                ) {
                    f.profile = profile;
                }
            }
        } else if let (Some(what), Some(p)) = (what, self.undeclared_in(written)) {
            let msg = format!(
                "the {what} \"{written}\" is not declared by the loaded profile {}",
                profile_name(p)
            );
            let profile = p.iri().map(str::to_string);
            if let Some(f) = self.emit_term(
                RuleId::ProfileUndeclaredInNamespace,
                false,
                node,
                field,
                written,
                msg,
            ) {
                f.profile = profile;
            }
        }
    }

    fn policy(&mut self, path: NodePath, p: &PolicyNode) {
        if p.locked.is_some() {
            return;
        }
        // Each profile that is not loaded once, the first MAX_NAMED by name.
        let mut declared: HashSet<&str> = HashSet::new();
        for id in &p.profile {
            if !id.is_empty()
                && !self.covers(id)
                && declared.insert(id)
                && declared.len() <= MAX_NAMED
            {
                let msg = format!("the policy declares the profile <{id}>, which is not loaded");
                let iri = self.expand(id);
                if let Some(f) = self.emit(
                    RuleId::ProfileNotLoaded,
                    false,
                    IssueTarget::Node(path.clone()),
                    msg,
                ) {
                    f.node = Some(p.id);
                    f.profile = Some(id.clone());
                    f.term = Some(TermRef {
                        written: id.clone(),
                        iri,
                    });
                }
            }
        }
        if declared.len() > MAX_NAMED {
            let msg = format!(
                "the policy declares {} more profiles that are not loaded",
                declared.len() - MAX_NAMED
            );
            if let Some(f) = self.emit(
                RuleId::ProfileNotLoaded,
                false,
                IssueTarget::Node(path.clone()),
                msg,
            ) {
                f.node = Some(p.id);
            }
        }
        self.term(&path, Field::Kind, &p.kind, None);
        if let Some(c) = &p.conflict {
            self.term(&path, Field::Conflict, c, None);
        }
        self.entities(&path, &p.assigner, &p.assignee, &p.target);
        self.actions(&path, &p.action);
        for (list, rules) in [
            (RuleList::Permission, &p.permission),
            (RuleList::Prohibition, &p.prohibition),
            (RuleList::Obligation, &p.obligation),
        ] {
            self.rules(&path, list, rules);
        }
    }

    fn rules(&mut self, owner: &NodePath, list: RuleList, rules: &[RuleNode]) {
        for (i, r) in rules.iter().enumerate() {
            if r.locked.is_some() {
                continue;
            }
            let path = owner.child(Step::Rule(list, i));
            self.entities(&path, &r.assigner, &r.assignee, &r.target);
            self.actions(&path, &r.action);
            self.constraints(&path, Step::Constraint, &r.constraint);
            for (list, rules) in [
                (RuleList::Duty, &r.duty),
                (RuleList::Remedy, &r.remedy),
                (RuleList::Consequence, &r.consequence),
            ] {
                self.rules(&path, list, rules);
            }
        }
    }

    fn entities(
        &mut self,
        owner: &NodePath,
        assigner: &[Entity],
        assignee: &[Entity],
        target: &[Entity],
    ) {
        for (role, list) in [
            (EntityRole::Assigner, assigner),
            (EntityRole::Assignee, assignee),
            (EntityRole::Target, target),
        ] {
            for (i, e) in list.iter().enumerate() {
                if e.locked.is_none() {
                    let path = owner.child(Step::Entity(role, i));
                    self.constraints(&path, Step::Refinement, &e.refinement);
                }
            }
        }
    }

    fn actions(&mut self, owner: &NodePath, actions: &[ActionNode]) {
        for (i, a) in actions.iter().enumerate() {
            if a.locked.is_some() {
                continue;
            }
            let path = owner.child(Step::Action(i));
            self.term(&path, Field::Name, &a.name, Some("action"));
            self.constraints(&path, Step::Refinement, &a.refinement);
        }
    }

    fn constraints(&mut self, owner: &NodePath, step: fn(usize) -> Step, list: &[ConstraintNode]) {
        for (i, c) in list.iter().enumerate() {
            let path = owner.child(step(i));
            match c {
                ConstraintNode::Atomic(a) => self.atomic(&path, a),
                ConstraintNode::Logical(l) => self.constraints(&path, Step::Child, &l.children),
                ConstraintNode::Reference { .. } | ConstraintNode::Opaque { .. } => {}
            }
        }
    }

    fn atomic(&mut self, path: &NodePath, a: &AtomicConstraint) {
        self.term(path, Field::LeftOperand, &a.left, Some("left operand"));
        self.term(path, Field::Operator, &a.operator, Some("operator"));
        if let RightOperand::Values(values) = &a.right {
            for (k, l) in values.iter().enumerate() {
                if let Literal::Iri(i) = l {
                    self.term(path, Field::RightOperand(k), i, Some("right operand"));
                }
            }
        }
        if let Some(u) = &a.unit {
            self.term(path, Field::Unit, u, None);
        }
        // The left operand's ds-prose hints, when a loaded profile has any.
        let Some(hints) = self
            .resolve(&a.left)
            .filter(|(t, _)| t.kinds.contains(&TermKind::LeftOperand))
            .map(|(t, _)| &t.hints)
        else {
            return;
        };
        if !a.operator.is_empty()
            && !hints.operators.is_empty()
            && !hints.operators.contains(&self.expand(&a.operator))
        {
            let names: Vec<String> = hints
                .operators
                .iter()
                .map(|o| local_name(o).to_string())
                .collect();
            let msg = format!(
                "the profile suggests the operators {} for \"{}\"",
                join_with(&names, "or"),
                a.left
            );
            let op = a.operator.clone();
            self.emit_term(
                RuleId::ProseOperatorNotAllowed,
                false,
                path,
                Field::Operator,
                &op,
                msg,
            );
        }
        let Some((fits, noun)) = hints
            .datatype
            .as_deref()
            .and_then(|d| d.strip_prefix(XSD))
            .and_then(lexical)
        else {
            return;
        };
        if let RightOperand::Values(values) = &a.right {
            for (k, l) in values.iter().enumerate() {
                if matches!(l, Literal::Iri(_)) {
                    continue;
                }
                let raw = l.raw();
                if !fits(&raw) {
                    let msg = format!(
                        "\"{raw}\" is not {noun}, which the profile expects for \"{}\"",
                        a.left
                    );
                    self.emit(
                        RuleId::ProseDatatypeLexical,
                        false,
                        slot(path, Field::RightOperand(k)),
                        msg,
                    );
                }
            }
        }
    }
}
