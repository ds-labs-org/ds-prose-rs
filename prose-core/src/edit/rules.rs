//! What an editor may offer and the reducer will accept.
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
    pub rule_assigner: Limit,
    pub rule_assignee: Limit,
    pub rule_target: Limit,
    pub rule_action: Limit,
    pub action_refinement: Limit,
    pub entity_refinement: Limit,
    pub constraints: Limit,
    pub logical_children: Limit,
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
            rule_assigner: Limit::ANY,
            rule_assignee: Limit::ANY,
            rule_target: Limit::ANY,
            rule_action: Limit::at_least(1),
            action_refinement: Limit::ANY,
            entity_refinement: Limit::ANY,
            constraints: Limit::ANY,
            logical_children: Limit::ANY,
            right_operand_values: Limit::at_least(1),
            profile: Limit::ANY,
            inherit_from: Limit::ANY,
            set_operators: ["isAnyOf", "isAllOf", "isNoneOf"]
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
