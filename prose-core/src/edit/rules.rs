//! What an editor may offer and the reducer will accept.
use super::apply::NewItem;
use super::model::*;
use super::path::*;
use crate::words::local_name;

/// How many items a list may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub min: usize,
    pub max: Option<usize>,
}

impl Limit {
    pub const ANY: Limit = Limit { min: 0, max: None };
    /// Never offered; adding is rejected.
    pub const NONE: Limit = Limit {
        min: 0,
        max: Some(0),
    };
    pub const fn exactly(n: usize) -> Limit {
        Limit {
            min: n,
            max: Some(n),
        }
    }
    pub const fn at_most(n: usize) -> Limit {
        Limit {
            min: 0,
            max: Some(n),
        }
    }
    pub const fn at_least(n: usize) -> Limit {
        Limit { min: n, max: None }
    }
}

/// What a freshly added node starts with.
#[derive(Debug, Clone, PartialEq)]
pub struct NewNodeDefaults {
    pub policy_kind: String,
    pub uid_prefix: String,
    pub action: String,
    pub left_operand: String,
    pub operator: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditRules {
    pub policies: Limit,
    pub permission: Limit,
    pub prohibition: Limit,
    pub obligation: Limit,
    pub duty: Limit,
    pub remedy: Limit,
    pub consequence: Limit,
    pub policy_assigner: Limit,
    pub policy_assignee: Limit,
    pub policy_target: Limit,
    pub policy_action: Limit,
    /// The policy's own identifier: `max: Some(0)` when the host holds it
    /// elsewhere. It is then not set on a new policy and cannot be typed.
    pub policy_uid: Limit,
    pub rule_assigner: Limit,
    pub rule_assignee: Limit,
    pub rule_target: Limit,
    pub rule_action: Limit,
    pub action_refinement: Limit,
    pub entity_refinement: Limit,
    pub constraints: Limit,
    pub logical_children: Limit,
    /// How deep `and`/`or`/`xone` groups may nest: `max` is the deepest
    /// level a group may be at (a group directly in a constraint list is at
    /// level 1, one inside it at level 2), and `min` is not used. Applies to
    /// adding and wrapping; a document read with deeper groups keeps them,
    /// and ungrouping is always allowed.
    pub logical_depth: Limit,
    /// Applies to operators in `set_operators`; any other operator is
    /// capped at one value.
    pub right_operand_values: Limit,
    pub profile: Limit,
    pub inherit_from: Limit,
    /// Compared after `wording::local_name`.
    pub set_operators: Vec<String>,
    pub allow_rule_kind_change: bool,
    pub allow_reorder: bool,
    pub allow_logical: bool,
    pub defaults: NewNodeDefaults,
}

impl Default for EditRules {
    /// The permissive ODRL JSON-LD preset.
    fn default() -> Self {
        EditRules {
            policies: Limit::at_least(1),
            permission: Limit::ANY,
            prohibition: Limit::ANY,
            obligation: Limit::ANY,
            duty: Limit::ANY,
            remedy: Limit::ANY,
            consequence: Limit::ANY,
            policy_assigner: Limit::ANY,
            policy_assignee: Limit::ANY,
            policy_target: Limit::ANY,
            policy_action: Limit::ANY,
            policy_uid: Limit::at_most(1),
            rule_assigner: Limit::ANY,
            rule_assignee: Limit::ANY,
            rule_target: Limit::ANY,
            rule_action: Limit::at_least(1),
            action_refinement: Limit::ANY,
            entity_refinement: Limit::ANY,
            constraints: Limit::ANY,
            logical_children: Limit::ANY,
            logical_depth: Limit::ANY,
            right_operand_values: Limit::at_least(1),
            profile: Limit::ANY,
            inherit_from: Limit::ANY,
            // ODRL22.ttl defines all six as set-based; matches
            // odrl.set-operator-cardinality.
            set_operators: [
                "isA", "hasPart", "isPartOf", "isAnyOf", "isAllOf", "isNoneOf",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            allow_rule_kind_change: true,
            allow_reorder: true,
            allow_logical: true,
            defaults: NewNodeDefaults {
                policy_kind: "Set".into(),
                uid_prefix: "urn:policy:".into(),
                action: "use".into(),
                left_operand: String::new(),
                operator: "eq".into(),
            },
        }
    }
}

/// Whether ODRL itself defines this follow-up list under this parent: a
/// permission has duties, a prohibition remedies, and obligations, duties,
/// remedies and consequences have consequences.
pub fn odrl_position(parent: Option<RuleList>, list: RuleList) -> bool {
    use RuleList::*;
    matches!(
        (parent, list),
        (None, Permission | Prohibition | Obligation)
            | (Some(Permission), Duty)
            | (Some(Prohibition), Remedy)
            | (Some(Obligation | Duty | Remedy | Consequence), Consequence)
    )
}

impl EditRules {
    pub fn is_set_operator(&self, op: &str) -> bool {
        let local = local_name(op);
        self.set_operators.iter().any(|s| local_name(s) == local)
    }

