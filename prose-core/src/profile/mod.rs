//! ODRL profiles: the vocabulary a policy's `profile` points to, read so the
//! prose can use the profile's own labels and definitions.
//!
//! A profile widens and annotates. It never closes an input: terms it does
//! not declare still read and edit as before, and anything a profile says
//! about a policy is advisory. Profile text is third-party input and is made
//! plain text before it is kept.
mod text;

pub use text::LangText;
