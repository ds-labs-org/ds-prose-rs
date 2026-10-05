//! Findings a host computes about the current document and hands back to
//! the editor, which shows them at their targets.
use super::path::{ListPath, NodePath, SlotPath};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IssueTarget {
    Doc,
    Node(NodePath),
    Slot(SlotPath),
    List(ListPath),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub target: IssueTarget,
    pub severity: Severity,
    pub message: String,
}
