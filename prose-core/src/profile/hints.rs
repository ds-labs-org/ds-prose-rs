//! Layering hints. A left operand's [`OperandHints`] start as what the
//! loaded profile's `prose:` triples say; each further source (the operator
//! overlay, then the companion shapes; later a host's runtime value source)
//! adds a patch over them, so the precedence is profile < overlay < shapes. A patch holds only the facets its source
//! gives (an empty list or `None` is "not given"), and carries its
//! provenance, so a finding can name the source that closed the slot.
use super::{HintSource, OperandHints};

/// Whether `patch` gives any facet at all.
pub(crate) fn gives_any(patch: &OperandHints) -> bool {
    !patch.operators.is_empty()
        || !patch.values.is_empty()
        || !patch.value_tree.is_empty()
        || !patch.literal_values.is_empty()
        || !patch.units.is_empty()
        || patch.datatype.is_some()
        || patch.pattern.is_some()
        || patch.min_inclusive.is_some()
        || patch.max_inclusive.is_some()
}

/// `base` with the facets `patch` gives laid over it, recorded as coming
/// from `source`. A facet a patch gives replaces the base's for that facet
/// alone; a patch never touches what a term is called.
///
/// A patch that gives anything closes the left operand: what its source
/// says is binding. Facets the base only suggested (the base was not closed)
/// and the patch does not give are dropped, so the patch makes binding
/// nothing but what it says; the facets of a base that was already closed
/// stay, with the source that gave them.
pub(crate) fn layer(base: &OperandHints, patch: &OperandHints, source: HintSource) -> OperandHints {
    if !gives_any(patch) {
        return base.clone();
    }
    let mut out = base.clone();
    if !base.closed {
        out.operators.clear();
        out.values.clear();
        out.value_tree.clear();
        out.literal_values.clear();
        out.units.clear();
        out.datatype = None;
    }
    if !patch.operators.is_empty() {
        out.operators = patch.operators.clone();
        out.sources.operators = source;
    }
    if !patch.values.is_empty() || !patch.value_tree.is_empty() || !patch.literal_values.is_empty()
    {
        out.values = patch.values.clone();
        out.value_tree = patch.value_tree.clone();
        out.literal_values = patch.literal_values.clone();
        out.sources.values = source;
    }
    if !patch.units.is_empty() {
        out.units = patch.units.clone();
        out.sources.units = source;
    }
    if patch.datatype.is_some() {
        out.datatype = patch.datatype.clone();
        out.sources.datatype = source;
    }
    if patch.pattern.is_some() || patch.min_inclusive.is_some() || patch.max_inclusive.is_some() {
        out.pattern = patch.pattern.clone();
        out.pattern_flags = patch.pattern_flags.clone();
        out.min_inclusive = patch.min_inclusive.clone();
        out.max_inclusive = patch.max_inclusive.clone();
        out.sources.lexical = source;
    }
    out.closed = true;
    out
}

/// `base` with an operator overlay's patch laid over it. The overlay's
/// facets replace the base's for that facet alone, recorded as
/// [`HintSource::Overlay`]. What the overlay says about closing is its own:
/// with `prose:closed true` it closes the left operand as [`layer`] does;
/// without it the facets are suggestions and the base stays as open (or as
/// closed) as it was. `prose:plural` only ever switches the plural on.
pub(crate) fn layer_overlay(base: &OperandHints, patch: &OperandHints) -> OperandHints {
    let mut out = if patch.closed {
        let mut closed = layer(base, patch, HintSource::Overlay);
        // An overlay that closes without giving a facet closes the base as it is.
        closed.closed = true;
        closed
    } else if gives_any(patch) {
        let mut out = base.clone();
        if !patch.operators.is_empty() {
            out.operators = patch.operators.clone();
            out.sources.operators = HintSource::Overlay;
        }
        if !patch.values.is_empty() || !patch.value_tree.is_empty() {
            out.values = patch.values.clone();
            out.value_tree = patch.value_tree.clone();
            out.sources.values = HintSource::Overlay;
        }
        if !patch.units.is_empty() {
            out.units = patch.units.clone();
            out.sources.units = HintSource::Overlay;
        }
        if patch.datatype.is_some() {
            out.datatype = patch.datatype.clone();
            out.sources.datatype = HintSource::Overlay;
        }
        out
    } else {
        base.clone()
    };
    out.plural |= patch.plural;
    out
}
