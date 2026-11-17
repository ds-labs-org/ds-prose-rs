//! The loaded profiles and language preferences one reading uses, and how a
//! policy's compact terms expand.
use super::{HintSource, OperandHints, Overlay, ProfileVocab, Term, TermKind, ns};
use crate::shapes::ShapesGraph;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Bound;
use std::sync::{Arc, OnceLock};

/// What the prose is worded with: the loaded profiles (first loaded wins on
/// a term two of them declare) and the preferred label languages.
/// `Wording::default()` is the built-in wording, no profile.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wording {
    profiles: Vec<Arc<ProfileVocab>>,
    languages: Vec<String>,
    conflicts: Vec<String>,
    overlays: Vec<Arc<Overlay>>,
    shapes: Option<Arc<ShapesGraph>>,
    /// Left operand IRI -> its profile hints with the overlays, then the
    /// shapes, laid over them, for each left operand one of them gives
    /// something for. Rebuilt whenever a profile, an overlay or the shapes
    /// are added, so their order does not matter.
    shaped: HashMap<String, OperandHints>,
}

impl Wording {
    pub fn new() -> Wording {
        Wording::default()
    }

    /// Adds a profile after those already loaded. A term an earlier profile
    /// already declares keeps the earlier definition, and is reported in
    /// [`Wording::conflicts`].
    pub fn with_profile(mut self, p: ProfileVocab) -> Wording {
        for t in p.terms() {
            if let Some((_, first)) = self.lookup(&t.iri) {
                let msg = format!(
                    "\"{}\" is declared by both {} and {}; the first loaded is used",
                    t.iri,
                    name(first),
                    name(&p)
                );
                self.conflicts.push(msg);
            }
        }
        self.profiles.push(Arc::new(p));
        self.layer_hints();
        self
    }

    /// Adds the companion SHACL shapes (see [`crate::shapes`]): they add
    /// hints to the left operands the loaded profiles declare, and may close
    /// the lists of actions and left operands for the whole policy. They
    /// never add or relabel a term. Replaces shapes given earlier; the order
    /// of this call and [`Wording::with_profile`] does not matter.
    pub fn with_shapes(mut self, shapes: ShapesGraph) -> Wording {
        self.shapes = Some(Arc::new(shapes));
        self.layer_hints();
        self
    }

    /// Adds an operator overlay (see [`Overlay`]): its `prose:` hints merge
    /// onto the left operands the loaded profiles declare, and never add or
    /// relabel a term. An overlay is not a second declaration, so it is no
    /// "declared by both" conflict. Overlays are laid over the profiles in
    /// the order they are added (a later one's facet replaces an earlier
    /// one's), and the shapes over those: profile < overlay < shapes. The
    /// order of this call and [`Wording::with_profile`] does not matter.
    pub fn with_overlay(mut self, overlay: Overlay) -> Wording {
        self.overlays.push(Arc::new(overlay));
        self.layer_hints();
        self
    }

    /// The overlays, in the order they were added.
    pub fn overlays(&self) -> impl Iterator<Item = &Overlay> {
        self.overlays.iter().map(|o| &**o)
    }

    /// What `overlay` says that this wording cannot use: its own warnings,
    /// then a line for each term it names that no loaded profile declares as
    /// a left operand (ignored). Lower-case fragments, each once.
    pub fn overlay_warnings(&self, overlay: &Overlay) -> Vec<String> {
        let mut out: Vec<String> = overlay.warnings().to_vec();
        for (iri, _) in overlay.terms() {
            let msg = match self.lookup(iri) {
                None => format!(
                    "ignored <{iri}>: no loaded profile declares it, and an overlay never adds a term"
                ),
                Some((t, _)) if !t.kinds.contains(&TermKind::LeftOperand) => format!(
                    "ignored <{iri}>: it is not a left operand, and an overlay only gives hints to left operands"
                ),
                Some(_) => continue,
            };
            if !out.contains(&msg) {
                out.push(msg);
            }
        }
        out
    }

