//! Pins `#[non_exhaustive]` on the enums README.md lists under Compatibility.
//!
//! A doctest compiles as another crate, so each match below, which names every
//! variant and then adds a wildcard, has a reachable wildcard only while the
//! enum is `#[non_exhaustive]`; without the attribute `unreachable_patterns`
//! (denied) fails it. A variant added later keeps the test green; dropping the
//! attribute does not.
//!
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::EditEvent;
//! fn f(x: &EditEvent) -> u8 {
//!     match x {
//!         EditEvent::SetText { .. } => 0,
//!         EditEvent::SetChoice { .. } => 0,
//!         EditEvent::Add { .. } => 0,
//!         EditEvent::Remove { .. } => 0,
//!         EditEvent::Move { .. } => 0,
//!         EditEvent::ChangeRuleKind { .. } => 0,
//!         EditEvent::Wrap { .. } => 0,
//!         EditEvent::Unwrap { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::EditError;
//! fn f(x: &EditError) -> u8 {
//!     match x {
//!         EditError::NoSuchPath { .. } => 0,
//!         EditError::Stale { .. } => 0,
//!         EditError::NotAllowed { .. } => 0,
//!         EditError::Locked { .. } => 0,
//!         EditError::Invalid { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::NewItem;
//! fn f(x: &NewItem) -> u8 {
//!     match x {
//!         NewItem::Default { .. } => 0,
//!         NewItem::Logical { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::FocusTarget;
//! fn f(x: &FocusTarget) -> u8 {
//!     match x {
//!         FocusTarget::Slot { .. } => 0,
//!         FocusTarget::AddButton { .. } => 0,
//!         FocusTarget::Node { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::ListKind;
//! fn f(x: &ListKind) -> u8 {
//!     match x {
//!         ListKind::Policies { .. } => 0,
//!         ListKind::Rules { .. } => 0,
//!         ListKind::Entities { .. } => 0,
//!         ListKind::Actions { .. } => 0,
//!         ListKind::Refinements { .. } => 0,
//!         ListKind::Constraints { .. } => 0,
//!         ListKind::Children { .. } => 0,
//!         ListKind::RightOperand { .. } => 0,
//!         ListKind::Profile { .. } => 0,
//!         ListKind::InheritFrom { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::Field;
//! fn f(x: &Field) -> u8 {
//!     match x {
//!         Field::Kind { .. } => 0,
//!         Field::Uid { .. } => 0,
//!         Field::Conflict { .. } => 0,
//!         Field::Profile { .. } => 0,
//!         Field::InheritFrom { .. } => 0,
//!         Field::Reference { .. } => 0,
//!         Field::Iri { .. } => 0,
//!         Field::PartOf { .. } => 0,
//!         Field::Name { .. } => 0,
//!         Field::LeftOperand { .. } => 0,
//!         Field::Operator { .. } => 0,
//!         Field::RightOperand { .. } => 0,
//!         Field::OperandReference { .. } => 0,
//!         Field::Unit { .. } => 0,
//!         Field::LogicalOp { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::Step;
//! fn f(x: &Step) -> u8 {
//!     match x {
//!         Step::Rule { .. } => 0,
//!         Step::Entity { .. } => 0,
//!         Step::Action { .. } => 0,
//!         Step::Refinement { .. } => 0,
//!         Step::Constraint { .. } => 0,
//!         Step::Child { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::NodeRef;
//! fn f(x: &NodeRef<'_>) -> u8 {
//!     match x {
//!         NodeRef::Policy { .. } => 0,
//!         NodeRef::Rule { .. } => 0,
//!         NodeRef::Entity { .. } => 0,
//!         NodeRef::Action { .. } => 0,
//!         NodeRef::Constraint { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::Segment;
//! fn f(x: &Segment) -> u8 {
//!     match x {
//!         Segment::Text { .. } => 0,
//!         Segment::Slot { .. } => 0,
//!         Segment::Choice { .. } => 0,
//!         Segment::RuleKind { .. } => 0,
//!         Segment::List { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::SlotKind;
//! fn f(x: &SlotKind) -> u8 {
//!     match x {
//!         SlotKind::PolicyKind { .. } => 0,
//!         SlotKind::Uid { .. } => 0,
//!         SlotKind::Party { .. } => 0,
//!         SlotKind::Asset { .. } => 0,
//!         SlotKind::Action { .. } => 0,
//!         SlotKind::LeftOperand { .. } => 0,
//!         SlotKind::Literal { .. } => 0,
//!         SlotKind::Unit { .. } => 0,
//!         SlotKind::OperandReference { .. } => 0,
//!         SlotKind::PartOf { .. } => 0,
//!         SlotKind::Profile { .. } => 0,
//!         SlotKind::PolicyRef { .. } => 0,
//!         SlotKind::Reference { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::ChoiceKind;
//! fn f(x: &ChoiceKind) -> u8 {
//!     match x {
//!         ChoiceKind::Operator { .. } => 0,
//!         ChoiceKind::LogicalOp { .. } => 0,
//!         ChoiceKind::Conflict { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::EmptyText;
//! fn f(x: &EmptyText) -> u8 {
//!     match x {
//!         EmptyText::Nothing { .. } => 0,
//!         EmptyText::AnyoneSubject { .. } => 0,
//!         EmptyText::AnyoneObject { .. } => 0,
//!         EmptyText::TheAssignee { .. } => 0,
//!         EmptyText::UnspecifiedAsset { .. } => 0,
//!         EmptyText::UnspecifiedAction { .. } => 0,
//!         EmptyText::NoAssigner { .. } => 0,
//!         EmptyText::NoParent { .. } => 0,
//!         EmptyText::NoProfile { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::edit::IssueTarget;
//! fn f(x: &IssueTarget) -> u8 {
//!     match x {
//!         IssueTarget::Doc { .. } => 0,
//!         IssueTarget::Node { .. } => 0,
//!         IssueTarget::Slot { .. } => 0,
//!         IssueTarget::List { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
