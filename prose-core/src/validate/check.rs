//! The walk over a document: every term, constraint and literal, checked in
//! document order against the loaded profiles.
use super::inherit;
use super::lexical::lexical;
use super::{Expected, Finding, Fix, Level, Mode, Options, RuleId, TermRef};
use crate::edit::*;
use crate::profile::{
    OdrlVocab, OperandHints, Prefixes, ProfileVocab, Term, TermKind, Wording, is_odrl_context,
    odrl_term,
};
use crate::read::join_with;
use crate::words::local_name;
use crate::words_ctx::{namespace, profile_name};
use std::collections::{HashMap, HashSet};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";

/// Whether `iri` is one of ODRL's own terms: in its namespace, or one of
/// the Creative Commons actions its Common Vocabulary lists. Such a term
/// needs no profile declaration, whoever redeclares it.
fn odrl_owned(iri: &str) -> bool {
    iri.starts_with(ODRL) || odrl_term(iri, OdrlVocab::Common).is_some()
}
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

#[cfg(test)]
thread_local! {
    static INDEX_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many indexes this thread has built: a test counts them.
#[cfg(test)]
pub(crate) fn index_builds() -> usize {
    INDEX_BUILDS.with(|n| n.get())
}

impl Index {
    pub(crate) fn new(w: &Wording) -> Index {
        #[cfg(test)]
        INDEX_BUILDS.with(|n| n.set(n.get() + 1));
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
    // Advisory with no profile says nothing, as check_profile always has,
    // unless a level set by hand asks for something.
    if o.mode() == Mode::Advisory && w.is_empty() && o.levels.is_empty() {
        return Vec::new();
    }
    let mut c = Checker::new(doc, w, index, o);
    if [
        RuleId::OdrlInheritanceCycle,
        RuleId::OdrlAgreementParties,
        RuleId::OdrlOfferAssigner,
        RuleId::OdrlRuleTarget,
        RuleId::OdrlRuleAction,
    ]
    .iter()
    .any(|r| o.level_of(*r, false) != Level::Off)
    {
        c.inheritance(doc);
    }
    c.imports();
    c.contexts(doc);
    c.unread_keys(doc);
    for note in &doc.warnings {
        c.emit(
            RuleId::InputReaderNote,
            false,
            IssueTarget::Doc,
            note.clone(),
        );
    }
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

/// Whether `ev` may be applied under strict mode's reducer gate: `Err`
/// with the finding when it writes a term into a vocabulary slot that a
/// refusable rule reports as an error there.
///
/// Slot-local: only the written value is checked, never what depends on
/// it, so a new left operand is not refused for the operator or values the
/// constraint already has. Everything else is admitted: the empty string,
/// free text, right operands, structural events, an event on a path that
/// is not there (the reducer refuses it), every event in advisory mode, and
/// every event while the policy's vocabulary is incomplete.
pub(crate) fn admissible(
    doc: &EditDoc,
    w: &Wording,
    index: &Index,
    o: &Options,
    ev: &EditEvent,
) -> Result<(), Box<Finding>> {
    if o.mode() != Mode::Strict {
        return Ok(());
    }
    let (slot, value) = match ev {
        EditEvent::SetText { slot, value, .. } | EditEvent::SetChoice { slot, value, .. } => {
            (slot, value.as_str())
        }
        _ => return Ok(()),
    };
    if value.is_empty() {
        return Ok(());
    }
    let pos = match slot.field {
        Field::Kind => Pos::PolicyKind,
        Field::Conflict => Pos::Conflict,
        Field::Name => Pos::Action,
        Field::LeftOperand => Pos::LeftOperand,
        Field::Operator => Pos::Operator,
        Field::Unit => Pos::Unit,
        _ => return Ok(()),
    };
    let Some(policy) = doc.policies.get(slot.node.policy) else {
        return Ok(());
    };
    let atomic = match (pos, doc.node(&slot.node)) {
        (Pos::PolicyKind | Pos::Conflict, Some(NodeRef::Policy(_))) => None,
        (Pos::Action, Some(NodeRef::Action(_))) => None,
        (
            Pos::LeftOperand | Pos::Operator | Pos::Unit,
            Some(NodeRef::Constraint(ConstraintNode::Atomic(a))),
        ) => Some(a),
        _ => return Ok(()),
    };
    if policy.locked.is_some() {
        return Ok(());
    }
    let mut c = Checker::new(doc, w, index, o);
    c.imports();
    c.contexts(doc);
    c.enter(NodePath::policy(slot.node.policy), policy);
    if c.incomplete() {
        return Ok(());
    }
    c.out.clear();
    let id = doc.node(&slot.node).map(|n| n.id()).unwrap_or_default();
    c.term(&slot.node, id, slot.field, value, pos);
    // The left operand's hints bind the operator and the unit written
    // under it; they are read as the constraint stands, with the new value.
    if let Some(a) = atomic
        && matches!(pos, Pos::Operator | Pos::Unit)
        && let Some(h) = c
            .resolve(&a.left)
            .filter(|(t, _)| t.kinds.contains(&TermKind::LeftOperand))
            .map(|(t, _)| &t.hints)
    {
        let mut written = a.clone();
        if pos == Pos::Operator {
            written.operator = value.to_string();
            c.hint_operator(&slot.node, &written, h);
        } else {
            written.unit = Some(value.to_string());
            c.hint_unit(&slot.node, &written, h);
        }
    }
    let here = IssueTarget::Slot(slot.clone());
    match c.out.into_iter().find(|f| {
        f.severity == Severity::Error && !f.gap && f.rule.info().refusable && f.target == here
    }) {
        Some(f) => Err(Box::new(f)),
        None => Ok(()),
    }
}

struct Checker<'a> {
    w: &'a Wording,
    profiles: Vec<&'a ProfileVocab>,
    index: &'a Index,
    options: &'a Options,
    prefixes: Prefixes,
    /// The policy being checked.
    cur: Option<PolicyCtx>,
    /// A loaded profile builds on one that is not loaded: no policy's
    /// vocabulary is complete.
    unsure: bool,
    /// The document has a `@context` the reader does not apply (remote,
    /// imported or nested), which may bind any prefix or term: no policy's
    /// terms read as they would in JSON-LD for certain.
    context_unread: bool,
    /// Per policy: on an inheritance cycle within the document. Empty when
    /// the structural tier is off.
    cycles: Vec<bool>,
    /// Per policy: the parents it names that are not in the document.
    remote: Vec<Vec<String>>,
    /// Per policy: the policy-level parts (`INH_*`) it has of its own or
    /// takes from its parents in the document, at any depth (IM 2.9), and
    /// `INH_REMOTE` when it or an ancestor names a parent outside the
    /// document. Empty when the structural tier is off.
    inherited: Vec<u8>,
    out: Vec<Finding>,
}

const INH_ASSIGNER: u8 = 1;
const INH_ASSIGNEE: u8 = 2;
const INH_TARGET: u8 = 4;
const INH_ACTION: u8 = 8;
/// An ancestor is outside the document: what it would give is unknown.
const INH_REMOTE: u8 = 16;

/// What the terms of one policy are checked against.
struct PolicyCtx {
    path: NodePath,
    id: NodeId,
    /// How many profiles the policy lists.
    listed: usize,
    /// The profiles it declares, expanded, without a trailing `#` or `/`.
    declared: Vec<String>,
    /// A declared profile is not loaded.
    incomplete: bool,
    /// The ODRL policy class the policy's type is, or is a subclass of.
    class: Class,
    /// Policy-level properties, shared by every rule (IM 2.7.1).
    assigner: bool,
    assignee: bool,
    target: bool,
    action: bool,
    /// Whatever the rules lack may come from a parent outside the document.
    remote_parent: bool,
}

/// The policy classes with structural requirements of their own.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Offer,
    Agreement,
    Other,
}

/// Where a term is written, and so which kind of term belongs there.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pos {
    PolicyKind,
    Conflict,
    Action,
    LeftOperand,
    Operator,
    /// An IRI right operand value: data as much as vocabulary.
    Value,
    Unit,
}

