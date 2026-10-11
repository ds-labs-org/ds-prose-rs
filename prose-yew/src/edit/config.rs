//! Everything a host can configure about the editor: limits (through
//! [`EditRules`]), wording, class names, inline styles, vocabularies and
//! button looks. Nothing here is specific to a host; the defaults are the
//! ODRL JSON-LD preset.
use prose_core::edit::{EditRules, EmptyText, Field, ListKind, ListPath, NodePath, RuleList, Step};
use prose_core::profile::{Prefixes, ProfileVocab, Term, TermKind, Wording};
use prose_core::wording;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use yew::prelude::*;

/// What a + or - button shows. The accessible name is always the specific
/// label, whatever is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonContent {
    /// Only the symbol: "+" or the minus sign.
    Symbol,
    /// Only words: "Add condition".
    Text,
    /// Symbol and noun: "+ condition".
    #[default]
    SymbolAndText,
}

/// When the + and - controls are visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Reveal {
    #[default]
    Always,
    /// Dimmed until the pointer is over, or focus is inside, the block.
    OnHoverOrFocus,
}

/// A vocabulary entry offered while typing into a free-text slot.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// The raw text written into the slot when picked.
    pub value: AttrValue,
    pub label: AttrValue,
    pub hint: Option<AttrValue>,
    /// The heading the suggestion is listed under, e.g. "From profile:
    /// Research Data Profile". `None`: listed under `Labels::group_core`
    /// when the list has groups, under no heading otherwise.
    pub group: Option<AttrValue>,
}

impl Suggestion {
    pub fn new(value: &str, label: &str, hint: Option<&str>) -> Suggestion {
        Suggestion {
            value: value.to_string().into(),
            label: label.to_string().into(),
            hint: hint.map(|h| h.to_string().into()),
            group: None,
        }
    }
}

/// An entry of a closed list. The value "" means "absent".
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub value: AttrValue,
    pub label: AttrValue,
}