    /// The companion shapes, when some were given.
    pub fn shapes(&self) -> Option<&ShapesGraph> {
        self.shapes.as_deref()
    }

    /// The hints for a left operand `t` (a term this wording looked up):
    /// the loaded profile's own `prose:` hints with the companion shapes
    /// laid over them. A facet a shape gives replaces the profile's for that
    /// facet, and a left operand a shape says anything about is closed, with
    /// [`OperandHints::sources`] naming who closed each facet. Without
    /// shapes, or for a left operand none of them selects, these are
    /// `t.hints`.
    pub fn operand_hints<'a>(&'a self, t: &'a Term) -> &'a OperandHints {
        self.shaped.get(&t.iri).unwrap_or(&t.hints)
    }

    /// The terms of `kind` the shapes close the policy to (`sh:in` on
    /// `odrl:action` or `odrl:leftOperand`), as expanded IRIs; `None` when
    /// no shape closes that list.
    pub fn closed_terms(&self, kind: TermKind) -> Option<&[String]> {
        let shapes = self.shapes.as_deref()?;
        match kind {
            TermKind::Action => shapes.closed_actions(),
            TermKind::LeftOperand => shapes.closed_left_operands(),
            _ => None,
        }
    }

    /// Recomputes `shaped` from the profiles, the overlays and the shapes.
    fn layer_hints(&mut self) {
        self.shaped.clear();
        let mut shaped: HashMap<String, OperandHints> = HashMap::new();
        for overlay in self.overlays.clone() {
            for (iri, patch) in overlay.terms() {
                let Some((term, _)) = self.lookup(iri) else {
                    continue;
                };
                if !term.kinds.contains(&TermKind::LeftOperand) {
                    continue;
                }
                let base = shaped.get(&term.iri).unwrap_or(&term.hints);
                let layered = super::layer_overlay(base, patch);
                shaped.insert(term.iri.clone(), layered);
            }
        }
        if let Some(shapes) = self.shapes.clone() {
            self.layer_shapes(&shapes, &mut shaped);
        }
        shaped.retain(|iri, h| self.lookup(iri).is_none_or(|(t, _)| *h != t.hints));
        self.shaped = shaped;
    }

    /// Lays the shapes over `shaped` (the profiles' hints with the overlays
    /// over them).
    fn layer_shapes(&self, shapes: &ShapesGraph, shaped: &mut HashMap<String, OperandHints>) {
        let narrower = narrower_values(self);
        for (left, shape) in shapes.constraints() {
            let Some((term, _)) = self.lookup(left) else {
                continue;
            };
            if !term.kinds.contains(&TermKind::LeftOperand) {
                continue;
            }
            let mut patch = OperandHints {
                operators: shape.operators.clone(),
                units: shape.units.clone(),
                datatype: shape.datatype.clone(),
                min_inclusive: shape.min_inclusive.clone(),
                max_inclusive: shape.max_inclusive.clone(),
                ..OperandHints::default()
            };
            if let Some((src, flags)) = &shape.pattern {
                patch.pattern = Some(src.clone());
                patch.pattern_flags = flags.clone();
            }
            if !shape.values.is_empty() || !shape.literals.is_empty() {
                // `sh:in` is exactly its list: no descendants.
                patch.literal_values = shape.literals.clone();
                patch.values = shape.values.clone();
                patch.value_tree = shape.values.iter().map(|v| (v.clone(), 0)).collect();
            } else if !shape.classes.is_empty() {
                let tree = class_members(self, &narrower, &shape.classes);
                patch.values = tree
                    .iter()
                    .filter(|(_, d)| *d == 0)
                    .map(|(v, _)| v.clone())
                    .collect();
                patch.value_tree = tree;
            }
            let base = shaped.get(&term.iri).unwrap_or(&term.hints);
            let layered = super::layer(base, &patch, HintSource::Shapes);
            shaped.insert(term.iri.clone(), layered);
        }
    }

