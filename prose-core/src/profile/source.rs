//! Runtime value sources: values the host takes from a registry or a catalog
//! (participant DIDs, credential types, asset ids, purposes) and hands to the
//! editor per left operand, or per party role. They change daily, so they do
//! not belong in a profile; the editor never fetches them, the host does, and
//! caches them.
//!
//! A source is a list of [`SourceValue`]s (a value, and optionally the label
//! and the group a select shows it under) and whether it is closed. A closed
//! source is binding in strict validation; an open one only suggests. A host
//! source sits on top of the hints layers: profile < overlay < shapes < host,
//! for the values facet alone ([`Wording::with_value_sources`](super::Wording::with_value_sources)).
//!
//! A value that leaves the source later is never dropped from a document: it
//! stays, and a closed source flags it (`prose.value-not-allowed`, whose
//! `expected.source` is `"host"`). An empty source is no source: a registry
//! fetch that failed must not flag every value.
use crate::edit::EntityRole;

/// One value a source offers: what is written into the policy, and how the
/// editor shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceValue {
    value: String,
    label: Option<String>,
    group: Option<String>,
}

impl SourceValue {
    /// A value with no label and no group; the select shows the value itself.
    pub fn new(value: &str) -> SourceValue {
        SourceValue {
            value: value.to_string(),
            label: None,
            group: None,
        }
    }

    /// The text a select shows for the value.
    pub fn with_label(mut self, label: &str) -> SourceValue {
        self.label = Some(label.to_string());
        self
    }

    /// The `<optgroup>` heading the value is listed under.
    pub fn with_group(mut self, group: &str) -> SourceValue {
        self.group = Some(group.to_string());
        self
    }

    /// What is written into the policy: an IRI (a DID, a term) or any text.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The label shown for the value, if the host gave one.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// The group the value is listed under, if the host gave one.
    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }
}

/// The values a host gives for one left operand or one party role.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValueSource {
    values: Vec<SourceValue>,
    closed: bool,
}

impl ValueSource {
    /// `closed`: the values are the only ones strict validation takes.
    /// Otherwise they are suggestions. A value listed twice is kept once, at
    /// its first place.
    pub fn new(values: Vec<SourceValue>, closed: bool) -> ValueSource {
        let mut seen = std::collections::HashSet::new();
        let values = values
            .into_iter()
            .filter(|v| seen.insert(v.value.clone()))
            .collect();
        ValueSource { values, closed }
    }

    /// The values, in the host's order.
    pub fn values(&self) -> &[SourceValue] {
        &self.values
    }

    /// Whether the source is closed: a value it does not list is a finding.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Whether the source lists nothing, which makes it no source at all.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Whether `value`, written as it is, is one of the listed values.
    pub fn contains(&self, value: &str) -> bool {
        self.values.iter().any(|v| v.value == value)
    }
}

/// Every source a host passed: by the IRI of a left operand, and by party
/// role (assigner, assignee, target).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValueSources {
    operands: Vec<(String, ValueSource)>,
    roles: Vec<(EntityRole, ValueSource)>,
}

impl ValueSources {
    /// No sources.
    pub fn new() -> ValueSources {
        ValueSources::default()
    }

    /// Whether no source is set.
    pub fn is_empty(&self) -> bool {
        self.operands.is_empty() && self.roles.is_empty()
    }

    /// Sets the source for the left operand `iri` (an expanded IRI),
    /// replacing the one it had.
    pub fn set_operand(&mut self, iri: &str, source: ValueSource) {
        match self.operands.iter_mut().find(|(i, _)| i == iri) {
            Some(slot) => slot.1 = source,
            None => self.operands.push((iri.to_string(), source)),
        }
    }

    /// Removes the source of the left operand `iri`; whether there was one.
    pub fn remove_operand(&mut self, iri: &str) -> bool {
        let before = self.operands.len();
        self.operands.retain(|(i, _)| i != iri);
        self.operands.len() != before
    }

    /// The source for the left operand `iri`, if any.
    pub fn operand(&self, iri: &str) -> Option<&ValueSource> {
        self.operands.iter().find(|(i, _)| i == iri).map(|(_, s)| s)
    }

    /// The left operands that have a source, with it, in the order they were set.
    pub fn operands(&self) -> impl Iterator<Item = (&str, &ValueSource)> {
        self.operands.iter().map(|(i, s)| (i.as_str(), s))
    }

    /// Sets the source for the parties (or, for the target, the assets) of
    /// `role`, replacing the one it had.
    pub fn set_role(&mut self, role: EntityRole, source: ValueSource) {
        match self.roles.iter_mut().find(|(r, _)| *r == role) {
            Some(slot) => slot.1 = source,
            None => self.roles.push((role, source)),
        }
    }

    /// Removes the source of `role`; whether there was one.
    pub fn remove_role(&mut self, role: EntityRole) -> bool {
        let before = self.roles.len();
        self.roles.retain(|(r, _)| *r != role);
        self.roles.len() != before
    }

    /// The source for the party role `role`, if any.
    pub fn role(&self, role: EntityRole) -> Option<&ValueSource> {
        self.roles.iter().find(|(r, _)| *r == role).map(|(_, s)| s)
    }
}