    /// Whether an operator may be switched from `from` to `to` on a
    /// constraint holding `values` right operands. A set operator may be
    /// switched to a single-value one only down to one value; a constraint
    /// that already holds several values under a single-value operator
    /// loses nothing by another single-value one.
    pub fn operator_switch_allowed(&self, from: &str, to: &str, values: usize) -> bool {
        values <= 1 || self.is_set_operator(to) || !self.is_set_operator(from)
    }

    pub fn limit(&self, doc: &EditDoc, list: &ListPath) -> Limit {
        let owner_is_policy = list.owner.as_ref().is_some_and(|o| o.steps.is_empty());
        match list.kind {
            ListKind::Policies => self.policies,
            ListKind::Rules(RuleList::Permission) => self.permission,
            ListKind::Rules(RuleList::Prohibition) => self.prohibition,
            ListKind::Rules(RuleList::Obligation) => self.obligation,
            ListKind::Rules(RuleList::Duty) => self.duty,
            ListKind::Rules(RuleList::Remedy) => self.remedy,
            ListKind::Rules(RuleList::Consequence) => self.consequence,
            ListKind::Entities(role) => match (owner_is_policy, role) {
                (true, EntityRole::Assigner) => self.policy_assigner,
                (true, EntityRole::Assignee) => self.policy_assignee,
                (true, EntityRole::Target) => self.policy_target,
                (false, EntityRole::Assigner) => self.rule_assigner,
                (false, EntityRole::Assignee) => self.rule_assignee,
                (false, EntityRole::Target) => self.rule_target,
            },
            ListKind::Actions => {
                if owner_is_policy {
                    self.policy_action
                } else {
                    self.rule_action
                }
            }
            ListKind::Refinements => match list.owner.as_ref().and_then(NodePath::last) {
                Some(Step::Entity(..)) => self.entity_refinement,
                _ => self.action_refinement,
            },
            ListKind::Constraints => self.constraints,
            ListKind::Children => self.logical_children,
            ListKind::RightOperand => {
                let set = list
                    .owner
                    .as_ref()
                    .and_then(|o| doc.node(o))
                    .is_some_and(|n| match n {
                        NodeRef::Constraint(ConstraintNode::Atomic(a)) => {
                            self.is_set_operator(&a.operator)
                        }
                        _ => false,
                    });
                if set {
                    self.right_operand_values
                } else {
                    Limit {
                        min: self.right_operand_values.min.min(1),
                        max: Some(1),
                    }
                }
            }
            ListKind::Profile => self.profile,
            ListKind::InheritFrom => self.inherit_from,
        }
    }

    pub fn can_add(&self, doc: &EditDoc, list: &ListPath) -> bool {
        self.add_refusal(doc, list).is_none()
    }