    /// The label languages to prefer, most preferred first.
    pub fn with_languages(mut self, langs: Vec<String>) -> Wording {
        self.languages = langs;
        self
    }

    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    /// The loaded profiles, in load order.
    pub fn profiles(&self) -> impl Iterator<Item = &ProfileVocab> {
        self.profiles.iter().map(|p| &**p)
    }

    /// The term with this expanded IRI, and the profile that declares it.
    pub fn lookup(&self, iri: &str) -> Option<(&Term, &ProfileVocab)> {
        self.profiles
            .iter()
            .find_map(|p| p.term(iri).map(|t| (t, &**p)))
    }

    /// [`Wording::lookup`] for a term as a policy writes it.
    pub fn lookup_written(&self, p: &Prefixes, written: &str) -> Option<(&Term, &ProfileVocab)> {
        self.lookup(&self.expand(p, written))
    }

    /// `written` as an IRI: a full IRI as it is; a compact one with the
    /// policy's prefixes, then the loaded profiles' own prefixes in load
    /// order; a bare word or `odrl:` term in the ODRL namespace unless the
    /// policy binds it otherwise. An unbound prefix is kept as written.
    pub fn expand(&self, p: &Prefixes, written: &str) -> String {
        if let Some(iri) = p.try_expand(written) {
            return iri;
        }
        if let Some((prefix, suffix)) = written.split_once(':') {
            for vocab in self.profiles() {
                if let Some(iri) = vocab.prefixes.get(prefix) {
                    return format!("{iri}{suffix}");
                }
            }
        }
        written.to_string()
    }

    /// Whether the profile `profile_iri` is loaded. A trailing `#` or `/` is
    /// ignored; ODRL's core profile is always covered.
    pub fn covers(&self, profile_iri: &str) -> bool {
        let want = trim(profile_iri);
        if want == trim(ns::ODRL) || want == format!("{}core", ns::ODRL) {
            return true;
        }
        self.profiles().any(|p| p.iri().map(trim) == Some(want))
    }

    /// One message for every term two loaded profiles both declare.
    pub fn conflicts(&self) -> &[String] {
        &self.conflicts
    }

    /// Whether no profile is loaded: the wording is the built-in one.
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

/// Broader value -> its narrower right operands across the loaded profiles
/// (the first profile that declares a term wins), in load order.
fn narrower_values(w: &Wording) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for p in w.profiles() {
        for t in p.terms() {
            if !seen.insert(&t.iri) || !t.kinds.contains(&TermKind::RightOperand) {
                continue;
            }
            for b in &t.broader {
                out.entry(b.clone()).or_default().push(t.iri.clone());
            }
        }
    }
    out
}

