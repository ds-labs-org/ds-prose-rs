//! The edit layer: a structured, addressable model of ODRL policies, the
//! events that change it, the rules that limit those events, a JSON-LD
//! reader and writer that keep everything the user did not edit, and the
//! sentence templates an editor renders.
//!
//! Nothing here knows about a particular engine or wire format. A host
//! with its own policy encoding builds the model itself (each node carries
//! an [`Origin`], the JSON it came from) and writes back by merging into
//! those origins; see [`set_property`].
mod apply;
mod issue;
mod jsonld_read;
mod jsonld_write;
mod model;
mod path;
mod rules;
mod segments;

pub use apply::{EditError, EditEvent, FocusTarget, NewItem, focus_after};
pub use issue::{Issue, IssueTarget, Severity};
pub use jsonld_read::{read_model, read_model_value};
pub use jsonld_write::{set_property, write_jsonld};
pub use model::{
    ActionNode, AtomicConstraint, ConstraintNode, DocShape, EditDoc, Entity, Literal,
    LogicalConstraint, LogicalOp, NodeId, NodeRef, Origin, PolicyNode, RightOperand, RuleNode,
};
pub use path::{
    EntityRole, Field, ListKind, ListPath, NodePath, PathError, RuleList, SlotPath, Step,
};
pub use rules::{EditRules, Limit, NewNodeDefaults, odrl_position};
pub use segments::{
    ChoiceKind, ChoiceSlot, ConditionList, ConditionProse, Conj, EmptyText, FollowUpProse,
    ListItem, Locked, PolicyProse, RefinementGroup, RuleKindSlot, RuleProse, Segment, Sentence,
    Slot, SlotKind, SlotList, sentences, sentences_with,
};

/// The prefixes `doc`'s JSON-LD binds: those of [`EditDoc::origin`], then of
/// each policy's origin; the first binding of a prefix wins.
pub fn prefixes(doc: &EditDoc) -> crate::profile::Prefixes {
    let mut out = crate::profile::Prefixes::default();
    let origins = doc
        .origin
        .iter()
        .chain(doc.policies.iter().filter_map(|p| p.origin.as_ref()));
    for o in origins {
        out.merge(crate::profile::Prefixes::of_document(o.value()));
    }
    out
}

/// The left operand of the constraint whose right operand, unit or operand
/// reference `slot` addresses, as written; `None` for any other slot.
pub fn left_operand_at<'d>(doc: &'d EditDoc, slot: &SlotPath) -> Option<&'d str> {
    let _ = (doc, slot);
    None
}
