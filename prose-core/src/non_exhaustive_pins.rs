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
//! use prose_core::edit::StrictEdit;
//! fn f(x: &StrictEdit) -> u8 {
//!     match x {
//!         StrictEdit::Flag => 0,
//!         StrictEdit::RefuseTerms => 0,
//!         _ => 1,
//!     }
//! }
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
//! use prose_core::wording::ValueShape;
//! fn f(x: &ValueShape) -> u8 {
//!     match x {
//!         ValueShape::Instant => 0,
//!         ValueShape::DateOnly => 0,
//!         ValueShape::Duration => 0,
//!         ValueShape::Other => 0,
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
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::profile::TermKind;
//! fn f(x: &TermKind) -> u8 {
//!     match x {
//!         TermKind::Action => 0,
//!         TermKind::LeftOperand => 0,
//!         TermKind::Operator => 0,
//!         TermKind::RightOperand => 0,
//!         TermKind::ConflictTerm => 0,
//!         TermKind::PolicyClass => 0,
//!         TermKind::RuleClass => 0,
//!         TermKind::AssetClass => 0,
//!         TermKind::PartyClass => 0,
//!         TermKind::PartyFunction => 0,
//!         TermKind::AssetRelation => 0,
//!         TermKind::LogicalOperand => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::profile::ProfileError;
//! fn f(x: &ProfileError) -> u8 {
//!     match x {
//!         ProfileError::TooLarge { .. } => 0,
//!         ProfileError::Json { .. } => 0,
//!         ProfileError::LooksLikeHtml => 0,
//!         ProfileError::ContextOnly => 0,
//!         ProfileError::JsonSchema => 0,
//!         ProfileError::LooksLikePolicy => 0,
//!         ProfileError::NoTerms => 0,
//!         ProfileError::UnsupportedFormat => 0,
//!         ProfileError::LimitExceeded { .. } => 0,
//!         ProfileError::Turtle { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::profile::OdrlVocab;
//! fn f(x: &OdrlVocab) -> u8 {
//!     match x {
//!         OdrlVocab::Common => 0,
//!         OdrlVocab::Core => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::validate::Mode;
//! fn f(x: &Mode) -> u8 {
//!     match x {
//!         Mode::Advisory => 0,
//!         Mode::Strict => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::validate::Tier;
//! fn f(x: &Tier) -> u8 {
//!     match x {
//!         Tier::Profile => 0,
//!         Tier::Odrl => 0,
//!         Tier::Prose => 0,
//!         Tier::Input => 0,
//!         Tier::Heuristic => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::validate::Basis;
//! fn f(x: &Basis) -> u8 {
//!     match x {
//!         Basis::Standard => 0,
//!         Basis::StandardDerived => 0,
//!         Basis::Convention => 0,
//!         Basis::Heuristic => 0,
//!         Basis::Input => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::validate::Verdict;
//! fn f(x: &Verdict) -> u8 {
//!     match x {
//!         Verdict::Conforms => 0,
//!         Verdict::Violations => 0,
//!         Verdict::Incomplete => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_core::validate::RuleId;
//! fn f(x: &RuleId) -> u8 {
//!     match x {
//!         RuleId::ProfileNotLoaded => 0,
//!         RuleId::ProfileImportNotLoaded => 0,
//!         RuleId::ProfileUndeclaredInNamespace => 0,
//!         RuleId::ProfileUnknownTerm => 0,
//!         RuleId::ProfileWrongKind => 0,
//!         RuleId::ProfileNotDeclared => 0,
//!         RuleId::ProfileConflictTerm => 0,
//!         RuleId::ProfileUnboundPrefix => 0,
//!         RuleId::ProfileDeprecatedTerm => 0,
//!         RuleId::OdrlPolicyUid => 0,
//!         RuleId::OdrlNoRule => 0,
//!         RuleId::OdrlOfferAssigner => 0,
//!         RuleId::OdrlAgreementParties => 0,
//!         RuleId::OdrlRuleTarget => 0,
//!         RuleId::OdrlRuleAction => 0,
//!         RuleId::OdrlConstraintShape => 0,
//!         RuleId::OdrlSetOperatorCardinality => 0,
//!         RuleId::OdrlConsequenceNesting => 0,
//!         RuleId::OdrlInheritanceCycle => 0,
//!         RuleId::ProseOperatorNotAllowed => 0,
//!         RuleId::ProseValueNotAllowed => 0,
//!         RuleId::ProseIriExpected => 0,
//!         RuleId::ProseUnitNotAllowed => 0,
//!         RuleId::ProseDatatypeLexical => 0,
//!         RuleId::ProseDatatypeConflict => 0,
//!         RuleId::ProseIllTypedLiteral => 0,
//!         RuleId::InputUnreadable => 0,
//!         RuleId::InputUnchecked => 0,
//!         RuleId::InputReaderNote => 0,
//!         RuleId::HeuristicOrderedOnUnordered => 0,
//!         _ => 1,
//!     }
//! }
//! ```