/// The right operands that belong to any of `classes` (`sh:class`): those
/// the profile types with the class, the class itself when it is a right
/// operand, and the right operands directly narrower than it; each with the
/// descendants a `skos:broader` link leads to, depth first, each once, with
/// its depth below the member that holds it. At most `max_values_per_key`
/// values, and no deeper than `max_depth`.
fn class_members(
    w: &Wording,
    narrower: &HashMap<String, Vec<String>>,
    classes: &[String],
) -> Vec<(String, usize)> {
    let limits = super::ProfileLimits::default();
    let mut members: Vec<&str> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for p in w.profiles() {
        for t in p.terms() {
            if !seen.insert(&t.iri) || !t.kinds.contains(&TermKind::RightOperand) {
                continue;
            }
            let of_class = classes
                .iter()
                .any(|c| *c == t.iri || t.types.contains(c) || t.broader.contains(c));
            if of_class {
                members.push(&t.iri);
            }
        }
    }
    // A member narrower than another member is listed under it.
    let member_set: HashSet<&str> = members.iter().copied().collect();
    let by_iri = |iri: &str| w.lookup(iri).map(|(t, _)| t);
    let roots: Vec<&str> = members
        .iter()
        .copied()
        .filter(|m| {
            !by_iri(m).is_some_and(|t| t.broader.iter().any(|b| member_set.contains(b.as_str())))
        })
        .collect();
    let mut out: Vec<(String, usize)> = Vec::new();
    let mut done: HashSet<&str> = HashSet::new();
    let kids: HashMap<&str, Vec<&str>> = narrower
        .iter()
        .map(|(k, v)| (k.as_str(), v.iter().map(String::as_str).collect()))
        .collect();
    // An explicit stack, so a deep hierarchy cannot overflow the call stack:
    // (index into `child_lists`, the next item of that list, its depth).
    let mut child_lists: Vec<Vec<&str>> = vec![roots];
    let mut frames: Vec<(usize, usize, usize)> = vec![(0, 0, 0)];
    while let Some(&(list, next, depth)) = frames.last() {
        let Some(&item) = child_lists[list].get(next) else {
            frames.pop();
            continue;
        };
        frames.last_mut().unwrap().1 += 1;
        if !done.insert(item) {
            continue;
        }
        if out.len() >= limits.max_values_per_key {
            break;
        }
        out.push((item.to_string(), depth));
        if depth + 1 < limits.max_depth
            && let Some(k) = kids.get(item)
        {
            child_lists.push(k.clone());
            frames.push((child_lists.len() - 1, 0, depth + 1));
        }
    }
    out
}

/// A local name a compact IRI may end with: not empty, no `/`, `#`, `:` or
/// whitespace.
fn is_local_name(s: &str) -> bool {
    !s.is_empty() && !s.contains(['/', '#', ':']) && !s.contains(char::is_whitespace)
}

fn trim(iri: &str) -> &str {
    iri.trim_end_matches(['#', '/'])
}

fn name(p: &ProfileVocab) -> String {
    match p.iri() {
        Some(i) => format!("<{i}>"),
        None => "an unnamed profile".to_string(),
    }
}

/// The prefixes and term definitions a policy document's `@context` binds.
/// Only inline contexts are read; the W3C ODRL context, named as a remote
/// context, adds the prefixes it is known to bind.
///
/// Equality compares the bindings in effect.
#[derive(Clone, Debug, Default)]
pub struct Prefixes {
    /// Prefix or term -> IRI. A map, so a large `@context` costs its size.
    entries: BTreeMap<String, String>,
    /// The scope these bindings sit in (a node's own `@context` inside its
    /// document's): a key not bound here is looked up there. Shared, not
    /// copied, so many nodes under one large `@context` stay cheap.
    outer: Option<Arc<Prefixes>>,
    /// Namespace -> the greatest key without a `:` bound to it, for
    /// [`Prefixes::compact`]; built on first use, from `entries` alone.
    by_ns: OnceLock<BTreeMap<String, String>>,
}

impl PartialEq for Prefixes {
    fn eq(&self, other: &Prefixes) -> bool {
        match (&self.outer, &other.outer) {
            (None, None) => self.entries == other.entries,
            _ => self.bindings() == other.bindings(),
        }
    }
}