impl Pos {
    /// How `check_profile` named the position in its "not declared"
    /// message; it said nothing about the others.
    fn what(self) -> Option<&'static str> {
        match self {
            Pos::Action => Some("action"),
            Pos::LeftOperand => Some("left operand"),
            Pos::Operator => Some("operator"),
            Pos::Value => Some("right operand"),
            _ => None,
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Pos::PolicyKind => "policy type",
            Pos::Conflict => "conflict strategy",
            Pos::Action => "action",
            Pos::LeftOperand => "left operand",
            Pos::Operator => "operator",
            Pos::Value => "right operand",
            Pos::Unit => "unit",
        }
    }

    /// The kind a term written here must have (the `rdfs:range` of the
    /// ODRL property); `None` where any IRI may stand.
    fn kind(self) -> Option<TermKind> {
        match self {
            Pos::PolicyKind => Some(TermKind::PolicyClass),
            Pos::Conflict => Some(TermKind::ConflictTerm),
            Pos::Action => Some(TermKind::Action),
            Pos::LeftOperand => Some(TermKind::LeftOperand),
            Pos::Operator => Some(TermKind::Operator),
            Pos::Value | Pos::Unit => None,
        }
    }
}

/// A kind with its article, for a message.
fn kind_noun(k: TermKind) -> &'static str {
    match k {
        TermKind::Action => "an action",
        TermKind::LeftOperand => "a left operand",
        TermKind::Operator => "an operator",
        TermKind::RightOperand => "a right operand value",
        TermKind::ConflictTerm => "a conflict strategy",
        TermKind::PolicyClass => "a policy type",
        TermKind::RuleClass => "a rule type",
        TermKind::AssetClass => "an asset class",
        TermKind::PartyClass => "a party class",
        TermKind::PartyFunction => "a party function",
        TermKind::AssetRelation => "an asset relation",
        TermKind::LogicalOperand => "a logical operator",
    }
}

fn trim(iri: &str) -> &str {
    iri.trim_end_matches(['#', '/'])
}

/// A fix that writes `value` into `slot`, with the event that slot takes.
fn fix_slot(label: String, safe: bool, slot: SlotPath, expect: NodeId, value: String) -> Fix {
    let event = match slot.field {
        Field::Operator | Field::Conflict | Field::LogicalOp => EditEvent::SetChoice {
            slot,
            expect,
            value: value.clone(),
        },
        Field::RightOperand(_) => EditEvent::SetIri {
            slot,
            expect,
            value: value.clone(),
        },
        _ => EditEvent::SetText {
            slot,
            expect,
            value: value.clone(),
        },
    };
    Fix {
        label,
        safe,
        replacement: value,
        events: vec![event],
    }
}

/// A `@context` the reader does not apply.
enum Unread {
    /// A remote context, or one an `@import` names.
    Remote(String),
    /// A context on a node below a policy.
    Nested,
}

/// Where a JSON value sits, for which of its `@context`s the reader applies
/// ([`Prefixes::of_document`]): those of a top-level node and of its
/// `@graph` items, nothing below.
#[derive(Clone, Copy, PartialEq)]
enum Depth {
    Top,
    GraphItem,
    Below,
}