    /// Why an add would be refused, as a message; `None` when allowed.
    pub(crate) fn add_refusal(&self, doc: &EditDoc, list: &ListPath) -> Option<String> {
        let Some((_, len)) = list_info(doc, list) else {
            return Some("no such list".into());
        };
        if let Some(owner) = &list.owner {
            match resolve_locked(doc, owner) {
                None => return Some("no such list".into()),
                Some((_, Some(reason))) => return Some(format!("locked: {reason}")),
                Some((node, None)) => {
                    if matches!(node, NodeRef::Rule(r) if r.reference.is_some()) {
                        return Some("a rule given only by reference has nothing to add to".into());
                    }
                    if let ListKind::Rules(kind) = list.kind {
                        let parent = owner.rule_chain().last().copied();
                        if !kind.is_top_level() && !odrl_position(parent, kind) {
                            return Some(format!(
                                "ODRL has no {} under a {}",
                                kind.as_str(),
                                parent.map_or("policy", RuleList::as_str)
                            ));
                        }
                    }
                    match (list.kind, node) {
                        (
                            ListKind::RightOperand,
                            NodeRef::Constraint(ConstraintNode::Atomic(a)),
                        ) if !matches!(a.right, RightOperand::Values(_)) => {
                            return Some("the value is not a list of literals".into());
                        }
                        _ => {}
                    }
                }
            }
        }
        match self.limit(doc, list).max {
            Some(max) if len >= max => Some(format!("at most {max} allowed")),
            _ => None,
        }
    }

    /// How many groups enclose the node at `path`.
    fn enclosing_groups(path: &NodePath) -> usize {
        path.steps
            .iter()
            .filter(|s| matches!(s, Step::Child(_)))
            .count()
    }

    /// Why a group would nest too deep at `depth`; `None` when it fits.
    fn depth_refusal(&self, depth: usize) -> Option<String> {
        match self.logical_depth.max {
            Some(max) if depth > max => Some(match max {
                0 => "groups are not allowed".to_string(),
                1 => "groups may not nest".to_string(),
                _ => format!("groups may nest at most {max} deep"),
            }),
            _ => None,
        }
    }

    /// As [`EditRules::add_refusal`], for adding `item`: a new group also
    /// has to fit [`EditRules::logical_depth`].
    pub(crate) fn add_item_refusal(
        &self,
        doc: &EditDoc,
        list: &ListPath,
        item: NewItem,
    ) -> Option<String> {
        if let Some(why) = self.add_refusal(doc, list) {
            return Some(why);
        }
        if matches!(item, NewItem::Logical(_)) {
            let owner = list.owner.as_ref()?;
            let inside = usize::from(list.kind == ListKind::Children);
            return self.depth_refusal(Self::enclosing_groups(owner) + inside + 1);
        }
        None
    }

    /// Whether `item` may be added to `list`: [`EditRules::can_add`], and a
    /// group within [`EditRules::logical_depth`].
    pub fn can_add_item(&self, doc: &EditDoc, list: &ListPath, item: NewItem) -> bool {
        self.add_item_refusal(doc, list, item).is_none()
    }

    /// Why wrapping the constraint at `path` in a group is refused by
    /// [`EditRules::logical_depth`]; `None` when it fits.
    pub(crate) fn wrap_refusal(&self, doc: &EditDoc, path: &NodePath) -> Option<String> {
        fn height(c: &ConstraintNode) -> usize {
            match c {
                ConstraintNode::Logical(l) => 1 + l.children.iter().map(height).max().unwrap_or(0),
                _ => 0,
            }
        }
        let below = match doc.node(path) {
            Some(NodeRef::Constraint(c)) => height(c),
            _ => 0,
        };
        self.depth_refusal(Self::enclosing_groups(path) + 1 + below)
    }

    /// Whether the constraint at `path` may be wrapped in a group within
    /// [`EditRules::logical_depth`].
    pub fn can_wrap(&self, doc: &EditDoc, path: &NodePath) -> bool {
        self.allow_logical && self.wrap_refusal(doc, path).is_none()
    }

    pub fn can_remove(&self, doc: &EditDoc, list: &ListPath) -> bool {
        self.remove_refusal(doc, list).is_none()
    }

    pub(crate) fn remove_refusal(&self, doc: &EditDoc, list: &ListPath) -> Option<String> {
        let Some((_, len)) = list_info(doc, list) else {
            return Some("no such list".into());
        };
        if let Some(owner) = &list.owner {
            match resolve_locked(doc, owner) {
                None => return Some("no such list".into()),
                Some((_, Some(reason))) => return Some(format!("locked: {reason}")),
                Some(_) => {}
            }
        }
        let min = self.limit(doc, list).min;
        (len <= min).then(|| format!("at least {min} required"))
    }
}