impl Prefixes {
    /// The contexts of `v`, of its `@graph` items, or of each node of a
    /// top-level array, in document order; the first binding of a key wins.
    /// Within one `@context` the later entry wins, as in JSON-LD.
    pub fn of_document(v: &Value) -> Prefixes {
        let mut nodes: Vec<&Value> = Vec::new();
        let tops: Vec<&Value> = match v {
            Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for top in tops {
            nodes.push(top);
            if let Some(g) = top.get("@graph") {
                match g {
                    Value::Array(a) => nodes.extend(a),
                    other => nodes.push(other),
                }
            }
        }
        let mut out = Prefixes::default();
        let mut budget = Budget::default();
        for n in nodes {
            if let Some(c) = n.get("@context") {
                out.merge(Prefixes::of_context(c, &mut budget));
            }
        }
        out
    }

    /// `term` as an IRI; see [`Wording::expand`]. Without a profile, an
    /// unbound prefix is kept as written.
    pub fn expand(&self, term: &str) -> String {
        self.try_expand(term).unwrap_or_else(|| term.to_string())
    }

    /// `iri` as a policy with these prefixes would write it: an ODRL term
    /// bare, any other as a compact IRI with a prefix bound here, never with
    /// an invented prefix. `None` when no bound prefix fits.
    pub fn compact(&self, iri: &str) -> Option<String> {
        if self.outer.is_some() {
            return self.flattened().compact(iri);
        }
        let fits = |written: String| {
            (self.try_expand(&written).as_deref() == Some(iri)).then_some(written)
        };
        if let Some(local) = iri.strip_prefix(ns::ODRL)
            && is_local_name(local)
            && let Some(bare) = fits(local.to_string())
        {
            return Some(bare);
        }
        // The longest bound namespace that leaves a plain local name. A
        // shorter one leaves a longer name that holds this one, so only the
        // longest namespace shorter than the IRI is looked at.
        let by_ns = self.by_ns.get_or_init(|| {
            let mut m = BTreeMap::new();
            // In key order, so the greatest key bound to a namespace wins.
            for (k, v) in self.entries.iter().filter(|(k, _)| !k.contains(':')) {
                m.insert(v.clone(), k.clone());
            }
            m
        });
        let last = iri.chars().next_back()?;
        let (ns, prefix) = longest_prefix(by_ns, iri, &iri[..iri.len() - last.len_utf8()])?;
        let local = &iri[ns.len()..];
        if !is_local_name(local) {
            return None;
        }
        fits(format!("{prefix}:{local}"))
    }

    /// `None` for a compact IRI whose prefix nothing here binds.
    pub(crate) fn try_expand(&self, term: &str) -> Option<String> {
        match term.split_once(':') {
            Some((prefix, suffix)) => {
                if prefix == "_" || suffix.starts_with("//") {
                    return Some(term.to_string());
                }
                if let Some(iri) = self.get(prefix) {
                    return Some(format!("{iri}{suffix}"));
                }
                (prefix == "odrl").then(|| ns::odrl(suffix))
            }
            None => Some(
                self.get(term)
                    .map_or_else(|| ns::odrl(term), str::to_string),
            ),
        }
    }

    /// What `key` is bound to here, else in the enclosing scopes.
    fn get(&self, key: &str) -> Option<&str> {
        let mut layer = self;
        loop {
            if let Some(v) = layer.entries.get(key) {
                return Some(v);
            }
            layer = layer.outer.as_deref()?;
        }
    }

    /// Every binding in effect: an inner one hides an outer one of the same
    /// key.
    fn bindings(&self) -> BTreeMap<&str, &str> {
        let mut out = BTreeMap::new();
        let mut layer = Some(self);
        while let Some(l) = layer {
            for (k, v) in &l.entries {
                out.entry(k.as_str()).or_insert(v.as_str());
            }
            layer = l.outer.as_deref();
        }
        out
    }

    /// The bindings in effect, in one layer.
    fn flattened(&self) -> Prefixes {
        Prefixes {
            entries: self
                .bindings()
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            ..Prefixes::default()
        }
    }

    /// These bindings, inside the scope `outer`: a key not bound here is
    /// looked up there.
    pub(crate) fn inside(self, outer: Arc<Prefixes>) -> Prefixes {
        match self.outer {
            None => Prefixes {
                entries: self.entries,
                outer: Some(outer),
                ..Prefixes::default()
            },
            // Not built by the crate; kept correct all the same.
            Some(_) => Prefixes {
                entries: self.flattened().entries,
                outer: Some(outer),
                ..Prefixes::default()
            },
        }
    }

    /// Adds `other`'s keys that are not bound here yet.
    pub(crate) fn merge(&mut self, other: Prefixes) {
        let other = match other.outer {
            None => other.entries,
            Some(_) => other.flattened().entries,
        };
        for (k, v) in other {
            if self.get(&k).is_none() {
                self.entries.insert(k, v);
            }
        }
        self.by_ns = OnceLock::new();
    }

    fn set(&mut self, key: &str, iri: String) {
        self.entries.insert(key.to_string(), iri);
    }

    /// One `@context` value: a string, an object, or an array of them.
    /// Compact values are expanded only while `budget` lasts; one budget is
    /// shared by the contexts of a document.
    pub(crate) fn of_context(c: &Value, budget: &mut Budget) -> Prefixes {
        let mut out = Prefixes::default();
        let list: Vec<&Value> = match c {
            Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for c in list {
            match c {
                Value::String(s) if ns::is_odrl_context(s) => {
                    for (p, iri) in ns::ODRL_CONTEXT {
                        out.set(p, iri.to_string());
                    }
                }
                Value::Object(o) => {
                    for (k, v) in o.iter().filter(|(k, _)| !k.starts_with('@')) {
                        let iri = match v {
                            Value::String(s) => Some(s.as_str()),
                            Value::Object(d) => d.get("@id").and_then(Value::as_str),
                            _ => None,
                        };
                        match iri {
                            Some(i) if !i.starts_with('@') => out.set(k, i.to_string()),
                            _ => {
                                out.entries.remove(k);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        budget.grant(
            out.entries
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>(),
        );
        // Values written as compact IRIs ("fineTune": "ex:fineTune"); a few
        // passes resolve chains. Each pass reads the values of the one
        // before. A value the budget cannot pay for stays as written.
        for _ in 0..4 {
            let mut changed: Vec<(String, String)> = Vec::new();
            for (k, v) in &out.entries {
                if let Some((prefix, suffix)) = v.split_once(':')
                    && prefix != k
                    && !suffix.starts_with("//")
                    && let Some(iri) = out.get(prefix)
                    && budget.spend(iri.len() + suffix.len())
                {
                    changed.push((k.clone(), format!("{iri}{suffix}")));
                }
            }
            if changed.is_empty() {
                break;
            }
            out.entries.extend(changed);
        }
        out
    }
}

/// The longest key of `m` that is a prefix of `s` and no longer than
/// `upto` (itself a prefix of `s`), with its value. Each step either finds
/// it or shortens `upto` past where the nearest key differs from `s`, so a
/// lookup costs a few range queries, not a scan of `m`.
fn longest_prefix<'m>(
    m: &'m BTreeMap<String, String>,
    s: &str,
    upto: &str,
) -> Option<(&'m str, &'m str)> {
    let mut upto = upto;
    loop {
        let (k, v) = m
            .range::<str, _>((Bound::Unbounded, Bound::Included(upto)))
            .next_back()?;
        if s.starts_with(k.as_str()) {
            return Some((k, v));
        }
        // `k` sorts before `upto` and differs from `s` within `upto`: every
        // key still in the running is a prefix of `s` shorter than that.
        let mut common = k.bytes().zip(s.bytes()).take_while(|(a, b)| a == b).count();
        while !s.is_char_boundary(common) {
            common -= 1;
        }
        upto = &s[..common];
    }
}

/// What expanding a policy's compact `@context` values may add: 64 KiB,
/// plus 16 times the bytes of every context read. Many short terms bound
/// to one long namespace (`"k1": "big:"`) would otherwise each hold a copy
/// of it, gigabytes from a small text.
#[derive(Debug)]
pub(crate) struct Budget {
    left: usize,
}

impl Default for Budget {
    fn default() -> Budget {
        Budget { left: 64 * 1024 }
    }
}

impl Budget {
    /// Adds what a context of `written` bytes may expand to.
    fn grant(&mut self, written: usize) {
        self.left = self.left.saturating_add(written.saturating_mul(16));
    }

    /// Takes `bytes`, if they are left.
    fn spend(&mut self, bytes: usize) -> bool {
        let ok = bytes <= self.left;
        if ok {
            self.left -= bytes;
        }
        ok
    }
}
