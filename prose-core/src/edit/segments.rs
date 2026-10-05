//! The edit templates: policies as sentences made of text, slots to edit
//! and lists to add to. One uniform wording that shows every slot (the v0.1
//! `render` elides empty ones), built on the same `words` helpers.
use super::model::*;
use super::path::*;
use super::rules::odrl_position;
use crate::read::join_with;
use crate::words::{self, local_name};

#[derive(Debug, Clone, PartialEq)]
pub struct PolicyProse {
    pub node: NodeId,
    pub path: NodePath,
    pub locked: Option<Locked>,
    pub heading: Sentence,
    pub intro: Sentence,
    pub notes: Vec<Sentence>,
    pub refinements: Vec<RefinementGroup>,
    pub rules: Vec<RuleProse>,
    pub add_rule_lists: Vec<ListPath>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleProse {
    pub node: NodeId,
    pub path: NodePath,
    pub list: RuleList,
    pub index: usize,
    pub locked: Option<Locked>,
    pub sentence: Sentence,
    pub refinements: Vec<RefinementGroup>,
    pub conditions: ConditionList,
    pub follow_ups: Vec<FollowUpProse>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RefinementGroup {
    pub action: NodePath,
    pub action_name: String,
    pub conditions: ConditionList,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConditionList {
    pub list: ListPath,
    pub owner: NodeId,
    pub in_duty: bool,
    pub items: Vec<ConditionProse>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FollowUpProse {
    pub list: ListPath,
    pub kind: RuleList,
    pub parent: RuleList,
    pub in_odrl_position: bool,
    pub rules: Vec<RuleProse>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConditionProse {
    pub node: NodeId,
    pub path: NodePath,
    pub index: usize,
    pub locked: Option<Locked>,
    pub sentence: Sentence,
    /// `Some` for a logical group.
    pub children: Option<ConditionList>,
}

/// A node kept exactly as written.
#[derive(Debug, Clone, PartialEq)]
pub struct Locked {
    pub reason: String,
    /// Compact JSON of the origin, cut to 200 characters and "...".
    pub json: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sentence {
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    Text(String),
    Slot(Slot),
    Choice(ChoiceSlot),
    RuleKind(RuleKindSlot),
    List(SlotList),
}

/// A piece of free text a person can edit.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub path: SlotPath,
    pub owner: NodeId,
    pub kind: SlotKind,
    pub raw: String,
    pub display: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotKind {
    PolicyKind,
    Uid,
    Party,
    Asset,
    Action,
    LeftOperand,
    Literal,
    Unit,
    OperandReference,
    PartOf,
    Profile,
    PolicyRef,
    Reference,
}

/// A pick from a closed list.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceSlot {
    pub path: SlotPath,
    pub owner: NodeId,
    pub kind: ChoiceKind,
    pub raw: String,
    pub display: String,
    /// The constraint's left operand, for operator wording; empty otherwise.
    pub left_operand: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChoiceKind {
    Operator,
    LogicalOp,
    Conflict,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleKindSlot {
    pub rule: NodePath,
    pub owner: NodeId,
    pub current: RuleList,
    /// Only top-level rules can change kind.
    pub changeable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlotList {
    pub path: ListPath,
    pub owner: NodeId,
    pub items: Vec<ListItem>,
    /// Shown read-only when `items` is empty: what the rule inherits.
    pub inherited: Vec<Slot>,
    pub conj: Conj,
    pub empty: EmptyText,
    /// Shown only when items or inherited are shown.
    pub lead: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    /// The entity or action id; `None` for lists of plain values.
    pub node: Option<NodeId>,
    pub index: usize,
    pub locked: Option<Locked>,
    pub slot: Slot,
    pub extra: Vec<Segment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conj {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmptyText {
    Nothing,
    AnyoneSubject,
    AnyoneObject,
    TheAssignee,
    UnspecifiedAsset,
    UnspecifiedAction,
    NoAssigner,
    NoParent,
    NoProfile,
}

impl EmptyText {
    pub fn default_text(self) -> &'static str {
        match self {
            EmptyText::Nothing => "",
            EmptyText::AnyoneSubject => "Anyone",
            EmptyText::AnyoneObject => "anyone",
            EmptyText::TheAssignee => "The assignee",
            EmptyText::UnspecifiedAsset => "an unspecified asset",
            EmptyText::UnspecifiedAction => "perform an unspecified action on",
            EmptyText::NoAssigner => "no assigner",
            EmptyText::NoParent => "no other policy",
            EmptyText::NoProfile => "no profile",
        }
    }
}

impl Conj {
    fn word(self) -> &'static str {
        match self {
            Conj::And => "and",
            Conj::Or => "or",
        }
    }
}

impl Sentence {
    /// The display text with the default empty texts.
    pub fn plain(&self) -> String {
        self.segments.iter().map(Segment::plain).collect()
    }
}

impl Segment {
    pub fn plain(&self) -> String {
        match self {
            Segment::Text(t) => t.clone(),
            Segment::Slot(s) => s.display.clone(),
            Segment::Choice(c) => c.display.clone(),
            Segment::RuleKind(k) => match k.current {
                RuleList::Permission => "may",
                RuleList::Prohibition => "must not",
                _ => "must",
            }
            .to_string(),
            Segment::List(l) => l.plain(),
        }
    }
}

impl ListItem {
    pub fn plain(&self) -> String {
        let mut s = self.slot.display.clone();
        s.extend(self.extra.iter().map(Segment::plain));
        s
    }
}

impl SlotList {
    /// The text this list shows: its items, else what it inherits, else the
    /// default empty text.
    pub fn plain(&self) -> String {
        let lead = self.lead.as_deref().unwrap_or("");
        if !self.items.is_empty() {
            let parts: Vec<String> = self.items.iter().map(ListItem::plain).collect();
            return format!("{lead}{}", join_with(&parts, self.conj.word()));
        }
        if !self.inherited.is_empty() {
            let parts: Vec<String> = self.inherited.iter().map(|s| s.display.clone()).collect();
            return format!("{lead}{}", join_with(&parts, self.conj.word()));
        }
        self.empty.default_text().to_string()
    }
}

// --- building -------------------------------------------------------------------

/// Sentences for every policy of the document.
pub fn sentences(doc: &EditDoc) -> Vec<PolicyProse> {
    doc.policies
        .iter()
        .enumerate()
        .map(|(i, p)| policy_prose(i, p))
        .collect()
}

fn locked_of(reason: &Option<String>, origin: Option<&Origin>) -> Option<Locked> {
    reason.as_ref().map(|r| locked(r, origin))
}

fn locked(reason: &str, origin: Option<&Origin>) -> Locked {
    let json = origin
        .and_then(|o| serde_json::to_string(o.value()).ok())
        .unwrap_or_default();
    let json = if json.chars().count() > 200 {
        let cut: String = json.chars().take(200).collect();
        format!("{cut}...")
    } else {
        json
    };
    Locked {
        reason: reason.to_string(),
        json,
    }
}

fn text(t: &str) -> Segment {
    Segment::Text(t.to_string())
}

fn slot(
    node: &NodePath,
    field: Field,
    owner: NodeId,
    kind: SlotKind,
    raw: &str,
    display: String,
) -> Slot {
    Slot {
        path: SlotPath {
            node: node.clone(),
            field,
        },
        owner,
        kind,
        raw: raw.to_string(),
        display,
    }
}

const KEPT: &str = "(kept as written)";

fn entity_display(e: &Entity, noun: &str) -> String {
    match (&e.iri, &e.part_of) {
        (Some(iri), None) => iri.clone(),
        (Some(iri), Some(c)) => format!("{iri} (a member of {c})"),
        (None, Some(c)) => format!("any {noun} in the collection {c}"),
        (None, None) => String::new(),
    }
}

fn entity_items(
    owner: &NodePath,
    role: EntityRole,
    list: &[Entity],
    noun: &str,
    kind: SlotKind,
    in_duty: bool,
) -> Vec<ListItem> {
    list.iter()
        .enumerate()
        .map(|(i, e)| {
            let path = owner.child(Step::Entity(role, i));
            if let Some(reason) = &e.locked {
                return ListItem {
                    node: Some(e.id),
                    index: i,
                    locked: Some(locked(reason, e.origin.as_ref())),
                    slot: slot(&path, Field::Iri, e.id, kind, "", KEPT.into()),
                    extra: vec![],
                };
            }
            let (main, mut extra) = match (&e.iri, &e.part_of) {
                (Some(iri), part) => {
                    let extra = part
                        .as_ref()
                        .map(|c| {
                            vec![
                                text(" (a member of "),
                                Segment::Slot(slot(
                                    &path,
                                    Field::PartOf,
                                    e.id,
                                    SlotKind::PartOf,
                                    c,
                                    c.clone(),
                                )),
                                text(")"),
                            ]
                        })
                        .unwrap_or_default();
                    (slot(&path, Field::Iri, e.id, kind, iri, iri.clone()), extra)
                }
                (None, Some(c)) => (
                    slot(
                        &path,
                        Field::PartOf,
                        e.id,
                        SlotKind::PartOf,
                        c,
                        format!("any {noun} in the collection {c}"),
                    ),
                    vec![],
                ),
                (None, None) => (
                    slot(&path, Field::Iri, e.id, kind, "", String::new()),
                    vec![],
                ),
            };
            if !e.refinement.is_empty() {
                let parts: Vec<String> =
                    cond_items(&path, Step::Refinement, &e.refinement, in_duty)
                        .iter()
                        .map(inline_condition)
                        .collect();
                extra.push(text(&format!(" where {}", parts.join("; "))));
            }
            ListItem {
                node: Some(e.id),
                index: i,
                locked: None,
                slot: main,
                extra,
            }
        })
        .collect()
}

/// Entities as read-only slots, for a rule that inherits them.
fn entity_inherited(
    owner: &NodePath,
    role: EntityRole,
    list: &[Entity],
    noun: &str,
    kind: SlotKind,
) -> Vec<Slot> {
    list.iter()
        .enumerate()
        .filter(|(_, e)| e.locked.is_none())
        .map(|(i, e)| {
            let path = owner.child(Step::Entity(role, i));
            let field = if e.iri.is_none() && e.part_of.is_some() {
                Field::PartOf
            } else {
                Field::Iri
            };
            slot(
                &path,
                field,
                e.id,
                kind,
                e.iri.as_deref().unwrap_or(""),
                entity_display(e, noun),
            )
        })
        .collect()
}

fn action_items(owner: &NodePath, list: &[ActionNode]) -> Vec<ListItem> {
    list.iter()
        .enumerate()
        .map(|(i, a)| {
            let path = owner.child(Step::Action(i));
            let display = if a.name.is_empty() {
                String::new()
            } else {
                words::action(&a.name)
            };
            ListItem {
                node: Some(a.id),
                index: i,
                locked: locked_of(&a.locked, a.origin.as_ref()),
                slot: if a.locked.is_some() {
                    slot(&path, Field::Name, a.id, SlotKind::Action, "", KEPT.into())
                } else {
                    slot(&path, Field::Name, a.id, SlotKind::Action, &a.name, display)
                },
                extra: vec![],
            }
        })
        .collect()
}

fn action_inherited(owner: &NodePath, list: &[ActionNode]) -> Vec<Slot> {
    action_items(owner, list)
        .into_iter()
        .filter(|i| i.locked.is_none())
        .map(|i| i.slot)
        .collect()
}

fn string_items(
    owner: &NodePath,
    owner_id: NodeId,
    list: &[String],
    profile: bool,
) -> Vec<ListItem> {
    list.iter()
        .enumerate()
        .map(|(i, s)| ListItem {
            node: None,
            index: i,
            locked: None,
            slot: slot(
                owner,
                if profile {
                    Field::Profile(i)
                } else {
                    Field::InheritFrom(i)
                },
                owner_id,
                if profile {
                    SlotKind::Profile
                } else {
                    SlotKind::PolicyRef
                },
                s,
                s.clone(),
            ),
            extra: vec![],
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn slot_list(
    owner: &NodePath,
    owner_id: NodeId,
    kind: ListKind,
    items: Vec<ListItem>,
    inherited: Vec<Slot>,
    conj: Conj,
    empty: EmptyText,
    lead: Option<&str>,
) -> Segment {
    Segment::List(SlotList {
        path: ListPath::of(owner, kind),
        owner: owner_id,
        items,
        inherited,
        conj,
        empty,
        lead: lead.map(String::from),
    })
}

/// What a rule inherits when it states none of its own.
#[derive(Clone, Default)]
struct Inh {
    assigner: Vec<Slot>,
    assignee: Vec<Slot>,
    target: Vec<Slot>,
    action: Vec<Slot>,
}

fn policy_prose(i: usize, p: &PolicyNode) -> PolicyProse {
    let path = NodePath::policy(i);
    let mut out = PolicyProse {
        node: p.id,
        path: path.clone(),
        locked: locked_of(&p.locked, p.origin.as_ref()),
        heading: Sentence::default(),
        intro: Sentence::default(),
        notes: vec![],
        refinements: vec![],
        rules: vec![],
        add_rule_lists: vec![],
    };
    if p.locked.is_some() {
        return out;
    }
    let kind_display = if p.kind.is_empty() {
        "Policy".to_string()
    } else {
        local_name(&p.kind).to_string()
    };
    let uid = p.uid.clone().unwrap_or_default();
    out.heading = Sentence {
        segments: vec![
            Segment::Slot(slot(
                &path,
                Field::Kind,
                p.id,
                SlotKind::PolicyKind,
                &p.kind,
                kind_display,
            )),
            text(" "),
            Segment::Slot(slot(
                &path,
                Field::Uid,
                p.id,
                SlotKind::Uid,
                &uid,
                uid.clone(),
            )),
        ],
    };
    out.intro = Sentence {
        segments: vec![
            text("Between "),
            slot_list(
                &path,
                p.id,
                ListKind::Entities(EntityRole::Assigner),
                entity_items(
                    &path,
                    EntityRole::Assigner,
                    &p.assigner,
                    "party",
                    SlotKind::Party,
                    false,
                ),
                vec![],
                Conj::And,
                EmptyText::NoAssigner,
                None,
            ),
            text(" (assigner) and "),
            slot_list(
                &path,
                p.id,
                ListKind::Entities(EntityRole::Assignee),
                entity_items(
                    &path,
                    EntityRole::Assignee,
                    &p.assignee,
                    "party",
                    SlotKind::Party,
                    false,
                ),
                vec![],
                Conj::And,
                EmptyText::AnyoneObject,
                None,
            ),
            text(" (assignee)."),
        ],
    };
    let conflict_raw = p.conflict.clone().unwrap_or_default();
    let conflict_display = match p.conflict.as_deref() {
        None | Some("") => "not stated".to_string(),
        Some(c) => match local_name(c) {
            "perm" => "the permission wins".into(),
            "prohibit" => "the prohibition wins".into(),
            "invalid" => "the whole policy is void".into(),
            other => format!("resolved by the strategy {other}"),
        },
    };
    out.notes = vec![
        Sentence {
            segments: vec![
                text("Applies to "),
                slot_list(
                    &path,
                    p.id,
                    ListKind::Entities(EntityRole::Target),
                    entity_items(
                        &path,
                        EntityRole::Target,
                        &p.target,
                        "asset",
                        SlotKind::Asset,
                        false,
                    ),
                    vec![],
                    Conj::And,
                    EmptyText::Nothing,
                    None,
                ),
                text("."),
            ],
        },
        Sentence {
            segments: vec![
                text("Covers the action "),
                slot_list(
                    &path,
                    p.id,
                    ListKind::Actions,
                    action_items(&path, &p.action),
                    vec![],
                    Conj::And,
                    EmptyText::Nothing,
                    None,
                ),
                text("."),
            ],
        },
        Sentence {
            segments: vec![
                text("Written to the profile "),
                slot_list(
                    &path,
                    p.id,
                    ListKind::Profile,
                    string_items(&path, p.id, &p.profile, true),
                    vec![],
                    Conj::And,
                    EmptyText::NoProfile,
                    None,
                ),
                text("."),
            ],
        },
        Sentence {
            segments: vec![
                text("If a permission and a prohibition conflict: "),
                Segment::Choice(ChoiceSlot {
                    path: SlotPath {
                        node: path.clone(),
                        field: Field::Conflict,
                    },
                    owner: p.id,
                    kind: ChoiceKind::Conflict,
                    raw: conflict_raw,
                    display: conflict_display,
                    left_operand: String::new(),
                }),
                text("."),
            ],
        },
        Sentence {
            segments: vec![
                text("Inherits the rules of "),
                slot_list(
                    &path,
                    p.id,
                    ListKind::InheritFrom,
                    string_items(&path, p.id, &p.inherit_from, false),
                    vec![],
                    Conj::And,
                    EmptyText::NoParent,
                    None,
                ),
                text("."),
            ],
        },
    ];
    out.refinements = refinement_groups(&path, &p.action);

    let inh = Inh {
        assigner: entity_inherited(
            &path,
            EntityRole::Assigner,
            &p.assigner,
            "party",
            SlotKind::Party,
        ),
        assignee: entity_inherited(
            &path,
            EntityRole::Assignee,
            &p.assignee,
            "party",
            SlotKind::Party,
        ),
        target: entity_inherited(
            &path,
            EntityRole::Target,
            &p.target,
            "asset",
            SlotKind::Asset,
        ),
        action: action_inherited(&path, &p.action),
    };
    for (kind, list) in [
        (RuleList::Permission, &p.permission),
        (RuleList::Prohibition, &p.prohibition),
        (RuleList::Obligation, &p.obligation),
    ] {
        for (idx, r) in list.iter().enumerate() {
            out.rules.push(rule_prose(
                path.child(Step::Rule(kind, idx)),
                kind,
                idx,
                r,
                &inh,
            ));
        }
    }
    out.add_rule_lists = [
        RuleList::Permission,
        RuleList::Prohibition,
        RuleList::Obligation,
    ]
    .into_iter()
    .map(|k| ListPath::of(&path, ListKind::Rules(k)))
    .collect();
    out
}

fn refinement_groups(owner: &NodePath, actions: &[ActionNode]) -> Vec<RefinementGroup> {
    actions
        .iter()
        .enumerate()
        .filter(|(_, a)| a.locked.is_none())
        .map(|(j, a)| {
            let action = owner.child(Step::Action(j));
            RefinementGroup {
                action_name: words::action(&a.name),
                conditions: ConditionList {
                    list: ListPath::of(&action, ListKind::Refinements),
                    owner: a.id,
                    in_duty: false,
                    items: cond_items(&action, Step::Refinement, &a.refinement, false),
                },
                action,
            }
        })
        .collect()
}

fn rule_prose(path: NodePath, list: RuleList, index: usize, r: &RuleNode, inh: &Inh) -> RuleProse {
    let in_duty = !matches!(list, RuleList::Permission | RuleList::Prohibition);
    let in_duty_label = matches!(
        list,
        RuleList::Obligation | RuleList::Duty | RuleList::Remedy | RuleList::Consequence
    );
    let empty_conditions = |owner: NodeId| ConditionList {
        list: ListPath::of(&path, ListKind::Constraints),
        owner,
        in_duty: in_duty_label,
        items: vec![],
    };
    let mut out = RuleProse {
        node: r.id,
        path: path.clone(),
        list,
        index,
        locked: locked_of(&r.locked, r.origin.as_ref()),
        sentence: Sentence::default(),
        refinements: vec![],
        conditions: empty_conditions(r.id),
        follow_ups: vec![],
    };
    if r.locked.is_some() {
        return out;
    }
    if let Some(reference) = &r.reference {
        out.sentence = Sentence {
            segments: vec![
                text("The rule "),
                Segment::Slot(slot(
                    &path,
                    Field::Reference,
                    r.id,
                    SlotKind::Reference,
                    reference,
                    reference.clone(),
                )),
                text(" is referenced here but not defined."),
            ],
        };
        return out;
    }
    let top = list.is_top_level();
    let own = |own_empty: bool, inherited: &Vec<Slot>| {
        if own_empty { inherited.clone() } else { vec![] }
    };
    let by = match list {
        RuleList::Permission => ", as permitted by ",
        RuleList::Prohibition => ", as prohibited by ",
        _ => ", as required by ",
    };
    let mut segs = vec![
        slot_list(
            &path,
            r.id,
            ListKind::Entities(EntityRole::Assignee),
            entity_items(
                &path,
                EntityRole::Assignee,
                &r.assignee,
                "party",
                SlotKind::Party,
                in_duty_label,
            ),
            own(r.assignee.is_empty(), &inh.assignee),
            Conj::And,
            if in_duty {
                EmptyText::TheAssignee
            } else {
                EmptyText::AnyoneSubject
            },
            None,
        ),
        text(" "),
        Segment::RuleKind(RuleKindSlot {
            rule: path.clone(),
            owner: r.id,
            current: list,
            changeable: top,
        }),
        text(" "),
        slot_list(
            &path,
            r.id,
            ListKind::Actions,
            action_items(&path, &r.action),
            own(r.action.is_empty(), &inh.action),
            Conj::And,
            EmptyText::UnspecifiedAction,
            None,
        ),
    ];
    let target_items = entity_items(
        &path,
        EntityRole::Target,
        &r.target,
        "asset",
        SlotKind::Asset,
        in_duty_label,
    );
    let target_inherited = own(r.target.is_empty(), &inh.target);
    if in_duty {
        segs.push(slot_list(
            &path,
            r.id,
            ListKind::Entities(EntityRole::Target),
            target_items,
            target_inherited,
            Conj::And,
            EmptyText::Nothing,
            Some(" "),
        ));
    } else {
        segs.push(text(" "));
        segs.push(slot_list(
            &path,
            r.id,
            ListKind::Entities(EntityRole::Target),
            target_items,
            target_inherited,
            Conj::And,
            EmptyText::UnspecifiedAsset,
            None,
        ));
    }
    segs.push(slot_list(
        &path,
        r.id,
        ListKind::Entities(EntityRole::Assigner),
        entity_items(
            &path,
            EntityRole::Assigner,
            &r.assigner,
            "party",
            SlotKind::Party,
            in_duty_label,
        ),
        own(r.assigner.is_empty(), &inh.assigner),
        Conj::And,
        EmptyText::Nothing,
        Some(by),
    ));
    segs.push(text("."));
    out.sentence = Sentence { segments: segs };
    out.refinements = refinement_groups(&path, &r.action);
    out.conditions = ConditionList {
        list: ListPath::of(&path, ListKind::Constraints),
        owner: r.id,
        in_duty: in_duty_label,
        items: cond_items(&path, Step::Constraint, &r.constraint, in_duty_label),
    };

    // Duties, remedies and consequences inherit only the effective parties.
    let child_inh = Inh {
        assigner: if r.assigner.is_empty() {
            inh.assigner.clone()
        } else {
            entity_inherited(
                &path,
                EntityRole::Assigner,
                &r.assigner,
                "party",
                SlotKind::Party,
            )
        },
        assignee: if r.assignee.is_empty() {
            inh.assignee.clone()
        } else {
            entity_inherited(
                &path,
                EntityRole::Assignee,
                &r.assignee,
                "party",
                SlotKind::Party,
            )
        },
        ..Inh::default()
    };
    for (kind, children) in [
        (RuleList::Duty, &r.duty),
        (RuleList::Remedy, &r.remedy),
        (RuleList::Consequence, &r.consequence),
    ] {
        let in_pos = odrl_position(Some(list), kind);
        if children.is_empty() && !in_pos {
            continue;
        }
        out.follow_ups.push(FollowUpProse {
            list: ListPath::of(&path, ListKind::Rules(kind)),
            kind,
            parent: list,
            in_odrl_position: in_pos,
            rules: children
                .iter()
                .enumerate()
                .map(|(i, c)| rule_prose(path.child(Step::Rule(kind, i)), kind, i, c, &child_inh))
                .collect(),
        });
    }
    out
}

fn cond_items(
    owner: &NodePath,
    step: fn(usize) -> Step,
    list: &[ConstraintNode],
    in_duty: bool,
) -> Vec<ConditionProse> {
    list.iter()
        .enumerate()
        .map(|(i, c)| cond_prose(owner.child(step(i)), i, c, in_duty))
        .collect()
}

fn is_or_operator(op: &str) -> bool {
    matches!(local_name(op), "isAnyOf" | "isNoneOf")
}

fn cond_prose(path: NodePath, index: usize, c: &ConstraintNode, in_duty: bool) -> ConditionProse {
    let mut out = ConditionProse {
        node: c.id(),
        path: path.clone(),
        index,
        locked: None,
        sentence: Sentence::default(),
        children: None,
    };
    match c {
        ConstraintNode::Atomic(a) => {
            let left_display = if a.left.is_empty() {
                String::new()
            } else {
                words::operand(&a.left)
            };
            let op_display = if a.operator.is_empty() {
                String::new()
            } else {
                words::operator_for(&a.left, &a.operator)
            };
            let mut segs = vec![
                Segment::Slot(slot(
                    &path,
                    Field::LeftOperand,
                    a.id,
                    SlotKind::LeftOperand,
                    &a.left,
                    left_display,
                )),
                text(" "),
                Segment::Choice(ChoiceSlot {
                    path: SlotPath {
                        node: path.clone(),
                        field: Field::Operator,
                    },
                    owner: a.id,
                    kind: ChoiceKind::Operator,
                    raw: a.operator.clone(),
                    display: op_display,
                    left_operand: a.left.clone(),
                }),
                text(" "),
            ];
            match &a.right {
                RightOperand::Values(vals) => {
                    let items = vals
                        .iter()
                        .enumerate()
                        .map(|(k, l)| ListItem {
                            node: None,
                            index: k,
                            locked: None,
                            slot: slot(
                                &path,
                                Field::RightOperand(k),
                                a.id,
                                SlotKind::Literal,
                                &l.raw(),
                                l.display(),
                            ),
                            extra: vec![],
                        })
                        .collect();
                    segs.push(slot_list(
                        &path,
                        a.id,
                        ListKind::RightOperand,
                        items,
                        vec![],
                        if is_or_operator(&a.operator) {
                            Conj::Or
                        } else {
                            Conj::And
                        },
                        EmptyText::Nothing,
                        None,
                    ));
                }
                RightOperand::Reference(r) => {
                    segs.push(text("the value at "));
                    segs.push(Segment::Slot(slot(
                        &path,
                        Field::OperandReference,
                        a.id,
                        SlotKind::OperandReference,
                        r,
                        r.clone(),
                    )));
                }
                RightOperand::Missing => segs.push(text("an unspecified value")),
            }
            if let Some(u) = &a.unit {
                segs.push(text(" "));
                segs.push(Segment::Slot(slot(
                    &path,
                    Field::Unit,
                    a.id,
                    SlotKind::Unit,
                    u,
                    words::humanise(local_name(u)),
                )));
            }
            out.sentence = Sentence { segments: segs };
        }
        ConstraintNode::Logical(l) => {
            out.sentence = Sentence {
                segments: vec![Segment::Choice(ChoiceSlot {
                    path: SlotPath {
                        node: path.clone(),
                        field: Field::LogicalOp,
                    },
                    owner: l.id,
                    kind: ChoiceKind::LogicalOp,
                    raw: l.op.as_str().to_string(),
                    display: l.op.phrase().to_string(),
                    left_operand: String::new(),
                })],
            };
            out.children = Some(ConditionList {
                list: ListPath::of(&path, ListKind::Children),
                owner: l.id,
                in_duty,
                items: cond_items(&path, Step::Child, &l.children, in_duty),
            });
        }
        ConstraintNode::Reference { id, iri, .. } => {
            out.sentence = Sentence {
                segments: vec![
                    text("the constraint "),
                    Segment::Slot(slot(
                        &path,
                        Field::Reference,
                        *id,
                        SlotKind::Reference,
                        iri,
                        iri.clone(),
                    )),
                    text(" holds"),
                ],
            };
        }
        ConstraintNode::Opaque { origin, reason, .. } => {
            out.locked = Some(locked(reason, Some(origin)));
        }
    }
    out
}

/// A condition on one line, for places a nested list does not fit.
fn inline_condition(c: &ConditionProse) -> String {
    let head = c.sentence.plain();
    match &c.children {
        None => head,
        Some(list) => {
            let kids: Vec<String> = list.items.iter().map(inline_condition).collect();
            format!("{head}: {}", kids.join("; "))
        }
    }
}