impl Choice {
    pub fn new(value: &str, label: &str) -> Choice {
        Choice {
            value: value.to_string().into(),
            label: label.to_string().into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vocabulary {
    pub actions: Vec<Suggestion>,
    pub left_operands: Vec<Suggestion>,
    pub policy_kinds: Vec<Suggestion>,
    pub parties: Vec<Suggestion>,
    pub assets: Vec<Suggestion>,
    /// Closed list of raw terms; the option labels come from
    /// `labels.operator_option` and `wording::operator_for`.
    pub operators: Vec<AttrValue>,
    /// Closed list; the value "" means absent.
    pub conflicts: Vec<Choice>,
}

impl Default for Vocabulary {
    fn default() -> Self {
        let actions = [
            "use",
            "transfer",
            "display",
            "distribute",
            "reproduce",
            "modify",
            "derive",
            "delete",
            "print",
            "play",
            "read",
            "execute",
            "archive",
            "aggregate",
            "anonymize",
            "attribute",
            "compensate",
            "inform",
            "obtainConsent",
            "sell",
        ];
        Vocabulary {
            actions: actions
                .iter()
                .map(|a| Suggestion::new(a, a, Some(&wording::action(a))))
                .collect(),
            left_operands: wording::known_left_operands()
                .iter()
                .map(|(term, phrase)| Suggestion::new(term, term, Some(phrase)))
                .collect(),
            policy_kinds: [
                "Set",
                "Offer",
                "Agreement",
                "Privacy",
                "Request",
                "Ticket",
                "Assertion",
            ]
            .iter()
            .map(|k| Suggestion::new(k, k, None))
            .collect(),
            parties: vec![],
            assets: vec![],
            operators: [
                "eq", "neq", "gt", "gteq", "lt", "lteq", "isA", "hasPart", "isPartOf", "isAllOf",
                "isAnyOf", "isNoneOf",
            ]
            .iter()
            .map(|o| AttrValue::from(o.to_string()))
            .collect(),
            conflicts: vec![
                Choice::new("", "not stated"),
                Choice::new("perm", "the permission wins"),
                Choice::new("prohibit", "the prohibition wins"),
                Choice::new("invalid", "the whole policy is void"),
            ],
        }
    }
}

impl Vocabulary {
    /// Adds the terms of the profiles `w` holds: actions, left operands and
    /// policy classes first, each under a "From profile: {title}" group,
    /// and operators and conflict terms after the built-in ones. Deprecated
    /// terms are left out, and an entry already listed (compared by IRI) is
    /// listed once. A profile term's value is its full IRI; the editor
    /// writes it as a compact IRI only where the policy binds the prefix.
    pub fn with_profiles(self, w: &Wording) -> Vocabulary {
        self.with_profiles_in(w, &Labels::default().group_profile)
    }

    /// As [`Vocabulary::with_profiles`], with the group heading template.
    pub(crate) fn with_profiles_in(mut self, w: &Wording, group: &str) -> Vocabulary {
        if w.is_empty() {
            return self;
        }
        let langs = w.languages();
        let no_prefixes = Prefixes::default();
        let iri_of = |value: &str| w.expand(&no_prefixes, value);
        let value_of = |iri: &str| no_prefixes.compact(iri).unwrap_or_else(|| iri.to_string());
        let (mut actions, mut left, mut kinds) = (vec![], vec![], vec![]);
        let mut seen: HashSet<String> = HashSet::new();
        for p in w.profiles() {
            let title = p
                .title()
                .pick(langs)
                .or(p.iri())
                .unwrap_or("unnamed profile");
            let g = AttrValue::from(fill(group, &[("profile", title)]));
            let entry = |t: &Term, hint: Option<String>| Suggestion {
                value: value_of(&t.iri).into(),
                label: t.display_label(langs).into(),
                hint: hint.filter(|h| !h.is_empty()).map(AttrValue::from),
                group: Some(g.clone()),
            };
            let definition = |t: &Term| t.definition.pick(langs).map(str::to_string);
            for t in action_order(p) {
                if seen.insert(t.iri.clone()) {
                    actions.push(entry(t, Some(action_hint(w, t))));
                }
            }
            for t in p.terms_of(TermKind::LeftOperand).filter(|t| !t.deprecated) {
                if seen.insert(t.iri.clone()) {
                    left.push(entry(t, definition(t)));
                }
            }
            for t in p.terms_of(TermKind::PolicyClass).filter(|t| !t.deprecated) {
                if seen.insert(t.iri.clone()) {
                    kinds.push(entry(t, definition(t)));
                }
            }
            for t in p.terms_of(TermKind::Operator).filter(|t| !t.deprecated) {
                if !self.operators.iter().any(|o| iri_of(o) == t.iri) {
                    self.operators.push(value_of(&t.iri).into());
                }
            }
            for t in p.terms_of(TermKind::ConflictTerm).filter(|t| !t.deprecated) {
                if !self.conflicts.iter().any(|c| iri_of(&c.value) == t.iri) {
                    self.conflicts
                        .push(Choice::new(&value_of(&t.iri), &t.display_label(langs)));
                }
            }
        }
        // Profile entries first; a listed entry with the same IRI goes.
        let merge = |new: Vec<Suggestion>, old: Vec<Suggestion>| -> Vec<Suggestion> {
            let iris: HashSet<String> = new.iter().map(|s| iri_of(&s.value)).collect();
            new.into_iter()
                .chain(
                    old.into_iter()
                        .filter(|s| !iris.contains(&iri_of(&s.value))),
                )
                .collect()
        };
        self.actions = merge(actions, std::mem::take(&mut self.actions));
        self.left_operands = merge(left, std::mem::take(&mut self.left_operands));
        self.policy_kinds = merge(kinds, std::mem::take(&mut self.policy_kinds));
        self
    }
}

/// A profile's actions, not deprecated, depth first along `includedIn`
/// within the profile: an action right after the one it is included in.
/// A tree goes where its first action in the document is; a cycle is cut
/// where it closes.
fn action_order(p: &ProfileVocab) -> Vec<&Term> {
    let acts: Vec<&Term> = p
        .terms_of(TermKind::Action)
        .filter(|t| !t.deprecated)
        .collect();
    let at: HashMap<&str, usize> = acts
        .iter()
        .enumerate()
        .map(|(i, t)| (t.iri.as_str(), i))
        .collect();
    let parent: Vec<Option<usize>> = acts
        .iter()
        .enumerate()
        .map(|(i, t)| {
            t.included_in
                .iter()
                .find_map(|p| at.get(p.as_str()).copied().filter(|&p| p != i))
        })
        .collect();
    let mut children: Vec<Vec<usize>> = vec![vec![]; acts.len()];
    for (i, p) in parent.iter().enumerate() {
        if let Some(p) = p {
            children[*p].push(i);
        }
    }
    let mut done = vec![false; acts.len()];
    // The climb that last passed each action, so a climb stops on a cycle.
    let mut climbed: Vec<Option<usize>> = vec![None; acts.len()];
    let mut out = Vec::with_capacity(acts.len());
    for first in 0..acts.len() {
        if done[first] {
            continue;
        }
        // Up to the root of this action's tree, or to where a cycle closes.
        let mut root = first;
        climbed[root] = Some(first);
        while let Some(p) = parent[root].filter(|&p| climbed[p] != Some(first) && !done[p]) {
            climbed[p] = Some(first);
            root = p;
        }
        let mut stack = vec![root];
        while let Some(i) = stack.pop() {
            if std::mem::replace(&mut done[i], true) {
                continue;
            }
            out.push(acts[i]);
            stack.extend(children[i].iter().rev().filter(|&&c| !done[c]));
        }
    }
    out
}

/// "{definition} · part of {broader action}", with either part left out
/// when the profile does not give it.
fn action_hint(w: &Wording, t: &Term) -> String {
    let langs = w.languages();
    let parent = t.included_in.first().map(|i| match w.lookup(i) {
        Some((p, _)) => p.display_label(langs),
        None if i.starts_with(ODRL_NS) => wording::action(wording::local_name(i)),
        None => wording::humanise(wording::local_name(i)),
    });
    let definition = t.definition.pick(langs);
    match (definition, parent) {
        (Some(d), Some(p)) => format!("{d} \u{b7} part of {p}"),
        (Some(d), None) => d.to_string(),
        (None, Some(p)) => format!("part of {p}"),
        (None, None) => String::new(),
    }
}

const ODRL_NS: &str = "http://www.w3.org/ns/odrl/2/";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// How a suggested value is written into a policy whose prefixes are
/// `prefixes`: a full IRI as a compact IRI or bare ODRL term where the
/// policy binds it, else in full; any other text as it is.
pub(crate) fn written_form(value: &str, prefixes: &Prefixes) -> AttrValue {
    let written = value
        .contains("://")
        .then(|| prefixes.compact(value))
        .flatten();
    AttrValue::from(written.unwrap_or_else(|| value.to_string()))
}

/// Texts shown where a list is empty.
#[derive(Debug, Clone, PartialEq)]
pub struct EmptyTexts {
    pub anyone_subject: AttrValue,
    pub anyone_object: AttrValue,
    pub the_assignee: AttrValue,
    pub unspecified_asset: AttrValue,
    pub unspecified_action: AttrValue,
    pub no_assigner: AttrValue,
    pub no_parent: AttrValue,
    pub no_profile: AttrValue,
}

impl Default for EmptyTexts {
    fn default() -> Self {
        let d = |e: EmptyText| AttrValue::from(e.default_text());
        EmptyTexts {
            anyone_subject: d(EmptyText::AnyoneSubject),
            anyone_object: d(EmptyText::AnyoneObject),
            the_assignee: d(EmptyText::TheAssignee),
            unspecified_asset: d(EmptyText::UnspecifiedAsset),
            unspecified_action: d(EmptyText::UnspecifiedAction),
            no_assigner: d(EmptyText::NoAssigner),
            no_parent: d(EmptyText::NoParent),
            no_profile: d(EmptyText::NoProfile),
        }
    }
}

impl EmptyTexts {
    pub fn get(&self, e: EmptyText) -> AttrValue {
        match e {
            EmptyText::Nothing => AttrValue::from(""),
            EmptyText::AnyoneSubject => self.anyone_subject.clone(),
            EmptyText::AnyoneObject => self.anyone_object.clone(),
            EmptyText::TheAssignee => self.the_assignee.clone(),
            EmptyText::UnspecifiedAsset => self.unspecified_asset.clone(),
            EmptyText::UnspecifiedAction => self.unspecified_action.clone(),
            EmptyText::NoAssigner => self.no_assigner.clone(),
            EmptyText::NoParent => self.no_parent.clone(),
            EmptyText::NoProfile => self.no_profile.clone(),
            // A text this version has no wording for is shown as nothing.
            _ => AttrValue::from(""),
        }
    }
}

macro_rules! text_struct {
    ($(#[$m:meta])* $name:ident { $($f:ident: $d:expr),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name { $(pub $f: AttrValue),* }
        impl Default for $name {
            fn default() -> Self { $name { $($f: AttrValue::from($d)),* } }
        }
    };
}

text_struct!(
    /// Nouns used in button labels and accessible names.
    Nouns {
        policy: "policy",
        permission: "permission",
        prohibition: "prohibition",
        obligation: "obligation",
        duty: "duty",
        remedy: "remedy",
        consequence: "consequence",
        assigner: "assigner",
        assignee: "assignee",
        target: "target",
        action: "action",
        refinement: "limit",
        condition: "condition",
        group: "condition group",
        value: "value",
        profile: "profile",
        parent: "parent policy",
    }
);

text_struct!(
    /// Names of the editable fields, for accessible names.
    FieldNames {
        kind: "policy type",
        uid: "identifier",
        conflict: "conflict strategy",
        profile: "profile",
        inherit_from: "parent policy",
        reference: "reference",
        iri: "identifier",
        part_of: "collection",
        name: "action",
        left_operand: "left operand",
        operator: "operator",
        right_operand: "value",
        operand_reference: "value reference",
        unit: "unit",
        logical_op: "logical operator",
    }
);

text_struct!(
    /// Placeholders shown in an empty slot.
    Placeholders {
        kind: "type",
        uid: "identifier",
        party: "party",
        asset: "asset",
        action: "action",
        left_operand: "left operand",
        literal: "value",
        unit: "unit",
        operand_reference: "IRI",
        part_of: "collection",
        profile: "profile",
        policy_ref: "policy identifier",
        reference: "IRI",
    }
);

/// All words the editor writes. Templates substitute `{noun}`, `{n}`,
/// `{field}`, `{owner}`, `{action}`, `{phrase}` and `{raw}`; see [`fill`].
#[derive(Debug, Clone, PartialEq)]
pub struct Labels {
    pub conditions: AttrValue,
    pub conditions_in_duty: AttrValue,
    pub refinements: AttrValue,
    pub follow_up_duty: AttrValue,
    pub follow_up_duty_elsewhere: AttrValue,
    pub follow_up_remedy: AttrValue,
    pub follow_up_consequence: AttrValue,
    /// A remedy outside a prohibition, a consequence outside a duty.
    pub follow_up_remedy_elsewhere: AttrValue,
    pub follow_up_consequence_elsewhere: AttrValue,
    /// Appended to follow-up labels outside ODRL's own positions.
    pub carried_note: AttrValue,
    pub notes_heading: AttrValue,
    pub may: AttrValue,
    pub must_not: AttrValue,
    pub must: AttrValue,
    pub empty: EmptyTexts,
    pub no_rules: AttrValue,
    pub no_policies: AttrValue,
    /// Accessible name templates. When a template has no `{owner}`, the
    /// owner chain (" of condition 1 of permission 1 ...") is appended so
    /// that every button has a distinct name.
    pub add: AttrValue,
    pub remove: AttrValue,
    pub move_up: AttrValue,
    pub move_down: AttrValue,
    pub wrap: AttrValue,
    pub unwrap: AttrValue,
    /// The visible text of the group and ungroup buttons and the reorder
    /// arrows (their accessible names are the templates above). Keep a
    /// visible text that is contained in the accessible name.
    pub wrap_text: AttrValue,
    pub unwrap_text: AttrValue,
    pub move_up_symbol: AttrValue,
    pub move_down_symbol: AttrValue,
    pub add_symbol: AttrValue,
    pub remove_symbol: AttrValue,
    pub operator_option: AttrValue,
    pub unknown_option: AttrValue,
    pub slot_aria: AttrValue,
    pub rule_kind_aria: AttrValue,
    pub locked: AttrValue,
    pub inherited: AttrValue,
    pub announce_added: AttrValue,
    pub announce_removed: AttrValue,
    pub nouns: Nouns,
    pub fields: FieldNames,
    pub placeholders: Placeholders,
    /// The heading of the built-in suggestions in a list that also has
    /// profile suggestions.
    pub group_core: AttrValue,
    /// The heading of one profile's suggestions; `{profile}` is its title.
    pub group_profile: AttrValue,
    /// The glossary's summary line.
    pub glossary_heading: AttrValue,
    /// Glossary lines: `{terms}` is a list of labels, `{note}` the usage
    /// note, `{profile}` the profile's title.
    pub kind_of: AttrValue,
    pub implies: AttrValue,
    pub usage_note: AttrValue,
    pub from_profile: AttrValue,
    /// Shown for a term the profile marks `owl:deprecated`.
    pub deprecated: AttrValue,
    /// The operator select's two groups, when the profile suggests
    /// operators for the left operand.
    pub suggested_operators: AttrValue,
    pub other_operators: AttrValue,
    /// The heading of profile values offered for a left operand the profile
    /// suggests no values for.
    pub profile_values: AttrValue,
    /// (xsd datatype IRI, placeholder) for a value slot whose left operand
    /// names a datatype.
    pub datatype_placeholders: Vec<(AttrValue, AttrValue)>,
}

impl Default for Labels {
    fn default() -> Self {
        let a = |s: &str| AttrValue::from(s.to_string());
        Labels {
            conditions: a("Applies only if"),
            conditions_in_duty: a("Applies only if"),
            refinements: a("Limits on the action {action}"),
            follow_up_duty: a("Duties to fulfil before this permission can be exercised"),
            follow_up_duty_elsewhere: a("Duties attached to this rule"),
            follow_up_remedy: a("Remedies if this prohibition is breached"),
            follow_up_consequence: a("Consequences if this duty is not fulfilled"),
            follow_up_remedy_elsewhere: a("Remedies attached to this rule"),
            follow_up_consequence_elsewhere: a("Consequences attached to this rule"),
            carried_note: a(""),
            notes_heading: a("Not rendered"),
            may: a("may"),
            must_not: a("must not"),
            must: a("must"),
            empty: EmptyTexts::default(),
            no_rules: a("No rules yet."),
            no_policies: a("No policies yet."),
            add: a("Add {noun}"),
            remove: a("Remove {noun} {n}"),
            move_up: a("Move {noun} {n} up"),
            move_down: a("Move {noun} {n} down"),
            wrap: a("Group {noun} {n}"),
            unwrap: a("Ungroup {noun} {n}"),
            wrap_text: a("Group"),
            unwrap_text: a("Ungroup"),
            move_up_symbol: a("\u{2191}"),
            move_down_symbol: a("\u{2193}"),
            add_symbol: a("+"),
            remove_symbol: a("\u{2212}"),
            operator_option: a("{phrase} ({raw})"),
            unknown_option: a("{raw} (not in the list)"),
            slot_aria: a("{field} of {owner}"),
            rule_kind_aria: a("Kind of {owner}"),
            locked: a("Kept exactly as written; not editable here"),
            inherited: a("From the policy"),
            announce_added: a("{noun} added"),
            announce_removed: a("{noun} removed"),
            nouns: Nouns::default(),
            fields: FieldNames::default(),
            placeholders: Placeholders::default(),
            group_core: a("ODRL core"),
            group_profile: a("From profile: {profile}"),
            glossary_heading: a("Terms from the profile"),
            kind_of: a("A kind of: {terms}"),
            implies: a("Implies: {terms}"),
            usage_note: a("Usage: {note}"),
            from_profile: a("From {profile}"),
            deprecated: a("Deprecated in the profile"),
            suggested_operators: a("Suggested by the profile"),
            other_operators: a("Other operators"),
            profile_values: a("Profile values (not specific to this operand)"),
            datatype_placeholders: [
                ("integer", "whole number"),
                ("decimal", "number"),
                ("boolean", "true or false"),
                ("date", "YYYY-MM-DD"),
                ("dateTime", "YYYY-MM-DDThh:mm:ss"),
                ("duration", "duration, e.g. P2Y"),
            ]
            .iter()
            .map(|(t, p)| (a(&format!("{XSD}{t}")), a(p)))
            .collect(),
        }
    }
}

/// Substitute `{key}` placeholders. Unknown placeholders are left as they
/// are, and a substituted value is never searched again.
pub fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    'outer: while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        if let Some(close) = after.find('}') {
            let key = &after[..close];
            for (k, v) in vars {
                if *k == key {
                    out.push_str(v);
                    rest = &after[close + 1..];
                    continue 'outer;
                }
            }
        }
        out.push('{');
        rest = after;
    }
    out.push_str(rest);
    out
}

