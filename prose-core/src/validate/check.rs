//! The walk over a document: every term, constraint and literal, checked in
//! document order against the loaded profiles.
use super::lexical::lexical;
use super::{Expected, Finding, Fix, Mode, Options, RuleId, TermRef};
use crate::edit::*;
use crate::profile::{OdrlVocab, Prefixes, ProfileVocab, Term, TermKind, Wording, odrl_term};
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
        cur: None,
        unsure: false,
        out: Vec::new(),
    };
    c.imports();
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
    out: Vec<Finding>,
}

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
        if self.options.mode() != Mode::Strict {
            return;
        }
        let iri = self.expand(written);
        if let Some(prefix) = self.unbound_prefix(written, &iri) {
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
        let kinds: Vec<TermKind> = match (resolved, odrl) {
            (Some((t, _)), o) => t.kinds.iter().copied().chain(o.map(|o| o.kind)).collect(),
            (None, Some(o)) => vec![o.kind],
            (None, None) => Vec::new(),
        };
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
                // A profile's term: the policy must name the profile (IM 2.1,
                // 3.2). ODRL's own terms need no declaration, whoever
                // redeclares them.
                if let Some(piri) = p.iri()
                    && !t.iri.starts_with(ODRL)
                    && !unsure
                    && !self.declares(piri)
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
                if odrl.is_some_and(|o| o.deprecated) {
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
        });
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
                a.id,
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
