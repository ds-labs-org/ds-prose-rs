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
//! use prose_yew::FocusKind;
//! fn f(x: &FocusKind) -> u8 {
//!     match x {
//!         FocusKind::Slot { .. } => 0,
//!         FocusKind::Choice { .. } => 0,
//!         FocusKind::RuleKind { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
//! ```
//! #![deny(unreachable_patterns)]
//! #![allow(dead_code)]
//! use prose_yew::DecorationAt;
//! fn f(x: &DecorationAt) -> u8 {
//!     match x {
//!         DecorationAt::Policy { .. } => 0,
//!         DecorationAt::Rule { .. } => 0,
//!         DecorationAt::Condition { .. } => 0,
//!         _ => 1,
//!     }
//! }
//! ```