impl Nouns {
    pub fn rule(&self, list: RuleList) -> &AttrValue {
        match list {
            RuleList::Permission => &self.permission,
            RuleList::Prohibition => &self.prohibition,
            RuleList::Obligation => &self.obligation,
            RuleList::Duty => &self.duty,
            RuleList::Remedy => &self.remedy,
            RuleList::Consequence => &self.consequence,
        }
    }

    pub fn entity(&self, role: prose_core::edit::EntityRole) -> &AttrValue {
        use prose_core::edit::EntityRole::*;
        match role {
            Assigner => &self.assigner,
            Assignee => &self.assignee,
            Target => &self.target,
        }
    }

    /// The noun for one step of a path.
    pub fn step(&self, step: Step) -> &AttrValue {
        match step {
            Step::Rule(l, _) => self.rule(l),
            Step::Entity(r, _) => self.entity(r),
            Step::Action(_) => &self.action,
            Step::Refinement(_) => &self.refinement,
            Step::Constraint(_) | Step::Child(_) => &self.condition,
            // No noun for a step this version does not know: the generic one.
            _ => &self.value,
        }
    }

    /// The noun for the items of a list.
    pub fn list(&self, kind: ListKind) -> &AttrValue {
        match kind {
            ListKind::Policies => &self.policy,
            ListKind::Rules(l) => self.rule(l),
            ListKind::Entities(r) => self.entity(r),
            ListKind::Actions => &self.action,
            ListKind::Refinements => &self.refinement,
            ListKind::Constraints | ListKind::Children => &self.condition,
            ListKind::RightOperand => &self.value,
            ListKind::Profile => &self.profile,
            ListKind::InheritFrom => &self.parent,
            // No noun for a list this version does not know: the generic one.
            _ => &self.value,
        }
    }
}

