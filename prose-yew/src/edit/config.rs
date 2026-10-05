//! Everything a host can configure about the editor: limits (through
//! [`EditRules`]), wording, class names, inline styles, vocabularies and
//! button looks. Nothing here is specific to a host; the defaults are the
//! ODRL JSON-LD preset.
use prose_core::edit::{EditRules, EmptyText, Field, ListKind, ListPath, NodePath, RuleList, Step};
use prose_core::wording;
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
}

impl Suggestion {
    pub fn new(value: &str, label: &str, hint: Option<&str>) -> Suggestion {
        Suggestion {
            value: value.to_string().into(),
            label: label.to_string().into(),
            hint: hint.map(|h| h.to_string().into()),
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
            Step::Refinement(_) | Step::Constraint(_) | Step::Child(_) => &self.condition,
        }
    }

    /// The noun for the items of a list.
    pub fn list(&self, kind: ListKind) -> &AttrValue {
        match kind {
            ListKind::Policies => &self.policy,
            ListKind::Rules(l) => self.rule(l),
            ListKind::Entities(r) => self.entity(r),
            ListKind::Actions => &self.action,
            ListKind::Refinements | ListKind::Constraints | ListKind::Children => &self.condition,
            ListKind::RightOperand => &self.value,
            ListKind::Profile => &self.profile,
            ListKind::InheritFrom => &self.parent,
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
}