/// The `@context`s of `doc`'s JSON that the reader does not apply, each
/// once, in document order. Linear in the size of the JSON; no recursion.
fn unread_contexts(doc: &EditDoc) -> Vec<Unread> {
    let roots: Vec<&serde_json::Value> = match &doc.origin {
        Some(o) => vec![o.value()],
        None => doc
            .policies
            .iter()
            .filter_map(|p| p.origin.as_ref().map(|o| o.value()))
            .collect(),
    };
    let mut todo: Vec<(&serde_json::Value, Depth)> = Vec::new();
    for root in roots {
        match root {
            serde_json::Value::Array(a) => todo.extend(a.iter().map(|v| (v, Depth::Top))),
            other => todo.push((other, Depth::Top)),
        }
    }
    let mut out: Vec<Unread> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut nested = false;
    while let Some((v, depth)) = todo.pop() {
        match v {
            serde_json::Value::Array(a) => todo.extend(a.iter().rev().map(|x| (x, depth))),
            serde_json::Value::Object(o) => {
                for (k, x) in o.iter().rev() {
                    match k.as_str() {
                        "@context" if depth == Depth::Below => {
                            if !nested {
                                nested = true;
                                out.push(Unread::Nested);
                            }
                        }
                        "@context" => {
                            let list: Vec<&serde_json::Value> = match x {
                                serde_json::Value::Array(a) => a.iter().collect(),
                                other => vec![other],
                            };
                            for c in list {
                                let remote = match c {
                                    serde_json::Value::String(s) => {
                                        (!is_odrl_context(s)).then_some(s.as_str())
                                    }
                                    serde_json::Value::Object(d) => {
                                        d.get("@import").and_then(|i| i.as_str())
                                    }
                                    _ => None,
                                };
                                if let Some(iri) = remote
                                    && seen.insert(iri.to_string())
                                {
                                    out.push(Unread::Remote(iri.to_string()));
                                }
                            }
                        }
                        "@graph" if depth == Depth::Top => match x {
                            serde_json::Value::Array(a) => {
                                todo.extend(a.iter().rev().map(|g| (g, Depth::GraphItem)))
                            }
                            other => todo.push((other, Depth::GraphItem)),
                        },
                        // A JSON literal's content is data, not JSON-LD.
                        "@value" => {}
                        _ => todo.push((x, Depth::Below)),
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn slot(node: &NodePath, field: Field) -> IssueTarget {
    IssueTarget::Slot(SlotPath {
        node: node.clone(),
        field,
    })
}

impl<'a> Checker<'a> {
    fn new(doc: &EditDoc, w: &'a Wording, index: &'a Index, o: &'a Options) -> Checker<'a> {
        Checker {
            w,
            profiles: w.profiles().collect(),
            index,
            options: o,
            prefixes: prefixes(doc),
            cur: None,
            unsure: false,
            context_unread: false,
            cycles: Vec::new(),
            remote: Vec::new(),
            inherited: Vec::new(),
            out: Vec::new(),
        }
    }

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
        if !iri.starts_with(ODRL) && odrl_owned(&iri) {
            // ODRL's own, as its Common Vocabulary lists the cc: actions,
            // whichever way a loaded copy of the vocabulary spells them.
            return None;
        }
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
            Level::Off => return None,
            Level::Info => Severity::Info,
            Level::Warning => Severity::Warning,
            Level::Error => Severity::Error,
        };
        let mut f = Finding::new(rule, severity, target, message);
        if rule == RuleId::InputUnchecked && self.options.fail_closed {
            // Fail closed: what could not be checked fails the policy.
            f.gap = false;
        }
        self.out.push(f);
        self.out.last_mut()
    }

    /// As [`Checker::emit`], about a term written in a slot.
    #[allow(clippy::too_many_arguments)]
    fn emit_term(
        &mut self,
        rule: RuleId,
        closed: bool,
        node: &NodePath,
        id: NodeId,
        field: Field,
        written: &str,
        message: String,
    ) -> Option<&mut Finding> {
        let iri = self.expand(written);
        let f = self.emit(rule, closed, slot(node, field), message)?;
        f.node = Some(id);
        f.term = Some(TermRef {
            written: written.to_string(),
            iri,
        });
        Some(f)
    }

    /// A term written in `field` of `node`, at position `pos`.
    ///
    /// Advisory, as `check_profile` always was: a loaded term that is
    /// deprecated, or a term in a loaded profile's namespace that no loaded
    /// profile declares. Strict adds the closed world: a compact IRI only a
    /// profile's prefixes expand, a term ODRL and the loaded profiles do not
    /// define, a term of the wrong kind, a profile term from a profile the
    /// policy does not declare, and ODRL's own deprecated terms.
    fn term(&mut self, node: &NodePath, id: NodeId, field: Field, written: &str, pos: Pos) {
        if written.is_empty() {
            return;
        }
        let resolved = self.resolve(written);
        match resolved {
            Some((t, p)) => {
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
                        id,
                        field,
                        written,
                        msg,
                    ) {
                        f.profile = profile;
                    }
                }
            }
            None => {
                if let (Some(what), Some(p)) = (pos.what(), self.undeclared_in(written)) {
                    // Strict: a declared profile that is missing may
                    // declare it in the same namespace.
                    if self.options.mode() == Mode::Strict && self.incomplete() {
                        return;
                    }
                    let msg = format!(
                        "the {what} \"{written}\" is not declared by the loaded profile {}",
                        profile_name(p)
                    );
                    let profile = p.iri().map(str::to_string);
                    if let Some(f) = self.emit_term(
                        RuleId::ProfileUndeclaredInNamespace,
                        false,
                        node,
                        id,
                        field,
                        written,
                        msg,
                    ) {
                        f.profile = profile;
                    }
                    return;
                }
            }
        }
        // What follows is off in advisory mode unless a level set by hand
        // switches a rule on.
        let iri = self.expand(written);
        // A context that is not read may bind the prefix.
        if let Some(prefix) = self
            .unbound_prefix(written, &iri)
            .filter(|_| !self.context_unread)
        {
            let msg = format!(
                "the policy's @context does not bind the prefix \"{prefix}\" of \"{written}\"; only a loaded profile's own prefixes read it as <{iri}>"
            );
            let fix = fix_slot(
                "Write the full IRI".to_string(),
                true,
                SlotPath {
                    node: node.clone(),
                    field,
                },
                id,
                iri.clone(),
            );
            if let Some(f) = self.emit_term(
                RuleId::ProfileUnboundPrefix,
                false,
                node,
                id,
                field,
                written,
                msg,
            ) {
                f.fixes.push(fix);
            }
            return;
        }
        // While a declared profile is missing, an unknown term may be one of
        // its own: say nothing rather than guess.
        let unsure = self.incomplete();
        let odrl = odrl_term(&iri, self.options.odrl());
        // Every loaded profile that declares the term, first loaded first:
        // the term is every kind any of them gives it.
        let declaring: Vec<&'a ProfileVocab> = match resolved {
            Some((t, _)) => self
                .profiles
                .iter()
                .copied()
                .filter(|q| q.term(&t.iri).is_some())
                .collect(),
            None => Vec::new(),
        };
        let kinds: Vec<TermKind> = declaring
            .iter()
            .filter_map(|q| q.term(&iri))
            .flat_map(|t| t.kinds.iter().copied())
            .chain(odrl.map(|o| o.kind))
            .collect();
        if kinds.is_empty() {
            match pos {
                Pos::Value | Pos::Unit => {}
                _ if unsure => {}
                Pos::Conflict => self.not_a_conflict_strategy(node, id, written),
                _ => {
                    let source = match self.options.odrl() {
                        OdrlVocab::Core => "the ODRL core vocabulary",
                        _ => "ODRL",
                    };
                    let msg = format!(
                        "the {} \"{written}\" is not defined by {source} or by a loaded profile",
                        pos.noun()
                    );
                    let expected = pos.kind();
                    if let Some(f) = self.emit_term(
                        RuleId::ProfileUnknownTerm,
                        false,
                        node,
                        id,
                        field,
                        written,
                        msg,
                    ) {
                        f.expected = expected.map(|k| Expected {
                            kinds: vec![k],
                            ..Expected::default()
                        });
                    }
                }
            }
            return;
        }
        if let Some(want) = pos.kind()
            && !kinds.contains(&want)
        {
            if unsure {
                return;
            }
            if pos == Pos::Conflict {
                self.not_a_conflict_strategy(node, id, written);
                return;
            }
            let msg = format!(
                "\"{written}\" is {}, not {}",
                kind_noun(kinds[0]),
                kind_noun(want)
            );
            if let Some(f) = self.emit_term(
                RuleId::ProfileWrongKind,
                false,
                node,
                id,
                field,
                written,
                msg,
            ) {
                f.expected = Some(Expected {
                    kinds: vec![want],
                    ..Expected::default()
                });
            }
            return;
        }
        match resolved {
            Some((t, p)) => {
                // A profile's term: the policy must name a profile that
                // declares it (IM 2.1, 3.2); any one of them will do. ODRL's
                // own terms need no declaration, whoever redeclares them, and
                // a profile without an IRI cannot be named.
                if let Some(piri) = p.iri()
                    && !odrl_owned(&t.iri)
                    && !unsure
                    && !declaring
                        .iter()
                        .any(|q| q.iri().is_none_or(|i| self.declares(i)))
                {
                    let msg = format!(
                        "\"{written}\" comes from the profile <{piri}>, which the policy does not declare"
                    );
                    let fix = self.declare_fix(piri);
                    let piri = piri.to_string();
                    if let Some(f) = self.emit_term(
                        RuleId::ProfileNotDeclared,
                        false,
                        node,
                        id,
                        field,
                        written,
                        msg,
                    ) {
                        f.profile = Some(piri);
                        f.fixes.extend(fix);
                    }
                }
            }
            None => {
                // Advisory mode never reads the ODRL table.
                if self.options.mode() == Mode::Strict && odrl.is_some_and(|o| o.deprecated) {
                    let msg = format!("\"{written}\" is deprecated in ODRL 2.2");
                    self.emit_term(
                        RuleId::ProfileDeprecatedTerm,
                        false,
                        node,
                        id,
                        field,
                        written,
                        msg,
                    );
                }
            }
        }
    }

    fn not_a_conflict_strategy(&mut self, node: &NodePath, id: NodeId, written: &str) {
        let msg = format!(
            "\"{written}\" is not a conflict strategy: use perm, prohibit or invalid, or a conflict term a loaded profile declares"
        );
        if let Some(f) = self.emit_term(
            RuleId::ProfileConflictTerm,
            false,
            node,
            id,
            Field::Conflict,
            written,
            msg,
        ) {
            f.expected = Some(Expected {
                kinds: vec![TermKind::ConflictTerm],
                ..Expected::default()
            });
        }
    }

    /// The prefix of `written` when only a loaded profile's own prefixes
    /// expand it (to `iri`): the policy's `@context` does not bind it, so a
    /// JSON-LD processor reads `written` as an IRI of its own.
    fn unbound_prefix<'w>(&self, written: &'w str, iri: &str) -> Option<&'w str> {
        let (prefix, suffix) = written.split_once(':')?;
        if prefix == "_" || suffix.starts_with("//") || iri == written {
            return None;
        }
        self.prefixes
            .try_expand(written)
            .is_none()
            .then_some(prefix)
    }

    /// Whether the policy being checked has a declared profile it could
    /// not load, so its vocabulary is not complete.
    fn incomplete(&self) -> bool {
        self.unsure || self.cur.as_ref().is_some_and(|c| c.incomplete)
    }

    /// The profiles the loaded ones build on (`owl:imports`,
    /// `prof:isProfileOf`) that are not loaded: recorded, never fetched.
    /// ODRL itself is always there.
    fn imports(&mut self) {
        let mut seen: HashSet<(&str, &str)> = HashSet::new();
        for p in self.profiles.clone() {
            for i in p.imports() {
                if self.w.covers(i) || !seen.insert((p.iri().unwrap_or(""), i)) {
                    continue;
                }
                self.unsure = true;
                let msg = format!(
                    "the loaded profile {} builds on <{i}>, which is not loaded",
                    profile_name(p)
                );
                if let Some(f) =
                    self.emit(RuleId::ProfileImportNotLoaded, false, IssueTarget::Doc, msg)
                {
                    f.profile = Some(i.clone());
                    f.term = Some(TermRef {
                        written: i.clone(),
                        iri: i.clone(),
                    });
                }
            }
        }
    }

    /// The `@context`s the reader does not apply: a remote one other than
    /// ODRL's, an `@import`, or one on a node below a policy. Each may bind
    /// any prefix or term, so the terms are not read for certain as a
    /// JSON-LD processor reads them: a gap, never fetched.
    fn contexts(&mut self, doc: &EditDoc) {
        let found = unread_contexts(doc);
        if found.is_empty() {
            return;
        }
        self.unsure = true;
        self.context_unread = true;
        for what in found.iter().take(MAX_NAMED) {
            let (msg, term) = match what {
                Unread::Remote(iri) => (
                    format!(
                        "the @context <{iri}> is not read, so the prefixes and terms it may define could not be checked"
                    ),
                    Some(TermRef {
                        written: iri.clone(),
                        iri: iri.clone(),
                    }),
                ),
                Unread::Nested => (
                    "a @context inside the policy is not read, so the prefixes and terms it may define could not be checked"
                        .to_string(),
                    None,
                ),
            };
            if let Some(f) = self.emit(RuleId::InputUnchecked, false, IssueTarget::Doc, msg) {
                f.term = term;
            }
        }
        if found.len() > MAX_NAMED {
            let msg = format!(
                "{} more @contexts are not read, so the prefixes and terms they may define could not be checked",
                found.len() - MAX_NAMED
            );
            self.emit(RuleId::InputUnchecked, false, IssueTarget::Doc, msg);
        }
    }

    /// ODRL property keys the reader skipped (compact or full IRIs, @context
    /// aliases): nothing under them was read, so none of it was checked.
    fn unread_keys(&mut self, doc: &EditDoc) {
        for k in doc.unread.iter().take(MAX_NAMED) {
            let msg = format!(
                "the ODRL property \"{k}\" is not read (only bare property keys are), so what it holds could not be checked"
            );
            if let Some(f) = self.emit(RuleId::InputUnchecked, false, IssueTarget::Doc, msg) {
                f.term = Some(TermRef {
                    written: k.clone(),
                    iri: k.clone(),
                });
            }
        }
        if doc.unread.len() > MAX_NAMED {
            let msg = format!(
                "{} more ODRL properties are not read, so what they hold could not be checked",
                doc.unread.len() - MAX_NAMED
            );
            self.emit(RuleId::InputUnchecked, false, IssueTarget::Doc, msg);
        }
    }

    /// Something the reader kept as written, or only a reference to: it
    /// could not be checked.
    fn unchecked(&mut self, target: IssueTarget, id: NodeId, message: String) {
        if let Some(f) = self.emit(RuleId::InputUnchecked, false, target, message) {
            f.node = Some(id);
        }
    }

    /// Whether the policy being checked names the profile `iri`.
    fn declares(&self, iri: &str) -> bool {
        let want = trim(iri);
        self.cur
            .as_ref()
            .is_some_and(|c| c.declared.iter().any(|d| d == want))
    }

    /// Adding `iri` to the profile list of the policy being checked.
    fn declare_fix(&self, iri: &str) -> Option<Fix> {
        let c = self.cur.as_ref()?;
        let list = ListPath::of(&c.path, ListKind::Profile);
        Some(Fix {
            label: format!("Declare the profile <{iri}>"),
            safe: true,
            replacement: iri.to_string(),
            events: vec![
                EditEvent::Add {
                    list,
                    index: c.listed,
                    expect: Some(c.id),
                    item: NewItem::Default,
                },
                EditEvent::SetText {
                    slot: SlotPath {
                        node: c.path.clone(),
                        field: Field::Profile(c.listed),
                    },
                    expect: c.id,
                    value: iri.to_string(),
                },
            ],
        })
    }

    fn policy(&mut self, path: NodePath, p: &PolicyNode) {
        if let Some(reason) = &p.locked {
            let msg = format!("this policy could not be checked: {reason}");
            self.unchecked(IssueTarget::Node(path), p.id, msg);
            return;
        }
        self.enter(path.clone(), p);
        self.policy_structure(&path, p);
        self.term(&path, p.id, Field::Kind, &p.kind, Pos::PolicyKind);
        if let Some(c) = &p.conflict {
            self.term(&path, p.id, Field::Conflict, c, Pos::Conflict);
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
        self.cur = None;
    }

    /// Makes the unlocked policy at `path` the one being checked: reports
    /// the profiles it declares that are not loaded, and records what its
    /// terms and rules are checked against.
    fn enter(&mut self, path: NodePath, p: &PolicyNode) {
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
        // Own parts, plus a parent's when the document has it (IM 2.9).
        let own = [
            (!p.assigner.is_empty(), INH_ASSIGNER),
            (!p.assignee.is_empty(), INH_ASSIGNEE),
            (!p.target.is_empty(), INH_TARGET),
            (!p.action.is_empty(), INH_ACTION),
        ]
        .iter()
        .fold(0u8, |a, (on, bit)| if *on { a | bit } else { a });
        let inh = own | self.inherited.get(path.policy).copied().unwrap_or(0);
        self.cur = Some(PolicyCtx {
            path: path.clone(),
            id: p.id,
            listed: p.profile.len(),
            declared: p
                .profile
                .iter()
                .map(|d| trim(&self.expand(d)).to_string())
                .collect(),
            incomplete: !declared.is_empty(),
            class: self.class_of(&p.kind),
            assigner: inh & INH_ASSIGNER != 0,
            assignee: inh & INH_ASSIGNEE != 0,
            target: inh & INH_TARGET != 0,
            action: inh & INH_ACTION != 0,
            remote_parent: inh & INH_REMOTE != 0,
        });
    }

    /// Works out, once, which policies inherit from themselves and which
    /// parents are not in the document.
    fn inheritance(&mut self, doc: &EditDoc) {
        let mut by_uid: HashMap<String, usize> = HashMap::new();
        for (i, p) in doc.policies.iter().enumerate() {
            if p.locked.is_none()
                && let Some(uid) = p.uid.as_deref().filter(|u| !u.is_empty())
            {
                by_uid.entry(self.expand(uid)).or_insert(i);
            }
        }
        let mut parents: Vec<Vec<usize>> = vec![Vec::new(); doc.policies.len()];
        self.remote = vec![Vec::new(); doc.policies.len()];
        for (i, p) in doc.policies.iter().enumerate() {
            if p.locked.is_some() {
                continue;
            }
            for parent in p.inherit_from.iter().filter(|x| !x.is_empty()) {
                match by_uid.get(&self.expand(parent)) {
                    Some(&j) => parents[i].push(j),
                    None => self.remote[i].push(parent.clone()),
                }
            }
        }
        self.cycles = inherit::on_cycle(&parents);
        // Push the parts down the links to a fixpoint. Each policy's value
        // gains at most five bits, so this ends on a cycle too.
        let mut eff: Vec<u8> = doc
            .policies
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let own = [
                    (!p.assigner.is_empty(), INH_ASSIGNER),
                    (!p.assignee.is_empty(), INH_ASSIGNEE),
                    (!p.target.is_empty(), INH_TARGET),
                    (!p.action.is_empty(), INH_ACTION),
                    (!self.remote[i].is_empty(), INH_REMOTE),
                ];
                own.iter()
                    .fold(0u8, |a, (on, bit)| if *on { a | bit } else { a })
            })
            .collect();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); parents.len()];
        for (i, ps) in parents.iter().enumerate() {
            for &j in ps {
                children[j].push(i);
            }
        }
        let mut todo: Vec<usize> = (0..eff.len()).collect();
        while let Some(v) = todo.pop() {
            for &c in &children[v] {
                let next = eff[c] | eff[v];
                if next != eff[c] {
                    eff[c] = next;
                    todo.push(c);
                }
            }
        }
        self.inherited = eff;
    }

    /// The policy's own requirements (IM 2.1, 2.9).
    fn policy_structure(&mut self, path: &NodePath, p: &PolicyNode) {
        let node = || IssueTarget::Node(path.clone());
        if p.uid.as_deref().is_none_or(str::is_empty) {
            self.emit(
                RuleId::OdrlPolicyUid,
                false,
                node(),
                "the policy has no uid".into(),
            );
        }
        // A child policy may take all its rules from its parents.
        if p.permission.is_empty()
            && p.prohibition.is_empty()
            && p.obligation.is_empty()
            && p.inherit_from.is_empty()
        {
            let msg = "the policy has no permission, prohibition or obligation".to_string();
            self.emit(RuleId::OdrlNoRule, false, node(), msg);
        }
        if self.cycles.get(path.policy).copied().unwrap_or(false) {
            let msg = format!(
                "the policy <{}> inherits from itself through inheritFrom",
                p.uid.as_deref().unwrap_or("")
            );
            self.emit(RuleId::OdrlInheritanceCycle, false, node(), msg);
        }
        // Filled only when a rule that reads inheritance is on. Whichever it
        // is, it stays quiet on what the parent may supply, so the gap is
        // reported whether or not the cycle rule is on.
        let remote = self.remote.get(path.policy).cloned().unwrap_or_default();
        for parent in remote {
            let msg = format!(
                "the parent policy <{parent}> is not in this document, so the inheritance could not be checked"
            );
            self.unchecked(node(), p.id, msg);
        }
    }

    /// A rule's own requirements, with the policy's shared properties
    /// pushed down (IM 2.1.2, 2.1.3, 2.6, 2.6.1-2, 2.6.3, 2.6.7, 2.7.1).
    fn rule_structure(&mut self, path: &NodePath, list: RuleList, r: &RuleNode) {
        let Some(c) = self.cur.as_ref() else {
            return;
        };
        // A part missing here may be supplied by a parent that is not in the
        // document: that is the reported gap, not an error.
        let unknown = c.remote_parent;
        let (class, assigner, assignee, target, action) = (
            c.class,
            !r.assigner.is_empty() || c.assigner || unknown,
            !r.assignee.is_empty() || c.assignee || unknown,
            !r.target.is_empty() || c.target || unknown,
            !r.action.is_empty() || c.action || unknown,
        );
        let node = || IssueTarget::Node(path.clone());
        if list.is_top_level() {
            match class {
                Class::Offer if !assigner => {
                    let msg = "this rule of an Offer has no assigner, on the rule or on the policy";
                    self.emit(RuleId::OdrlOfferAssigner, false, node(), msg.into());
                }
                Class::Agreement if !(assigner && assignee) => {
                    let missing = match (assigner, assignee) {
                        (false, false) => "no assigner and no assignee",
                        (false, true) => "no assigner",
                        _ => "no assignee",
                    };
                    let msg = format!(
                        "this rule of an Agreement has {missing}, on the rule or on the policy"
                    );
                    self.emit(RuleId::OdrlAgreementParties, false, node(), msg);
                }
                _ => {}
            }
        }
        if matches!(list, RuleList::Permission | RuleList::Prohibition) && !target {
            let msg = format!(
                "this {} has no target, on the rule or on the policy",
                list.as_str()
            );
            self.emit(RuleId::OdrlRuleTarget, false, node(), msg);
        }
        if !action {
            let msg = "this rule has no action, on the rule or on the policy";
            self.emit(RuleId::OdrlRuleAction, false, node(), msg.into());
        }
        if !r.consequence.is_empty() {
            let msg = match list {
                RuleList::Consequence => Some("this consequence has a consequence of its own"),
                RuleList::Remedy => Some(
                    "this remedy has a consequence; a remedy must not refer to a duty that has one",
                ),
                _ => None,
            };
            if let Some(msg) = msg {
                self.emit(RuleId::OdrlConsequenceNesting, false, node(), msg.into());
            }
        }
    }

    /// An atomic constraint's shape (IM 2.5.1).
    fn constraint_structure(&mut self, path: &NodePath, a: &AtomicConstraint) {
        if self.options.level_of(RuleId::OdrlConstraintShape, false) == Level::Off
            && self
                .options
                .level_of(RuleId::OdrlSetOperatorCardinality, false)
                == Level::Off
        {
            return;
        }
        let node = || IssueTarget::Node(path.clone());
        let mut missing: Vec<&str> = Vec::new();
        if a.left.is_empty() {
            missing.push("no left operand");
        }
        if a.operator.is_empty() {
            missing.push("no operator");
        }
        let both = a.origin.as_ref().is_some_and(|o| {
            o.value().get("rightOperand").is_some()
                && o.value().get("rightOperandReference").is_some()
        });
        if both {
            missing.push("both a rightOperand and a rightOperandReference");
        } else if a.right == RightOperand::Missing {
            missing.push("no right operand");
        }
        if !missing.is_empty() {
            let msg = format!("this constraint has {}", missing.join(" and "));
            if let Some(f) = self.emit(RuleId::OdrlConstraintShape, false, node(), msg) {
                f.node = Some(a.id);
            }
        }
        if let RightOperand::Values(values) = &a.right
            && values.len() > 1
        {
            let op = self.expand(&a.operator);
            let single = op.strip_prefix(ODRL).is_some_and(|l| {
                matches!(
                    l,
                    "eq" | "neq" | "gt" | "gteq" | "lt" | "lteq" | "isA" | "hasPart" | "isPartOf"
                )
            });
            if single {
                let msg = format!(
                    "\"{}\" compares with one value, but this constraint gives {}; use isAnyOf, isAllOf or isNoneOf for several",
                    a.operator,
                    values.len()
                );
                if let Some(f) = self.emit(RuleId::OdrlSetOperatorCardinality, false, node(), msg) {
                    f.node = Some(a.id);
                }
            }
        }
    }

    /// The ODRL class a policy type is: an Offer or an Agreement, also
    /// through a loaded profile's subclass of one, transitively.
    fn class_of(&self, kind: &str) -> Class {
        let offer = format!("{ODRL}Offer");
        let agreement = format!("{ODRL}Agreement");
        let mut todo = vec![self.expand(kind)];
        let mut seen: HashSet<String> = HashSet::new();
        while let Some(iri) = todo.pop() {
            if iri == offer {
                return Class::Offer;
            }
            if iri == agreement {
                return Class::Agreement;
            }
            if seen.len() >= 64 || !seen.insert(iri.clone()) {
                continue;
            }
            if let Some((t, _)) = self.w.lookup(&iri) {
                todo.extend(t.broader.iter().cloned());
            }
        }
        Class::Other
    }

    fn rules(&mut self, owner: &NodePath, list: RuleList, rules: &[RuleNode]) {
        for (i, r) in rules.iter().enumerate() {
            let path = owner.child(Step::Rule(list, i));
            if let Some(reason) = &r.locked {
                let msg = format!("this rule could not be checked: {reason}");
                self.unchecked(IssueTarget::Node(path), r.id, msg);
                continue;
            }
            if let Some(iri) = &r.reference {
                let msg =
                    format!("the rule <{iri}> is only referenced, so it could not be checked");
                self.unchecked(IssueTarget::Node(path), r.id, msg);
                continue;
            }
            self.rule_structure(&path, list, r);
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
                let path = owner.child(Step::Entity(role, i));
                match &e.locked {
                    None => self.constraints(&path, Step::Refinement, &e.refinement),
                    Some(reason) => {
                        let noun = match role {
                            EntityRole::Target => "asset",
                            _ => "party",
                        };
                        let msg = format!("this {noun} could not be checked: {reason}");
                        self.unchecked(IssueTarget::Node(path), e.id, msg);
                    }
                }
            }
        }
    }

    fn actions(&mut self, owner: &NodePath, actions: &[ActionNode]) {
        for (i, a) in actions.iter().enumerate() {
            let path = owner.child(Step::Action(i));
            if let Some(reason) = &a.locked {
                let msg = format!("this action could not be checked: {reason}");
                self.unchecked(IssueTarget::Node(path), a.id, msg);
                continue;
            }
            self.term(&path, a.id, Field::Name, &a.name, Pos::Action);
            self.constraints(&path, Step::Refinement, &a.refinement);
        }
    }

    fn constraints(&mut self, owner: &NodePath, step: fn(usize) -> Step, list: &[ConstraintNode]) {
        for (i, c) in list.iter().enumerate() {
            let path = owner.child(step(i));
            match c {
                ConstraintNode::Atomic(a) => self.atomic(&path, a),
                ConstraintNode::Logical(l) => self.constraints(&path, Step::Child, &l.children),
                ConstraintNode::Reference { id, iri, .. } => {
                    let msg = format!(
                        "the constraint <{iri}> is only referenced, so it could not be checked"
                    );
                    self.unchecked(IssueTarget::Node(path), *id, msg);
                }
                ConstraintNode::Opaque { id, reason, .. } => {
                    let msg = format!("this constraint could not be checked: {reason}");
                    self.unchecked(IssueTarget::Node(path), *id, msg);
                }
            }
        }
    }

    fn atomic(&mut self, path: &NodePath, a: &AtomicConstraint) {
        self.constraint_structure(path, a);
        self.term(path, a.id, Field::LeftOperand, &a.left, Pos::LeftOperand);
        self.term(path, a.id, Field::Operator, &a.operator, Pos::Operator);
        if let RightOperand::Values(values) = &a.right {
            for (k, l) in values.iter().enumerate() {
                if let Literal::Iri(i) = l {
                    self.term(path, a.id, Field::RightOperand(k), i, Pos::Value);
                }
            }
        }
        if let Some(u) = &a.unit {
            self.term(path, a.id, Field::Unit, u, Pos::Unit);
        }
        // The left operand's ds-prose hints, when a loaded profile has any.
        let hints = self
            .resolve(&a.left)
            .filter(|(t, _)| t.kinds.contains(&TermKind::LeftOperand))
            .map(|(t, _)| &t.hints);
        if let Some(h) = hints {
            self.hint_operator(path, a, h);
            self.hint_datatype(path, a, h);
        }
        // The rest is off in advisory mode unless a level set by hand
        // switches a rule on.
        if let Some(h) = hints {
            self.hint_values(path, a, h);
            self.hint_unit(path, a, h);
            self.datatype_conflict(path, a, h);
        }
        // A literal the hint's own check already reports at least as
        // loudly is said once.
        let hinted = hints
            .filter(|h| {
                self.options
                    .level_of(RuleId::ProseDatatypeLexical, h.closed)
                    >= self.options.level_of(RuleId::ProseIllTypedLiteral, false)
            })
            .and_then(|h| h.datatype.as_deref());
        self.ill_typed(path, a, hinted);
        self.ordered_on_unordered(path, a);
    }

    /// The operator, against `prose:operator`.
    fn hint_operator(&mut self, path: &NodePath, a: &AtomicConstraint, h: &OperandHints) {
        if a.operator.is_empty()
            || h.operators.is_empty()
            || h.operators.contains(&self.expand(&a.operator))
        {
            return;
        }
        let names: Vec<String> = h
            .operators
            .iter()
            .map(|o| local_name(o).to_string())
            .collect();
        let binding = h.closed && self.options.mode() == Mode::Strict;
        let msg = format!(
            "the profile {} the operators {} for \"{}\"",
            if binding { "allows only" } else { "suggests" },
            join_with(&names, "or"),
            a.left
        );
        if let Some(f) = self.emit_term(
            RuleId::ProseOperatorNotAllowed,
            h.closed,
            path,
            a.id,
            Field::Operator,
            &a.operator,
            msg,
        ) {
            f.expected = Some(Expected {
                operators: h.operators.clone(),
                closed: h.closed,
                ..Expected::default()
            });
        }
    }

    /// Each literal value, against the lexical space of `prose:datatype`.
    fn hint_datatype(&mut self, path: &NodePath, a: &AtomicConstraint, h: &OperandHints) {
        let Some((fits, noun)) = h
            .datatype
            .as_deref()
            .and_then(|d| d.strip_prefix(XSD))
            .and_then(lexical)
        else {
            return;
        };
        let RightOperand::Values(values) = &a.right else {
            return;
        };
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
                if let Some(f) = self.emit(
                    RuleId::ProseDatatypeLexical,
                    h.closed,
                    slot(path, Field::RightOperand(k)),
                    msg,
                ) {
                    f.expected = Some(Expected {
                        datatype: h.datatype.clone(),
                        closed: h.closed,
                        ..Expected::default()
                    });
                }
            }
        }
    }

    /// Each value, against `prose:rightOperand` and the values narrower
    /// than those. On an open operand only an IRI value is looked at, and
    /// only to say so; on a closed one every value must be allowed, and a
    /// string naming an allowed IRI should be that IRI.
    fn hint_values(&mut self, path: &NodePath, a: &AtomicConstraint, h: &OperandHints) {
        if h.value_tree.is_empty() {
            return;
        }
        let allowed: HashSet<&str> = h.value_tree.iter().map(|(v, _)| v.as_str()).collect();
        let expected = || Expected {
            values: h.value_tree.iter().map(|(v, _)| v.clone()).collect(),
            closed: h.closed,
            ..Expected::default()
        };
        let values = match &a.right {
            RightOperand::Values(v) => v,
            RightOperand::Reference(_) => {
                let msg = format!(
                    "the right operand is only referenced (rightOperandReference), so the values the profile allows for \"{}\" could not be checked",
                    a.left
                );
                if h.closed {
                    self.unchecked(IssueTarget::Node(path.clone()), a.id, msg);
                }
                return;
            }
            RightOperand::Missing => return,
        };
        for (k, l) in values.iter().enumerate() {
            let raw = l.raw();
            let field = Field::RightOperand(k);
            if let Literal::Iri(_) = l {
                if allowed.contains(self.expand(&raw).as_str()) {
                    continue;
                }
            } else if !h.closed {
                continue;
            } else if allowed.contains(self.expand(&raw).as_str()) {
                let msg = format!(
                    "\"{raw}\" names a value the profile allows for \"{}\", but as a plain string; write it as an IRI ({{\"@id\": \"{raw}\"}})",
                    a.left
                );
                let fix = fix_slot(
                    "Write it as an IRI".to_string(),
                    true,
                    SlotPath {
                        node: path.clone(),
                        field,
                    },
                    a.id,
                    raw.clone(),
                );
                if let Some(f) =
                    self.emit_term(RuleId::ProseIriExpected, true, path, a.id, field, &raw, msg)
                {
                    f.expected = Some(expected());
                    f.fixes.push(fix);
                }
                continue;
            }
            let msg = format!(
                "\"{raw}\" is not a value the profile allows for \"{}\"",
                a.left
            );
            if let Some(f) = self.emit_term(
                RuleId::ProseValueNotAllowed,
                h.closed,
                path,
                a.id,
                field,
                &raw,
                msg,
            ) {
                f.expected = Some(expected());
            }
        }
    }

    /// The unit, against `prose:unit`.
    fn hint_unit(&mut self, path: &NodePath, a: &AtomicConstraint, h: &OperandHints) {
        let Some(u) = a.unit.as_deref().filter(|u| !u.is_empty()) else {
            return;
        };
        if h.units.is_empty() || h.units.contains(&self.expand(u)) {
            return;
        }
        let names: Vec<String> = h.units.iter().map(|u| local_name(u).to_string()).collect();
        let binding = h.closed && self.options.mode() == Mode::Strict;
        let msg = format!(
            "the profile {} the units {} for \"{}\"",
            if binding { "allows only" } else { "suggests" },
            join_with(&names, "or"),
            a.left
        );
        if let Some(f) = self.emit_term(
            RuleId::ProseUnitNotAllowed,
            h.closed,
            path,
            a.id,
            Field::Unit,
            u,
            msg,
        ) {
            f.expected = Some(Expected {
                units: h.units.clone(),
                closed: h.closed,
                ..Expected::default()
            });
        }
    }

    /// A typed value, or the constraint's own `odrl:dataType`, naming
    /// another datatype than `prose:datatype`.
    fn datatype_conflict(&mut self, path: &NodePath, a: &AtomicConstraint, h: &OperandHints) {
        let Some(want) = h.datatype.as_deref() else {
            return;
        };
        let expected = || Expected {
            datatype: Some(want.to_string()),
            closed: h.closed,
            ..Expected::default()
        };
        if let RightOperand::Values(values) = &a.right {
            for (k, l) in values.iter().enumerate() {
                let Literal::Typed { value, datatype } = l else {
                    continue;
                };
                let got = self.expand(datatype);
                if got != want {
                    let msg = format!(
                        "\"{value}\" is typed <{got}>, but the profile gives \"{}\" the datatype <{want}>",
                        a.left
                    );
                    if let Some(f) = self.emit(
                        RuleId::ProseDatatypeConflict,
                        h.closed,
                        slot(path, Field::RightOperand(k)),
                        msg,
                    ) {
                        f.expected = Some(expected());
                    }
                }
            }
        }
        let own = a
            .origin
            .as_ref()
            .and_then(|o| o.value().get("dataType"))
            .and_then(|d| d.as_str().or_else(|| d.get("@id").and_then(|i| i.as_str())));
        if let Some(own) = own {
            let got = self.expand(own);
            if got != want {
                let msg = format!(
                    "the constraint's dataType is <{got}>, but the profile gives \"{}\" the datatype <{want}>",
                    a.left
                );
                if let Some(f) = self.emit(
                    RuleId::ProseDatatypeConflict,
                    h.closed,
                    IssueTarget::Node(path.clone()),
                    msg,
                ) {
                    f.expected = Some(expected());
                }
            }
        }
    }

    /// A typed literal outside its own datatype's lexical space (RDF 1.1
    /// calls it ill-typed). One typed with `hinted`, the left operand's
    /// `prose:datatype` when [`Checker::hint_datatype`] reports it at least
    /// as loudly, is left to that check.
    fn ill_typed(&mut self, path: &NodePath, a: &AtomicConstraint, hinted: Option<&str>) {
        let RightOperand::Values(values) = &a.right else {
            return;
        };
        for (k, l) in values.iter().enumerate() {
            let Literal::Typed { value, datatype } = l else {
                continue;
            };
            let iri = self.expand(datatype);
            if hinted == Some(iri.as_str()) {
                continue;
            }
            let Some((fits, noun)) = iri.strip_prefix(XSD).and_then(lexical) else {
                continue;
            };
            if !fits(value) {
                let msg = format!(
                    "\"{value}\" is not {noun}, so the literal typed {datatype} is ill-typed"
                );
                self.emit(
                    RuleId::ProseIllTypedLiteral,
                    false,
                    slot(path, Field::RightOperand(k)),
                    msg,
                );
            }
        }
    }

    /// An ordering operator with an IRI or a boolean value: likely a
    /// mistake, though no standard rules it out.
    fn ordered_on_unordered(&mut self, path: &NodePath, a: &AtomicConstraint) {
        let op = self.expand(&a.operator);
        let ordering = op
            .strip_prefix(ODRL)
            .is_some_and(|l| matches!(l, "gt" | "gteq" | "lt" | "lteq"));
        let unordered = match &a.right {
            RightOperand::Values(v) => v
                .iter()
                .any(|l| matches!(l, Literal::Iri(_) | Literal::Bool(_))),
            _ => false,
        };
        if ordering && unordered {
            let msg = format!(
                "\"{}\" orders values, but \"{}\" is compared with an IRI or a boolean, which have no order",
                a.operator, a.left
            );
            self.emit_term(
                RuleId::HeuristicOrderedOnUnordered,
                false,
                path,
                a.id,
                Field::Operator,
                &a.operator,
                msg,
            );
        }
    }
}