fn step_index(step: Step) -> usize {
    match step {
        Step::Rule(_, i)
        | Step::Entity(_, i)
        | Step::Action(i)
        | Step::Refinement(i)
        | Step::Constraint(i)
        | Step::Child(i) => i,
        // Unknown steps are not numbered.
        _ => 0,
    }
}

impl Labels {
    /// "condition 1 of permission 1 of policy 1": the last step first.
    pub fn describe(&self, path: &NodePath) -> String {
        let mut parts: Vec<String> = path
            .steps
            .iter()
            .rev()
            .map(|s| format!("{} {}", self.nouns.step(*s), step_index(*s) + 1))
            .collect();
        parts.push(format!("{} {}", self.nouns.policy, path.policy + 1));
        parts.join(" of ")
    }

    pub fn describe_list_owner(&self, list: &ListPath) -> String {
        list.owner
            .as_ref()
            .map(|o| self.describe(o))
            .unwrap_or_default()
    }

    pub fn field_name(&self, field: Field) -> String {
        let f = &self.fields;
        match field {
            Field::Kind => f.kind.to_string(),
            Field::Uid => f.uid.to_string(),
            Field::Conflict => f.conflict.to_string(),
            Field::Profile(i) => format!("{} {}", f.profile, i + 1),
            Field::InheritFrom(i) => format!("{} {}", f.inherit_from, i + 1),
            Field::Reference => f.reference.to_string(),
            Field::Iri => f.iri.to_string(),
            Field::PartOf => f.part_of.to_string(),
            Field::Name => f.name.to_string(),
            Field::LeftOperand => f.left_operand.to_string(),
            Field::Operator => f.operator.to_string(),
            Field::RightOperand(i) => format!("{} {}", f.right_operand, i + 1),
            Field::OperandReference => f.operand_reference.to_string(),
            Field::Unit => f.unit.to_string(),
            Field::LogicalOp => f.logical_op.to_string(),
            // A field this version has no name for is announced by its raw name.
            other => other.to_string(),
        }
    }

    /// The accessible name of a slot or select.
    pub fn slot_name(&self, slot: &prose_core::edit::SlotPath) -> String {
        fill(
            &self.slot_aria,
            &[
                ("field", &self.field_name(slot.field)),
                ("owner", &self.describe(&slot.node)),
            ],
        )
    }

    /// A button name from a template: `{noun}`, `{n}` and `{owner}`, with
    /// the owner chain appended when the template has no `{owner}`.
    pub fn button_name(&self, template: &str, noun: &str, n: Option<usize>, owner: &str) -> String {
        let n = n.map(|n| n.to_string()).unwrap_or_default();
        let mut s = fill(template, &[("noun", noun), ("n", &n), ("owner", owner)]);
        if !template.contains("{owner}") && !owner.is_empty() {
            s.push_str(" of ");
            s.push_str(owner);
        }
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

macro_rules! class_fields {
    ($($f:ident),* $(,)?) => {
        /// Extra class names, added next to the built-in `prose-*` classes
        /// (never replacing them), one per element kind.
        #[derive(Debug, Clone, PartialEq, Default)]
        pub struct ProseClasses { $(pub $f: Classes),* }

        /// Inline styles, one per element kind, with the same field names as
        /// [`ProseClasses`].
        #[derive(Debug, Clone, PartialEq, Default)]
        pub struct ProseStyles { $(pub $f: Option<AttrValue>),* }
    };
}

class_fields!(
    root,
    policy,
    heading,
    intro,
    notes,
    rules,
    rule,
    permission,
    prohibition,
    obligation,
    sentence,
    text,
    term,
    slot,
    slot_empty,
    slot_invalid,
    slot_warning,
    slot_whitespace,
    inherited,
    select,
    list,
    item,
    add,
    remove,
    reorder,
    wrap,
    toolbar,
    add_bar,
    label,
    conditions,
    condition,
    logical,
    follow_up,
    carried,
    locked,
    issue,
    issue_error,
    issue_warning,
    issue_info,
    warnings,
    decoration,
    suggestions,
    suggestion,
    suggestion_active,
    live,
    empty
);

/// The whole configuration of an editing view.
#[derive(Clone, PartialEq)]
pub struct EditConfig {
    pub rules: EditRules,
    pub labels: Labels,
    pub classes: ProseClasses,
    pub styles: ProseStyles,
    pub vocab: Vocabulary,
    pub add_button: ButtonContent,
    pub remove_button: ButtonContent,
    pub reveal: Reveal,
    /// Edit mode renders `<style>{BASE_CSS}</style>`; turn it off to bring
    /// your own.
    pub base_css: bool,
    /// Operator options read "is before (lt)" rather than "is before".
    pub show_raw_terms: bool,
    pub suggestions: bool,
    pub max_suggestions: usize,
    /// The host translates an operator switch the reducer would refuse (it
    /// joins the values into one before a switch from a set operator to a
    /// single-value one, as `bridge::plan` of the ds42 playground does), so
    /// the operator select offers every operator. Off: only the switches the
    /// reducer accepts as they are.
    pub host_plans_operator_switches: bool,
    /// The loaded profiles: their labels in the sentences, their terms in
    /// the suggestions and selects. Set it with [`EditConfig::with_wording`],
    /// which also extends `vocab`.
    pub wording: Rc<Wording>,
}

impl EditConfig {
    /// Uses the profiles `w` holds, and adds their terms to `vocab` (with
    /// the `labels.group_profile` heading, so set the labels first). Meant
    /// for a configuration without a profile yet: the terms of a profile
    /// set before are not taken out.
    pub fn with_wording(mut self, w: Rc<Wording>) -> EditConfig {
        self.vocab = self.vocab.with_profiles_in(&w, &self.labels.group_profile);
        self.wording = w;
        self
    }
}

impl Default for EditConfig {
    fn default() -> Self {
        EditConfig {
            rules: EditRules::default(),
            labels: Labels::default(),
            classes: ProseClasses::default(),
            styles: ProseStyles::default(),
            vocab: Vocabulary::default(),
            add_button: ButtonContent::SymbolAndText,
            remove_button: ButtonContent::Symbol,
            reveal: Reveal::Always,
            base_css: true,
            show_raw_terms: true,
            suggestions: true,
            max_suggestions: 8,
            host_plans_operator_switches: false,
            wording: Rc::new(Wording::default()),
        }
    }
}

/// The class list of an element: the built-in name plus the host's.
macro_rules! cls {
    ($cfg:expr, $field:ident, $($base:expr),+) => {
        yew::classes!($($base,)+ $cfg.classes.$field.clone())
    };
}
pub(crate) use cls;

/// The inline style of an element: the base style and then the state styles
/// that apply, in order, so a later (more specific) one wins. `None` when
/// the host set none of them.
pub(crate) fn merge_styles(parts: &[&Option<AttrValue>]) -> Option<AttrValue> {
    let set: Vec<&str> = parts
        .iter()
        .copied()
        .flatten()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    let mut out = String::new();
    for (i, p) in set.iter().enumerate() {
        out.push_str(p);
        if i + 1 < set.len() {
            if !p.ends_with(';') {
                out.push(';');
            }
            out.push(' ');
        }
    }
    (!out.is_empty()).then(|| AttrValue::from(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_substitutes_known_keys_only() {
        assert_eq!(
            fill("Remove {noun} {n} {x}", &[("noun", "rule"), ("n", "2")]),
            "Remove rule 2 {x}"
        );
        assert_eq!(fill("{a}{a}", &[("a", "{a}")]), "{a}{a}");
        assert_eq!(fill("no braces", &[]), "no braces");
        assert_eq!(fill("open { only", &[("noun", "x")]), "open { only");
    }

    #[test]
    fn describe_walks_from_the_last_step() {
        let l = Labels::default();
        let p: NodePath = "policy[0].permission[0].constraint[1]".parse().unwrap();
        assert_eq!(l.describe(&p), "condition 2 of permission 1 of policy 1");
    }

    #[test]
    fn button_name_appends_owner_unless_templated() {
        let l = Labels::default();
        assert_eq!(
            l.button_name(&l.remove, "condition", Some(2), "permission 1 of policy 1"),
            "Remove condition 2 of permission 1 of policy 1"
        );
        assert_eq!(l.button_name("{noun} of {owner}", "x", None, "y"), "x of y");
    }

    #[test]
    fn merge_styles_layers_in_order() {
        let a = Some(AttrValue::from("border-bottom: 1px dashed grey"));
        let b = Some(AttrValue::from("border-bottom: 2px solid red;"));
        assert_eq!(
            merge_styles(&[&a, &None, &b]).unwrap(),
            "border-bottom: 1px dashed grey; border-bottom: 2px solid red;"
        );
        // One style is passed through unchanged.
        assert_eq!(
            merge_styles(&[&None, &a]).unwrap(),
            "border-bottom: 1px dashed grey"
        );
        assert_eq!(merge_styles(&[&None, &None]), None);
        assert_eq!(merge_styles(&[&Some(AttrValue::from("  "))]), None);
    }

    #[test]
    fn vocabulary_defaults() {
        let v = Vocabulary::default();
        assert_eq!(v.actions.len(), 20);
        assert_eq!(v.operators.len(), 12);
        assert_eq!(v.policy_kinds.len(), 7);
        assert_eq!(v.conflicts[0].value, "");
    }

    // --- profiles -------------------------------------------------------------

    use prose_core::profile::ProfileVocab;

    const EX: &str = "https://example.org/odrl/profile/research/";
    const ODRL: &str = "http://www.w3.org/ns/odrl/2/";

    fn research() -> Wording {
        let text = include_str!("../../../prose-core/tests/profiles/research.jsonld");
        Wording::new().with_profile(ProfileVocab::parse(text).unwrap())
    }

    fn odrl22() -> Wording {
        let text = include_str!("../../../prose-core/tests/profiles/odrl22-excerpt.json");
        Wording::new().with_profile(ProfileVocab::parse(text).unwrap())
    }

    fn values(list: &[Suggestion]) -> Vec<&str> {
        list.iter().map(|s| s.value.as_str()).collect()
    }

    #[test]
    fn profile_actions_first_grouped_with_definition_hint() {
        let v = Vocabulary::default().with_profiles(&research());
        let group = Some(AttrValue::from("From profile: Research Data Profile"));
        let head: Vec<(&str, &str)> = v.actions[..3]
            .iter()
            .map(|s| (s.value.as_str(), s.label.as_str()))
            .collect();
        assert_eq!(
            head,
            [
                (&*format!("{EX}train"), "Train"),
                (&*format!("{EX}fineTune"), "Fine-tune"),
                (&*format!("{EX}pseudonymise"), "Pseudonymise"),
            ]
        );
        assert!(v.actions[..3].iter().all(|s| s.group == group));
        assert_eq!(
            v.actions[1].hint.as_deref(),
            Some("To adapt an already trained model using the Asset. \u{b7} part of Train")
        );
        assert!(
            v.actions[0]
                .hint
                .as_deref()
                .unwrap()
                .starts_with("To use the Asset as training data"),
            "{:?}",
            v.actions[0].hint
        );
        // The built-in actions follow, unchanged and ungrouped.
        assert_eq!(v.actions[3..], Vocabulary::default().actions[..]);

        // Left operands: the profile's first, with their definition.
        assert_eq!(
            values(&v.left_operands[..3]),
            [
                format!("{EX}dataSubjectCount"),
                format!("{EX}researchPurpose"),
                format!("{EX}retentionPeriod"),
            ]
        );
        assert_eq!(v.left_operands[0].label, "Number of data subjects");
        assert!(v.left_operands[..3].iter().all(|s| s.group == group));
        assert_eq!(
            v.left_operands[1].hint.as_deref(),
            Some("The research purpose the action of the Rule is exercised for.")
        );
        assert_eq!(
            v.left_operands[3..],
            Vocabulary::default().left_operands[..]
        );
    }

    #[test]
    fn actions_follow_included_in_depth_first_without_deprecated() {
        let p = ProfileVocab::parse(&format!(
            r#"{{
                "@context": {{"odrl": "{ODRL}", "ex": "{EX}",
                             "owl": "http://www.w3.org/2002/07/owl#",
                             "rdfs": "http://www.w3.org/2000/01/rdf-schema#"}},
                "@graph": [
                    {{"@id": "ex:child", "@type": "odrl:Action", "odrl:includedIn": {{"@id": "ex:parent"}}}},
                    {{"@id": "ex:other", "@type": "odrl:Action"}},
                    {{"@id": "ex:old", "@type": "odrl:Action", "owl:deprecated": true}},
                    {{"@id": "ex:parent", "@type": "odrl:Action", "rdfs:label": "Parent"}}
                ]
            }}"#
        ))
        .unwrap();
        let v = Vocabulary::default().with_profiles(&Wording::new().with_profile(p));
        assert_eq!(
            values(&v.actions[..3]),
            [
                format!("{EX}parent"),
                format!("{EX}child"),
                format!("{EX}other")
            ]
        );
        assert!(!values(&v.actions).contains(&&*format!("{EX}old")));
        assert_eq!(v.actions[1].hint.as_deref(), Some("part of Parent"));
        assert_eq!(
            v.actions[1].group.as_deref(),
            Some("From profile: unnamed profile")
        );
    }

    #[test]
    fn action_order_ends_on_an_included_in_cycle() {
        let text = include_str!("../../../prose-core/tests/profiles/hostile/cycle.jsonld");
        let p = ProfileVocab::parse(text).unwrap();
        let order: Vec<&str> = action_order(&p).iter().map(|t| t.iri.as_str()).collect();
        let ex = "https://example.org/cycle/";
        // a's tree is entered at b (where the cycle closes), and c follows a.
        assert_eq!(
            order,
            [format!("{ex}b"), format!("{ex}a"), format!("{ex}c")]
        );
    }

    #[test]
    fn dedup_by_iri() {
        let v = Vocabulary::default().with_profiles(&odrl22());
        let uses: Vec<&Suggestion> = v.actions.iter().filter(|s| s.value == "use").collect();
        assert_eq!(uses.len(), 1, "{:?}", values(&v.actions));
        assert_eq!(uses[0].label, "Use");
        assert_eq!(
            uses[0].group.as_deref(),
            Some("From profile: ODRL Version 2.2")
        );
        // An ODRL term is offered bare, so it is written bare; one that
        // ODRL 2.2 deprecates (commercialize) is not offered.
        assert!(values(&v.actions[..2]).contains(&"distribute"));
        assert!(!values(&v.actions).contains(&"commercialize"));
        assert_eq!(
            v.actions.iter().filter(|s| s.value == "distribute").count(),
            1
        );
        // A closed list keeps its built-in entry and wording.
        assert_eq!(v.operators.iter().filter(|o| *o == "gteq").count(), 1);
        assert_eq!(v.operators.len(), 12);
        assert_eq!(v.conflicts.iter().filter(|c| c.value == "perm").count(), 1);
        assert_eq!(v.conflicts, Vocabulary::default().conflicts);
        // Applied twice, nothing is listed twice.
        let twice = v.clone().with_profiles(&odrl22());
        assert_eq!(twice, v);
    }

    #[test]
    fn value_is_curie_only_if_policy_binds_prefix() {
        let doc = |context: &str| {
            let json = format!(r#"{{"@context": {context}, "@type": "Set", "permission": []}}"#);
            prose_core::edit::prefixes(&prose_core::edit::read_model(&json).unwrap())
        };
        let policy = doc(&format!(
            r#"["http://www.w3.org/ns/odrl.jsonld", {{"ex": "{EX}"}}]"#
        ));
        let unbound = doc(r#""http://www.w3.org/ns/odrl.jsonld""#);
        let iri = format!("{EX}fineTune");
        assert_eq!(written_form(&iri, &policy), "ex:fineTune");
        assert_eq!(written_form(&iri, &unbound), iri.as_str());
        assert_eq!(written_form(&format!("{ODRL}use"), &unbound), "use");
        // Text that is not a full IRI is written as it is.
        assert_eq!(written_form("use", &policy), "use");
        assert_eq!(written_form("ex:fineTune", &unbound), "ex:fineTune");
    }

    #[test]
    fn operators_conflicts_policy_kinds_extended() {
        let v = Vocabulary::default().with_profiles(&research());
        let d = Vocabulary::default();
        assert_eq!(v.operators[..12], d.operators[..]);
        assert_eq!(
            v.operators[12..],
            [AttrValue::from(format!("{EX}withinRegion"))]
        );
        assert_eq!(v.conflicts[..4], d.conflicts[..]);
        assert_eq!(
            v.conflicts[4..],
            [Choice::new(&format!("{EX}ignore"), "Ignore conflicts")]
        );
        assert_eq!(v.policy_kinds[0].value, format!("{EX}ResearchAgreement"));
        assert_eq!(v.policy_kinds[0].label, "Research agreement");
        assert_eq!(v.policy_kinds[1..], d.policy_kinds[..]);
        // Party and asset lists are not touched.
        assert_eq!(v.parties, d.parties);
        assert_eq!(v.assets, d.assets);
    }

    #[test]
    fn ex_archive_absent() {
        let v = Vocabulary::default().with_profiles(&research());
        let archive = format!("{EX}archive");
        for list in [&v.actions, &v.left_operands, &v.policy_kinds] {
            assert!(!values(list).contains(&archive.as_str()));
        }
        assert!(!v.operators.iter().any(|o| *o == archive));
        assert!(!v.conflicts.iter().any(|c| c.value == archive));
    }

    #[test]
    fn deprecated_terms_are_offered_nowhere() {
        let p = ProfileVocab::parse(&format!(
            r#"{{
                "@context": {{"odrl": "{ODRL}", "ex": "{EX}",
                             "owl": "http://www.w3.org/2002/07/owl#",
                             "rdfs": "http://www.w3.org/2000/01/rdf-schema#"}},
                "@graph": [
                    {{"@id": "ex:lo", "@type": "odrl:LeftOperand"}},
                    {{"@id": "ex:oldLo", "@type": "odrl:LeftOperand", "owl:deprecated": true}},
                    {{"@id": "ex:Kind", "rdfs:subClassOf": {{"@id": "odrl:Policy"}}}},
                    {{"@id": "ex:OldKind", "rdfs:subClassOf": {{"@id": "odrl:Policy"}},
                      "owl:deprecated": true}},
                    {{"@id": "ex:op", "@type": "odrl:Operator"}},
                    {{"@id": "ex:oldOp", "@type": "odrl:Operator", "owl:deprecated": true}},
                    {{"@id": "ex:cf", "@type": "odrl:ConflictTerm"}},
                    {{"@id": "ex:oldCf", "@type": "odrl:ConflictTerm", "owl:deprecated": true}}
                ]
            }}"#
        ))
        .unwrap();
        let v = Vocabulary::default().with_profiles(&Wording::new().with_profile(p));
        let has =
            |list: &[Suggestion], local: &str| values(list).contains(&&*format!("{EX}{local}"));
        // The current terms are offered, so the profile was read.
        assert!(has(&v.left_operands, "lo") && !has(&v.left_operands, "oldLo"));
        assert!(has(&v.policy_kinds, "Kind") && !has(&v.policy_kinds, "OldKind"));
        let ops: Vec<&str> = v.operators.iter().map(|o| o.as_str()).collect();
        assert!(ops.contains(&&*format!("{EX}op")));
        assert!(!ops.contains(&&*format!("{EX}oldOp")));
        let cfs: Vec<&str> = v.conflicts.iter().map(|c| c.value.as_str()).collect();
        assert!(cfs.contains(&&*format!("{EX}cf")));
        assert!(!cfs.contains(&&*format!("{EX}oldCf")));
    }

    #[test]
    fn with_wording_extends_the_vocabulary_with_the_labels_heading() {
        let mut cfg = EditConfig::default();
        cfg.labels.group_profile = "Profil : {profile}".into();
        let w = Rc::new(research());
        let cfg = cfg.with_wording(w.clone());
        assert!(Rc::ptr_eq(&cfg.wording, &w));
        assert_eq!(cfg.actions_group(), Some("Profil : Research Data Profile"));
        assert_eq!(EditConfig::default().wording.as_ref(), &Wording::default());
    }

    impl EditConfig {
        fn actions_group(&self) -> Option<&str> {
            self.vocab.actions.first().and_then(|s| s.group.as_deref())
        }
    }
}
