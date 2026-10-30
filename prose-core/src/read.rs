//! JSON-LD in, [`Document`] out.
use crate::ProseError;
use crate::model::*;
use crate::profile::{Budget, Prefixes, TermKind, Wording};
use crate::words::{ValueShape, duration, humanise, is_duration_type, join, local_name};
use crate::words_ctx::{WordCtx, profile_name};
use serde_json::{Map, Value};
use std::sync::Arc;

pub(crate) type Obj = Map<String, Value>;

pub(crate) const POLICY_KEYS: &[&str] = &[
    "@context",
    "@id",
    "@type",
    "uid",
    "id",
    "type",
    "profile",
    "conflict",
    "inheritFrom",
    "assigner",
    "assignee",
    "target",
    "action",
    "permission",
    "prohibition",
    "obligation",
];
pub(crate) const RULE_KEYS: &[&str] = &[
    "@id",
    "@type",
    "uid",
    "id",
    "type",
    "action",
    "target",
    "assigner",
    "assignee",
    "constraint",
    "refinement",
    "duty",
    "consequence",
    "remedy",
    "relation",
    "function",
    "failure",
    "output",
    "inheritFrom",
    "profile",
];
/// Metadata vocabularies whose properties are descriptive, not normative.
pub(crate) const QUIET_PREFIXES: &[&str] =
    &["dc:", "dct:", "dcterms:", "rdfs:", "skos:", "schema:"];

/// The ODRL namespace.
pub(crate) const ODRL_NS: &str = "http://www.w3.org/ns/odrl/2/";
/// The ODRL properties the reader looks up by their bare name, with
/// [`PARTY_FUNCTIONS`].
const ODRL_PROPERTIES: &[&str] = &[
    "uid",
    "profile",
    "conflict",
    "inheritFrom",
    "assigner",
    "assignee",
    "target",
    "action",
    "permission",
    "prohibition",
    "obligation",
    "constraint",
    "refinement",
    "duty",
    "consequence",
    "remedy",
    "relation",
    "function",
    "failure",
    "output",
    "leftOperand",
    "operator",
    "rightOperand",
    "rightOperandReference",
    "unit",
    "and",
    "or",
    "xone",
    "andSequence",
    "source",
    "partOf",
];

/// The party functions of the ODRL Common Vocabulary (Vocabulary 4.3), in
/// the order a rule's sentence names them.
const PARTY_FUNCTIONS: &[&str] = &[
    "attributedParty",
    "attributingParty",
    "compensatedParty",
    "compensatingParty",
    "consentingParty",
    "consentedParty",
    "contractingParty",
    "contractedParty",
    "informedParty",
    "informingParty",
    "trackingParty",
    "trackedParty",
];

/// How a party function reads with the one action of its rule.
enum Phrase {
    /// The parties are the verb's object, and the target follows `prep`:
    /// "compensate X for T".
    Object(&'static str),
    /// The parties follow the target, after `prep`; `untargeted` stands in
    /// for a target the rule does not name: "attribute the asset to X".
    After(&'static str, &'static str),
}

/// The phrasing of `function` with the ODRL action `action` (a local
/// name); `None` reads it as "(function: X)".
fn phrase(function: &str, action: &str) -> Option<Phrase> {
    Some(match (function, action) {
        ("attributedParty", "attribute") => Phrase::After("to", " the asset"),
        ("compensatedParty", "compensate") => Phrase::Object("for"),
        ("informedParty", "inform") => Phrase::Object("about"),
        ("consentingParty", "obtainConsent") => Phrase::After("from", ""),
        ("trackingParty", "acceptTracking") => Phrase::After("by", ""),
        _ => return None,
    })
}

/// The local name of an ODRL term written bare, as `odrl:local` or as a
/// full IRI; `None` for a term of another vocabulary.
fn odrl_local(term: &str) -> Option<&str> {
    term.strip_prefix(ODRL_NS)
        .or_else(|| term.strip_prefix("odrl:"))
        .or_else(|| (!term.contains(':')).then_some(term))
}

/// The policy metadata the reader reads, `timestamp` (the Dataspace
/// Protocol's, on an agreement) and `summary`, if `key` spells one: bare,
/// `odrl:` or `dspace:` prefixed, or as an ODRL or Dataspace Protocol IRI.
fn metadata_term(key: &str) -> Option<&'static str> {
    let local = key
        .strip_prefix(ODRL_NS)
        .or_else(|| key.strip_prefix("odrl:"))
        .or_else(|| key.strip_prefix("dspace:"))
        .or_else(|| {
            key.strip_prefix("https://w3id.org/dspace/")
                .and_then(|k| k.rsplit_once('/'))
                .map(|(_, local)| local)
        })
        .unwrap_or(key);
    ["timestamp", "summary"].into_iter().find(|t| *t == local)
}

/// The keys of policy `o` that spell its metadata, with the term each
/// spells: timestamps first, then summaries, each in key order.
pub(crate) fn metadata_keys(o: &Obj) -> Vec<(&'static str, &String)> {
    let mut metadata: Vec<(&'static str, &String)> = o
        .keys()
        .filter_map(|k| metadata_term(k).map(|t| (t, k)))
        .collect();
    metadata.sort_by_key(|(t, k)| (*t != "timestamp", *k));
    metadata
}

/// The note one value of the metadata `term` reads as: "Agreed on ..." (on
/// an `agreement`) or "Issued on ..." for a timestamp, "Summary (en): ..."
/// for a summary. `None` when the value is not a string, a number or a literal, or is
/// blank.
pub(crate) fn metadata_note(term: &str, agreement: bool, v: &Value) -> Option<String> {
    let (text, language) = match v {
        Value::String(s) => (s.clone(), None),
        Value::Number(n) if term == "timestamp" => (n.to_string(), None),
        Value::Object(o) => (
            o.get("@value")?.as_str()?.to_string(),
            o.get("@language").and_then(Value::as_str),
        ),
        _ => return None,
    };
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(match term {
        "timestamp" => match agreement {
            true => format!("Agreed {}.", when(text)),
            false => format!("Issued {}.", when(text)),
        },
        _ => {
            let label = match language {
                Some(l) => format!("Summary ({l})"),
                None => "Summary".to_string(),
            };
            let stop = match text.ends_with(['.', '!', '?']) {
                true => "",
                false => ".",
            };
            format!("{label}: {text}{stop}")
        }
    })
}

/// When, as a note says it: "on 2026-10-15 at 10:00 UTC" for an
/// `xsd:dateTime` or `xsd:date`; any other text "at" it, as written.
fn when(s: &str) -> String {
    let (date, time) = s.split_once('T').unwrap_or((s, ""));
    let digits =
        |b: &[u8], at: &[usize]| at.iter().all(|i| b.get(*i).is_some_and(u8::is_ascii_digit));
    let d = date.as_bytes();
    if d.len() != 10 || d[4] != b'-' || d[7] != b'-' || !digits(d, &[0, 1, 2, 3, 5, 6, 8, 9]) {
        return format!("at {}", fmt_str(s));
    }
    if time.is_empty() {
        return format!("on {date}");
    }
    let (clock, zone) = match time.strip_suffix('Z') {
        Some(clock) => (clock, " UTC".to_string()),
        None => match time
            .len()
            .checked_sub(6)
            .filter(|i| time.is_char_boundary(*i))
        {
            Some(i) if time[i..].starts_with(['+', '-']) => {
                (&time[..i], format!(" UTC{}", &time[i..]))
            }
            _ => (time, String::new()),
        },
    };
    let c = clock.as_bytes();
    let hh_mm = c.len() >= 5 && c[2] == b':' && digits(c, &[0, 1, 3, 4]);
    let seconds = c.len() == 5
        || (c.len() >= 8
            && c[5] == b':'
            && digits(c, &[6, 7])
            && (c.len() == 8
                || (c[8] == b'.' && c.len() > 9 && c[9..].iter().all(u8::is_ascii_digit))));
    if !hh_mm || !seconds {
        return format!("at {}", fmt_str(s));
    }
    let clock = match clock.len() {
        8 => clock.strip_suffix(":00").unwrap_or(clock),
        _ => clock,
    };
    format!("on {date} at {clock}{zone}")
}

/// The properties that declare a node's label, as IRIs, in the order one
/// is preferred to another on the same node.
const LABEL_IRIS: &[&str] = &[
    "http://www.w3.org/2000/01/rdf-schema#label",
    "http://www.w3.org/2004/02/skos/core#prefLabel",
    "http://purl.org/dc/terms/title",
    "http://schema.org/name",
];

/// The prefixes a label property is usually written with, for a document
/// that uses them without binding them.
const LABEL_PREFIXES: &[(&str, &str)] = &[
    ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
    ("skos", "http://www.w3.org/2004/02/skos/core#"),
    ("dct", "http://purl.org/dc/terms/"),
    ("dcterms", "http://purl.org/dc/terms/"),
    ("schema", "http://schema.org/"),
];

/// The most constraint references one document's reading follows.
const MAX_FOLLOWED: usize = 1000;

/// The most inherited rules one document's reading copies into the
/// policies that inherit them. Each is read again for each heir, so a long
/// chain of inheriting policies would cost the square of its length.
const MAX_INHERITED: usize = 5000;

/// The most terms one policy's glossary lists.
const MAX_GLOSSARY: usize = 30;

/// How deep the reader looks into a value that is already parsed. serde_json
/// stops a parsed document at 128 levels, so nothing parsed from text comes
/// near this; a value built in code can be deeper, and cutting it with a note
/// is better than overflowing the stack (a panic in wasm takes the page).
pub(crate) const MAX_DEPTH: usize = 256;

/// The reading of `value`, worded with `wording`. With no profile loaded it
/// is the v0.1 reading and every glossary is empty.
pub fn document_with(value: &Value, wording: &Wording) -> Result<Reading, ProseError> {
    let mut r = Reader::new(WordCtx::new(wording, Prefixes::default()));
    let mut notes = Vec::new();
    let value = &bare_odrl_keys(value, odrl_prefix_is_odrl(value), &mut notes);
    for note in notes {
        r.warn(note);
    }
    // Without a profile no term is expanded: the v0.1 path stays as it was.
    let scopes = match r.ctx.profiled() {
        true => node_prefixes(value),
        false => Vec::new(),
    };
    let mut doc = Document::default();
    let mut glossary = Vec::new();
    let mut mentions = Vec::new();
    let mut names = Vec::new();
    let nodes = policy_nodes(value);
    r.iris = Prefixes::of_document(value);
    // The wording's languages choose among labels with no profile loaded
    // too.
    r.labels = declared_labels(value, &r.iris, wording.languages());
    r.marking = !holds_marks(value);
    for v in declared_constraints(value) {
        if let Some(id) = ident(v) {
            let id = r.iri(&id);
            r.declared.entry(id).or_insert_with(|| v.clone());
        }
    }
    let objs: Vec<Option<&Obj>> = nodes
        .iter()
        .map(|v| match v {
            Value::Object(o) if looks_like_policy(o) => Some(o),
            _ => None,
        })
        .collect();
    let uids: Vec<Option<String>> = objs
        .iter()
        .map(|o| o.and_then(|o| get(o, &["uid", "@id", "id"]).and_then(ident)))
        .collect();
    let parents: Vec<Vec<String>> = objs
        .iter()
        .map(|o| o.map(parent_ids).unwrap_or_default())
        .collect();
    let (lineages, cut) = lineages(&uids, &parents, |id| r.iri(id));
    if cut {
        r.warn(format!(
            "stopped following inheritFrom after {MAX_ANCESTRY} ancestors"
        ));
    }
    for (i, v) in nodes.into_iter().enumerate() {
        match v {
            Value::Object(o) if looks_like_policy(o) => {
                if let Some(p) = scopes.get(i) {
                    r.ctx.set_prefixes(Prefixes::default().inside(p.clone()));
                }
                let lineage = &lineages[i];
                let ancestors: Vec<(&str, &Obj)> = lineage
                    .ancestors
                    .iter()
                    .filter_map(|&a| Some((uids[a].as_deref().unwrap_or_default(), objs[a]?)))
                    .collect();
                let lent;
                let o = match ancestors.is_empty() {
                    true => o,
                    false => {
                        lent = lend(o, &ancestors);
                        &lent
                    }
                };
                let mut policy = r.policy(o, &ancestors, lineage);
                names.push(r.unmark_policy(&mut policy));
                doc.policies.push(policy);
                r.glossed.clear();
                glossary.push(PolicyGlossary {
                    terms: std::mem::take(&mut r.glossary),
                });
                r.mentioned.clear();
                r.marked.clear();
                mentions.push(std::mem::take(&mut r.mentions));
            }
            // Read where a rule refers to it.
            Value::Object(o) if looks_like_constraint(o) && ident(v).is_some() => {}
            // Read where the policy names it.
            Value::Object(o) if only_labels(o, &r.iris) && ident(v).is_some() => {}
            _ => r.warn("skipped a node that is not an ODRL policy"),
        }
    }
    if doc.policies.is_empty() {
        return Err(ProseError::NoPolicy);
    }
    doc.warnings = r.warnings;
    Ok(Reading {
        document: doc,
        glossary,
        mentions,
        names,
    })
}

/// The marks [`Reader::named`] puts around the text of a mention: open,
/// the mention's index, middle, the text, close. Private-use characters, so
/// a text the reader writes has none of its own.
const MARK_OPEN: char = '\u{E000}';
const MARK_MID: char = '\u{E001}';
const MARK_CLOSE: char = '\u{E002}';

/// Whether a string or key of `v` holds a character [`Reader::named`]
/// marks with: the reader then marks nothing, so no text of the document
/// is read as a mark. Iterative, as [`declared_constraints`].
fn holds_marks(v: &Value) -> bool {
    let marks = |s: &str| s.contains([MARK_OPEN, MARK_MID, MARK_CLOSE]);
    let mut stack = vec![v];
    while let Some(x) = stack.pop() {
        match x {
            Value::String(s) if marks(s) => return true,
            Value::Array(a) => stack.extend(a),
            Value::Object(o) => {
                if o.keys().any(|k| marks(k)) {
                    return true;
                }
                stack.extend(o.values());
            }
            _ => {}
        }
    }
    false
}

/// `s` without the marks of the mentions in it, and where each was: only
/// marks of `(index, text)` among `marked`, which the reader put there, are
/// read as marks; any other character is kept as it is.
fn unmark(s: &mut String, marked: &std::collections::HashSet<(usize, String)>) -> Vec<NameSpan> {
    if !s.contains(MARK_OPEN) {
        return Vec::new();
    }
    let mut out = String::with_capacity(s.len());
    let mut spans = Vec::new();
    let mut rest = s.as_str();
    while let Some(at) = rest.find(MARK_OPEN) {
        out.push_str(&rest[..at]);
        let after = &rest[at + MARK_OPEN.len_utf8()..];
        let mark = after.split_once(MARK_MID).and_then(|(index, tail)| {
            let index: usize = index.parse().ok()?;
            let (text, tail) = tail.split_once(MARK_CLOSE)?;
            marked
                .contains(&(index, text.to_string()))
                .then_some((index, text, tail))
        });
        match mark {
            Some((mention, text, tail)) => {
                let start = out.len();
                out.push_str(text);
                spans.push(NameSpan {
                    start,
                    end: out.len(),
                    mention,
                });
                rest = tail;
            }
            None => {
                out.push(MARK_OPEN);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    *s = out;
    spans
}

/// The label each node of `v` declares for its identifier (expanded with
/// `iris`), unless another identifier declares the same: that label would
/// not tell them apart. The first node that labels an identifier wins.
/// Contexts are not looked into. Iterative, as [`declared_constraints`].
fn declared_labels(
    v: &Value,
    iris: &Prefixes,
    langs: &[String],
) -> std::collections::HashMap<String, String> {
    let mut labels = std::collections::HashMap::new();
    let mut stack = vec![v];
    while let Some(x) = stack.pop() {
        match x {
            Value::Array(a) => stack.extend(a.iter().rev()),
            Value::Object(o) => {
                if let (Some(id), Some(label)) = (
                    get(o, &["uid", "@id", "id"]).and_then(Value::as_str),
                    label_of(o, iris, langs),
                ) {
                    let id = match id.contains(':') {
                        true => iris.expand(id),
                        false => id.to_string(),
                    };
                    labels.entry(id).or_insert(label);
                }
                let kids: Vec<&Value> = o
                    .iter()
                    .filter(|(k, _)| *k != "@context")
                    .map(|(_, x)| x)
                    .collect();
                stack.extend(kids.into_iter().rev());
            }
            _ => {}
        }
    }
    let mut uses: std::collections::HashMap<String, usize> = Default::default();
    for l in labels.values() {
        *uses.entry(l.clone()).or_default() += 1;
    }
    labels.retain(|_, l| uses[l] == 1);
    labels
}

/// Whether `o` does nothing but label its identifier: apart from `@id`
/// (or its aliases), `@type` and `@context`, it has label properties only,
/// at least one. A node with anything more (a rule, a policy it holds) is
/// not read, and is reported as skipped.
fn only_labels(o: &Obj, iris: &Prefixes) -> bool {
    label_of(o, iris, &[]).is_some()
        && o.keys().all(|k| {
            matches!(
                k.as_str(),
                "@id" | "id" | "uid" | "@type" | "type" | "@context"
            ) || label_rank(k, iris).is_some()
        })
}

/// Where `key` stands among [`LABEL_IRIS`], if it is a label property.
fn label_rank(key: &str, iris: &Prefixes) -> Option<usize> {
    let (prefix, local) = key.split_once(':')?;
    let iri = iris.try_expand(key).or_else(|| {
        LABEL_PREFIXES
            .iter()
            .find(|(p, _)| *p == prefix)
            .map(|(_, ns)| format!("{ns}{local}"))
    })?;
    let iri = iri.replacen("https://schema.org/", "http://schema.org/", 1);
    LABEL_IRIS.iter().position(|l| *l == iri)
}

/// The label `o` declares: of its most preferred label property, the value
/// in the first of `langs` that has one (matched as [`LangText::pick`]
/// matches), else untagged, else in English, else the first.
///
/// [`LangText::pick`]: crate::profile::LangText::pick
fn label_of(o: &Obj, iris: &Prefixes, langs: &[String]) -> Option<String> {
    let (_, value) = o
        .iter()
        .filter_map(|(k, v)| Some((label_rank(k, iris)?, v)))
        .min_by_key(|(r, _)| *r)?;
    let values: Vec<(Option<&str>, &str)> = items(value)
        .into_iter()
        .filter_map(|x| match x {
            Value::String(s) => Some((None, s.as_str())),
            Value::Object(o) => Some((
                o.get("@language").and_then(Value::as_str),
                o.get("@value")?.as_str()?,
            )),
            _ => None,
        })
        .filter(|(_, s)| !s.trim().is_empty())
        .collect();
    let mut tagged = crate::profile::LangText::default();
    for (l, v) in &values {
        if let Some(l) = l {
            tagged.push(Some(l), v.to_string());
        }
    }
    tagged
        .preferred(langs)
        .or_else(|| values.iter().find(|(l, _)| l.is_none()).map(|(_, v)| *v))
        .or_else(|| tagged.preferred(&["en".to_string()]))
        .or(values.first().map(|(_, v)| *v))
        .map(|s| s.trim().to_string())
}

/// The policies `o` inherits from, as written.
fn parent_ids(o: &Obj) -> Vec<String> {
    o.get("inheritFrom")
        .map(|p| items(p).into_iter().filter_map(ident).collect())
        .unwrap_or_default()
}

fn inherits_note(parents: &[String]) -> String {
    format!("Inherits the rules of {}.", join(parents))
}

/// Where a policy's inherited rules come from (ODRL IM 2.9).
pub(crate) struct Lineage {
    /// The policies it inherits from that are in the document, by index:
    /// every ancestor once, depth first, never the policy itself.
    pub ancestors: Vec<usize>,
    /// The ancestors it names that are not in the document, as written.
    pub missing: Vec<String>,
    /// Whether every missing ancestor is one of its own parents.
    pub missing_are_parents: bool,
    /// Whether [`MAX_ANCESTRY`] left out some of its ancestors.
    pub cut: bool,
}

/// The most ancestors all of one document's lineages list together. Every
/// policy of an inheritance cycle has all the others as ancestors, so a long
/// one would cost the square of its length.
pub(crate) const MAX_ANCESTRY: usize = 20_000;

/// The lineage of each policy, from `uids[i]` and `parents[i]` (its
/// `inheritFrom`, as written), and whether [`MAX_ANCESTRY`] cut it short.
/// `key` expands an identifier so that a parent written as a compact IRI
/// finds the policy written with the full one.
pub(crate) fn lineages(
    uids: &[Option<String>],
    parents: &[Vec<String>],
    key: impl Fn(&str) -> String,
) -> (Vec<Lineage>, bool) {
    let mut index = std::collections::HashMap::new();
    for (i, uid) in uids.iter().enumerate() {
        if let Some(uid) = uid {
            index.entry(key(uid)).or_insert(i);
        }
    }
    // Each parent, as the policy it names or as written when it is not here.
    let found: Vec<Vec<Result<usize, &String>>> = parents
        .iter()
        .map(|ids| {
            ids.iter()
                .map(|id| index.get(&key(id)).copied().ok_or(id))
                .collect()
        })
        .collect();
    let mut left = MAX_ANCESTRY;
    let mut out = Vec::with_capacity(parents.len());
    for (i, ids) in parents.iter().enumerate() {
        let mut lineage = Lineage {
            ancestors: Vec::new(),
            missing: Vec::new(),
            missing_are_parents: true,
            cut: false,
        };
        let mut seen = std::collections::HashSet::from([i]);
        let mut stack: Vec<&Result<usize, &String>> = found[i].iter().rev().collect();
        while let Some(parent) = stack.pop() {
            match *parent {
                Err(id) => {
                    if !lineage.missing.contains(id) {
                        lineage.missing_are_parents &= ids.contains(id);
                        lineage.missing.push(id.clone());
                    }
                }
                Ok(a) if left > 0 && seen.insert(a) => {
                    left -= 1;
                    lineage.ancestors.push(a);
                    stack.extend(found[a].iter().rev());
                }
                Ok(a) => lineage.cut |= left == 0 && !seen.contains(&a),
            }
        }
        out.push(lineage);
    }
    (out, left == 0)
}

/// The policy-level properties a parent lends its child (ODRL IM 2.9).
const LENT: [&str; 5] = ["assigner", "assignee", "target", "action", "conflict"];

/// `o` with what its `ancestors` lend it (ODRL IM 2.9): each policy-level
/// asset, party, action and conflict strategy it lacks and its ancestors
/// agree on (where they disagree, each keeps its own for its own rules),
/// and every profile any of them is written to, after its own.
fn lend(o: &Obj, ancestors: &[(&str, &Obj)]) -> Obj {
    let mut out = o.clone();
    for k in LENT {
        if o.contains_key(k) {
            continue;
        }
        let mut lent = ancestors.iter().filter_map(|(_, a)| a.get(k));
        if let Some(first) = lent.next()
            && lent.all(|v| v == first)
        {
            out.insert(k.into(), first.clone());
        }
    }
    let own = o.get("profile").map(items).unwrap_or_default();
    let mut profiles: Vec<&Value> = own.clone();
    for (_, a) in ancestors {
        for p in a.get("profile").map(items).unwrap_or_default() {
            if !profiles.contains(&p) {
                profiles.push(p);
            }
        }
    }
    if profiles.len() > own.len() {
        let profiles = profiles.into_iter().cloned().collect();
        out.insert("profile".into(), Value::Array(profiles));
    }
    out
}

/// `v` with ODRL property keys written as compact IRIs (`odrl:permission`)
/// or full IRIs renamed to the bare term, as a JSON-LD processor compacting
/// against a context that declares the `odrl` prefix but not the terms
/// writes them (EDC does).
///
/// Only the document's ODRL structure is walked: the values of ODRL
/// properties, `@graph`, `@list`, `@set` and arrays. Everything else (an
/// extension payload, a literal's `@value`, a `@context`) is copied as it is,
/// to a bounded depth, so the author's own keys are never rewritten.
///
/// A bare key already on the object wins. Its prefixed twin is dropped from
/// the copy and reported in `notes`, on every kind of object (policy, rule,
/// constraint, action, party), so nothing the author wrote vanishes silently.
/// Of two prefixed spellings of one term, the first in document order is read.
fn bare_odrl_keys(v: &Value, odrl_prefix: bool, notes: &mut Vec<String>) -> Value {
    Rewriter {
        odrl_prefix,
        notes,
        cut: false,
    }
    .node(v, 0)
}

struct Rewriter<'a> {
    odrl_prefix: bool,
    notes: &'a mut Vec<String>,
    cut: bool,
}

impl Rewriter<'_> {
    /// The bare ODRL property `key` spells, if it is written as a compact IRI
    /// (when `odrl:` means ODRL here) or as a full IRI.
    fn bare_term(&self, key: &str) -> Option<&'static str> {
        let term = key
            .strip_prefix(ODRL_NS)
            .or_else(|| key.strip_prefix("odrl:").filter(|_| self.odrl_prefix))?;
        ODRL_PROPERTIES
            .iter()
            .chain(PARTY_FUNCTIONS)
            .copied()
            .find(|p| *p == term)
    }

    /// Whether the value under `key` can hold ODRL nodes the reader will read.
    fn holds_odrl(key: &str) -> bool {
        ODRL_PROPERTIES.contains(&key)
            || PARTY_FUNCTIONS.contains(&key)
            || matches!(key, "@graph" | "@list" | "@set")
    }

    fn cut_off(&mut self) -> Value {
        if !self.cut {
            self.cut = true;
            self.notes.push(format!(
                "content nested deeper than {MAX_DEPTH} levels is not read"
            ));
        }
        Value::Null
    }

    fn node(&mut self, v: &Value, depth: usize) -> Value {
        if depth > MAX_DEPTH {
            return self.cut_off();
        }
        match v {
            Value::Array(a) => Value::Array(a.iter().map(|x| self.node(x, depth + 1)).collect()),
            Value::Object(o) => {
                let mut out = Obj::new();
                for (k, x) in o {
                    match self.bare_term(k) {
                        Some(term) if o.contains_key(term) || out.contains_key(term) => {
                            self.notes.push(format!(
                                "ignored property \"{k}\": \"{term}\" on the same object is read instead"
                            ));
                        }
                        Some(term) => {
                            out.insert(term.to_string(), self.node(x, depth + 1));
                        }
                        None if Self::holds_odrl(k) => {
                            out.insert(k.clone(), self.node(x, depth + 1));
                        }
                        None => {
                            out.insert(k.clone(), self.copy(x, depth + 1));
                        }
                    }
                }
                Value::Object(out)
            }
            _ => v.clone(),
        }
    }

    /// `v` as it is, to the bounded depth.
    fn copy(&mut self, v: &Value, depth: usize) -> Value {
        if depth > MAX_DEPTH {
            return self.cut_off();
        }
        match v {
            Value::Array(a) => Value::Array(a.iter().map(|x| self.copy(x, depth + 1)).collect()),
            Value::Object(o) => Value::Object(
                o.iter()
                    .map(|(k, x)| (k.clone(), self.copy(x, depth + 1)))
                    .collect(),
            ),
            _ => v.clone(),
        }
    }
}

/// Whether `odrl:` means ODRL here: no `@context` in the document binds the
/// prefix to another IRI, in a context, in a term definition's own (type- or
/// property-scoped) context, or in a nested node's. There is no JSON-LD
/// processing, so a rebinding anywhere refuses the prefix everywhere and an
/// `odrl:` key is reported, not guessed at. Iterative: a hostile value cannot
/// overflow the stack.
fn odrl_prefix_is_odrl(v: &Value) -> bool {
    fn binds_elsewhere(ctx: &Value) -> bool {
        let mut stack = vec![ctx];
        while let Some(c) = stack.pop() {
            match c {
                Value::Array(a) => stack.extend(a),
                Value::Object(o) => {
                    if let Some(b) = o.get("odrl") {
                        let iri = match b {
                            Value::Object(d) => d.get("@id").and_then(Value::as_str),
                            _ => b.as_str(),
                        };
                        if iri != Some(ODRL_NS) {
                            return true;
                        }
                    }
                    for (k, def) in o {
                        if k != "odrl"
                            && let Some(inner) = def.as_object().and_then(|d| d.get("@context"))
                        {
                            stack.push(inner);
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }
    let mut stack = vec![v];
    while let Some(v) = stack.pop() {
        match v {
            Value::Array(a) => stack.extend(a),
            Value::Object(o) => {
                if o.get("@context").is_some_and(binds_elsewhere) {
                    return false;
                }
                stack.extend(o.iter().filter(|(k, _)| *k != "@context").map(|(_, x)| x));
            }
            _ => {}
        }
    }
    true
}

pub(crate) fn policy_nodes(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().flat_map(policy_nodes).collect(),
        Value::Object(o) if o.contains_key("@graph") => policy_nodes(&o["@graph"]),
        _ => vec![v],
    }
}

/// The prefixes of each node [`policy_nodes`] yields, in the same order:
/// the node's own context, then those of the objects whose `@graph` holds
/// it. A sibling's context is not in scope. Scopes are shared, never
/// copied: a node without a context of its own costs one pointer, and one
/// with a context costs that context.
pub(crate) fn node_prefixes(v: &Value) -> Vec<Arc<Prefixes>> {
    fn walk(v: &Value, outer: &Arc<Prefixes>, out: &mut Vec<Arc<Prefixes>>, budget: &mut Budget) {
        let scope = match v.get("@context") {
            Some(c) => Arc::new(Prefixes::of_context(c, budget).inside(outer.clone())),
            None => outer.clone(),
        };
        match v {
            Value::Array(a) => a.iter().for_each(|x| walk(x, outer, out, budget)),
            Value::Object(o) if o.contains_key("@graph") => walk(&o["@graph"], &scope, out, budget),
            _ => out.push(scope),
        }
    }
    let mut out = Vec::new();
    walk(
        v,
        &Arc::new(Prefixes::default()),
        &mut out,
        &mut Budget::default(),
    );
    out
}

pub(crate) fn looks_like_policy(o: &Obj) -> bool {
    types(o).iter().any(|t| {
        matches!(
            local_name(t),
            "Set"
                | "Offer"
                | "Agreement"
                | "Policy"
                | "Privacy"
                | "Ticket"
                | "Assertion"
                | "Request"
        )
    }) || ["permission", "prohibition", "obligation"]
        .iter()
        .any(|k| o.contains_key(*k))
}

/// The constraints `v` declares with an identifier, for rules to refer to
/// by it (ODRL Information Model, Example 19): the top-level and `@graph`
/// nodes first, then any constraint object with an identifier wherever it
/// sits (a node object with `@id` declares that node where it appears), in
/// document order. Contexts are not looked into. Iterative: a hostile value
/// cannot overflow the stack.
fn declared_constraints(v: &Value) -> Vec<&Value> {
    let declares = |x: &Value| match x {
        Value::Object(o) => {
            !looks_like_policy(o)
                && looks_like_constraint(o)
                && bare_reference(o).is_none()
                && ident(x).is_some()
        }
        _ => false,
    };
    let mut out: Vec<&Value> = policy_nodes(v)
        .into_iter()
        .filter(|x| declares(x))
        .collect();
    let mut stack = vec![v];
    while let Some(x) = stack.pop() {
        match x {
            Value::Array(a) => stack.extend(a.iter().rev()),
            Value::Object(o) => {
                if declares(x) {
                    out.push(x);
                }
                let kids: Vec<&Value> = o
                    .iter()
                    .filter(|(k, _)| *k != "@context")
                    .map(|(_, x)| x)
                    .collect();
                stack.extend(kids.into_iter().rev());
            }
            _ => {}
        }
    }
    out
}

/// A constraint declared as a node of its own, for rules to refer to by
/// its identifier (ODRL Information Model, Example 19).
fn looks_like_constraint(o: &Obj) -> bool {
    types(o)
        .iter()
        .any(|t| matches!(local_name(t), "Constraint" | "LogicalConstraint"))
        || ["leftOperand", "and", "or", "xone", "andSequence"]
            .iter()
            .any(|k| o.contains_key(*k))
}

pub(crate) fn types(o: &Obj) -> Vec<&str> {
    ["@type", "type"]
        .iter()
        .filter_map(|k| o.get(*k))
        .flat_map(items)
        .filter_map(Value::as_str)
        .collect()
}

/// Flatten arrays and `@list`/`@set` wrappers into the values they hold.
pub(crate) fn items(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().flat_map(items).collect(),
        Value::Object(o) => match o.get("@list").or_else(|| o.get("@set")) {
            Some(inner) => items(inner),
            None => vec![v],
        },
        _ => vec![v],
    }
}

pub(crate) fn get<'a>(o: &'a Obj, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|k| o.get(*k))
}

/// The identifier of a node object that carries nothing else but `@id`: a
/// rule written as `{"@id": "urn:rule:1"}` is a reference, like the bare
/// string. `uid` and `id` are not: an editor writes a rule whose other
/// properties were all removed as `{"uid": ..}`, and that is still a rule.
pub(crate) fn bare_reference(o: &Obj) -> Option<String> {
    if o.len() != 1 {
        return None;
    }
    o.get("@id").and_then(|x| x.as_str().map(String::from))
}

/// The identifier of a node: a bare string, or `uid`/`@id`/`id`.
pub(crate) fn ident(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => {
            get(o, &["uid", "@id", "id"]).and_then(|x| x.as_str().map(String::from))
        }
        _ => None,
    }
}

struct Reader<'a> {
    warnings: Vec<String>,
    /// The same messages, for de-duplication in constant time.
    seen: std::collections::HashSet<String>,
    ctx: WordCtx<'a>,
    /// The current policy's glossary, and the IRIs already in it.
    glossary: Vec<Gloss>,
    glossed: std::collections::HashSet<String>,
    /// The constraints the document declares as nodes of their own, by
    /// identifier; the ones being read now; how many references were
    /// followed.
    declared: std::collections::HashMap<String, Value>,
    following: Vec<String>,
    followed: usize,
    /// How many inherited rules were copied into their heirs.
    inherited: usize,
    /// How deep the constraint being read is nested, references included.
    nesting: usize,
    /// The document's prefixes, to tell two spellings of one constraint
    /// identifier (`ex:c1`, `http://ex.com/c1`) for the same IRI.
    iris: Prefixes,
    /// The labels the document declares, by IRI ([`declared_labels`]).
    labels: std::collections::HashMap<String, String>,
    /// The IRIs the current policy's text names, and the same IRIs, for
    /// de-duplication.
    mentions: Vec<Mention>,
    mentioned: std::collections::HashMap<String, usize>,
    /// The (mention, text) pairs [`Reader::named`] marked in the current
    /// policy's text, so [`unmark`] reads those marks and no others.
    marked: std::collections::HashSet<(usize, String)>,
    /// Whether [`Reader::named`] marks its text: not when the document
    /// holds a mark character of its own ([`holds_marks`]).
    marking: bool,
}

/// What a rule inherits from its policy when it states none of its own.
#[derive(Default, Clone)]
struct Inherited {
    assigner: Named,
    assignee: Named,
    target: Named,
    actions: Vec<Act>,
}

/// An action as written, as a verb phrase, and its refinements.
#[derive(Clone)]
struct Act {
    term: String,
    text: String,
    refinements: Vec<Condition>,
}

/// Parties or assets as a sentence names them, and the IRIs of the
/// undeclared constraints their refinements refer to.
#[derive(Default, Clone)]
struct Named {
    names: Vec<String>,
    refs: Vec<String>,
    /// Whether a name ends in a refinement ("X where a; b"), which nothing
    /// may follow in the same clause without reading as part of it.
    refined: bool,
}

impl<'a> Reader<'a> {
    fn new(ctx: WordCtx<'a>) -> Reader<'a> {
        Reader {
            warnings: Vec::new(),
            seen: Default::default(),
            ctx,
            glossary: Vec::new(),
            glossed: Default::default(),
            declared: Default::default(),
            following: Vec::new(),
            followed: 0,
            inherited: 0,
            nesting: 0,
            iris: Prefixes::default(),
            labels: Default::default(),
            mentions: Vec::new(),
            mentioned: Default::default(),
            marked: Default::default(),
            marking: false,
        }
    }

    /// Notes one use of `written` as a `kind`: its first use in a policy
    /// goes into the glossary, a deprecated term is warned about, and so is
    /// a term (`what` names it) a loaded profile's namespace does not
    /// declare. Nothing happens while no profile is loaded.
    fn term(&mut self, written: &str, kind: TermKind, what: Option<&str>) {
        if !self.ctx.profiled() || written.is_empty() {
            return;
        }
        if let Some((t, p)) = self.ctx.resolve(written) {
            if t.deprecated {
                self.warn(format!(
                    "\"{written}\" is deprecated in the profile {}",
                    profile_name(p)
                ));
            }
            if self.glossary.len() < MAX_GLOSSARY
                && !self.glossed.contains(&t.iri)
                && let Some(g) = self.ctx.gloss(written, kind)
            {
                self.glossed.insert(t.iri.clone());
                self.glossary.push(g);
            }
        } else if let (Some(what), Some(p)) = (what, self.ctx.undeclared_in(written)) {
            self.warn(format!(
                "the {what} \"{written}\" is not declared by the loaded profile {}",
                profile_name(p)
            ));
        }
    }

    /// An identifier the text names, as it reads: with the label the
    /// document declares for it, else as written. An IRI (or a labelled
    /// identifier) is noted among the policy's mentions, once however it is
    /// written, and its text marked with the mention's index; the policy's
    /// texts lose the marks in [`Reader::unmark_policy`], which notes where
    /// they were.
    fn named(&mut self, written: &str) -> String {
        let iri = self.iri(written);
        let label = self.labels.get(&iri).cloned();
        let text = label.clone().unwrap_or_else(|| written.to_string());
        if label.is_none() && !written.contains(':') {
            return text;
        }
        let index = match self.mentioned.get(&iri) {
            Some(&i) => i,
            None => {
                let i = self.mentions.len();
                self.mentioned.insert(iri.clone(), i);
                self.mentions.push(Mention {
                    written: written.to_string(),
                    iri,
                    label,
                });
                i
            }
        };
        if !self.marking {
            return text;
        }
        self.marked.insert((index, text.clone()));
        format!("{MARK_OPEN}{index}{MARK_MID}{text}{MARK_CLOSE}")
    }

    /// `p`'s texts without the marks of [`Reader::named`], and where they
    /// were.
    fn unmark_policy(&self, p: &mut Policy) -> PolicyNames {
        let marked = &self.marked;
        unmark(&mut p.heading, marked);
        for n in &mut p.notes {
            unmark(n, marked);
        }
        fn condition(
            c: &mut Condition,
            marked: &std::collections::HashSet<(usize, String)>,
        ) -> ConditionNames {
            ConditionNames {
                text: unmark(&mut c.text, marked),
                children: c
                    .children
                    .iter_mut()
                    .map(|c| condition(c, marked))
                    .collect(),
            }
        }
        fn rule(r: &mut Rule, marked: &std::collections::HashSet<(usize, String)>) -> RuleNames {
            RuleNames {
                sentence: unmark(&mut r.sentence, marked),
                refinements: r
                    .refinements
                    .iter_mut()
                    .map(|c| condition(c, marked))
                    .collect(),
                conditions: r
                    .conditions
                    .iter_mut()
                    .map(|c| condition(c, marked))
                    .collect(),
                follow_ups: r
                    .follow_ups
                    .iter_mut()
                    .map(|f| f.rules.iter_mut().map(|r| rule(r, marked)).collect())
                    .collect(),
            }
        }
        PolicyNames {
            intro: unmark(&mut p.intro, marked),
            rules: p.rules.iter_mut().map(|r| rule(r, marked)).collect(),
        }
    }

    fn warn(&mut self, msg: impl Into<String>) {
        let mut msg = msg.into();
        unmark(&mut msg, &self.marked);
        if self.seen.insert(msg.clone()) {
            self.warnings.push(msg);
        }
    }

    /// A declared profile as the note names it: a loaded profile with its
    /// title and version, any other by its IRI.
    fn profile_named(&self, id: &str) -> String {
        let Some(p) = self.ctx.profile(id) else {
            return id.to_string();
        };
        match (self.ctx.title(p), p.version()) {
            (Some(t), Some(v)) => format!("{t} ({id}, version {v})"),
            (Some(t), None) => format!("{t} ({id})"),
            (None, _) => id.to_string(),
        }
    }

    /// Warns of each key of `o` that is not in `known` or `also`, nor in a
    /// metadata vocabulary.
    fn unknown_keys_but(&mut self, o: &Obj, known: &[&str], also: &[String], what: &str) {
        // Sorted, not in document order: `serde_json` keeps document order when
        // its `preserve_order` feature is on (the edit module needs it), and
        // the v0.1 warnings were always alphabetical.
        let mut keys: Vec<&String> = o.keys().collect();
        keys.sort();
        for k in keys {
            if known.contains(&k.as_str())
                || also.contains(k)
                || QUIET_PREFIXES.iter().any(|p| k.starts_with(p))
            {
                continue;
            }
            self.warn(format!("ignored unknown property \"{k}\" on {what}"));
        }
    }

    /// The policy `o`, with the rules of its `ancestors` (as written, and
    /// the node) after its own, each marked with where it comes from. `o`
    /// already carries what they lend it ([`lend`]).
    fn policy(&mut self, o: &Obj, ancestors: &[(&str, &Obj)], lineage: &Lineage) -> Policy {
        let metadata = metadata_keys(o);
        let read: Vec<String> = metadata.iter().map(|(_, k)| (*k).clone()).collect();
        self.unknown_keys_but(o, POLICY_KEYS, &read, "a policy");
        let kind = match types(o)
            .iter()
            .map(|t| local_name(t))
            .find(|t| !matches!(*t, "Policy"))
        {
            Some("Set") => PolicyKind::Set,
            Some("Offer") => PolicyKind::Offer,
            Some("Agreement") => PolicyKind::Agreement,
            Some(other) => PolicyKind::Other(other.to_string()),
            // ODRL IM 2.1.1: a policy with no subclass, typed only as
            // `Policy` or not at all, is a Set.
            None => PolicyKind::Set,
        };
        let uid = get(o, &["uid", "@id", "id"]).and_then(ident);
        let class = types(o).iter().find_map(|t| self.ctx.policy_class(t));
        for t in types(o) {
            self.term(t, TermKind::PolicyClass, None);
        }
        let assigner_named = self.parties(o.get("assigner"));
        let assignee_named = self.parties(o.get("assignee"));
        let (assigner, assignee) = (&assigner_named.names, &assignee_named.names);

        let (label, intro) = match &kind {
            PolicyKind::Set => ("Policy set", "A set of rules".to_string()),
            PolicyKind::Offer => (
                "Offer",
                match assigner.is_empty() {
                    true => "An offer".to_string(),
                    false => format!("An offer made by {}", join(assigner)),
                },
            ),
            PolicyKind::Agreement => ("Agreement", "An agreement".to_string()),
            PolicyKind::Other(t) => ("Policy", format!("A policy of type {t}")),
        };
        let mut intro = intro;
        match (&kind, assigner.is_empty(), assignee.is_empty()) {
            (PolicyKind::Offer, _, false) => {
                intro.push_str(&format!(", addressed to {}", join(assignee)))
            }
            (PolicyKind::Offer, _, true) => {}
            (_, false, false) => intro.push_str(&format!(
                " between {} (assigner) and {} (assignee)",
                join(assigner),
                join(assignee)
            )),
            (_, false, true) => intro.push_str(&format!(" from {} (assigner)", join(assigner))),
            (_, true, false) => intro.push_str(&format!(" for {} (assignee)", join(assignee))),
            (_, true, true) => {}
        }
        intro.push('.');

        let mut notes = Vec::new();
        for (term, key) in metadata {
            for v in items(&o[key]) {
                match metadata_note(term, matches!(kind, PolicyKind::Agreement), v) {
                    Some(note) => notes.push(note),
                    None => self.warn(format!("skipped an unreadable {term}")),
                }
            }
        }
        if let Some(p) = o.get("profile") {
            let ids: Vec<String> = items(p).into_iter().filter_map(ident).collect();
            for id in &ids {
                if !self.ctx.covers(id) {
                    self.warn(format!(
                        "the policy declares the profile <{id}>, which is not loaded"
                    ));
                }
            }
            let ids: Vec<String> = ids.iter().map(|id| self.profile_named(id)).collect();
            if !ids.is_empty() {
                notes.push(format!("Written to the profile {}.", join(&ids)));
            }
        }
        if let Some(c) = o.get("conflict").and_then(ident) {
            self.term(&c, TermKind::ConflictTerm, None);
            notes.push(match local_name(&c) {
                "perm" => "If a permission and a prohibition conflict, the permission wins.".into(),
                "prohibit" => {
                    "If a permission and a prohibition conflict, the prohibition wins.".into()
                }
                "invalid" => {
                    "If a permission and a prohibition conflict, the whole policy is void.".into()
                }
                other => format!("Conflicts are resolved by the strategy {other}."),
            });
        }
        let parents = parent_ids(o);
        if !parents.is_empty() {
            notes.push(inherits_note(&parents));
        }

        let inherited = Inherited {
            assigner: assigner_named.clone(),
            assignee: assignee_named.clone(),
            target: self.assets(o.get("target")),
            actions: self.actions(o.get("action")),
        };
        let mut rules = Vec::new();
        for (key, kind) in [
            ("permission", RuleKind::Permission),
            ("prohibition", RuleKind::Prohibition),
            ("obligation", RuleKind::Obligation),
        ] {
            if let Some(v) = o.get(key) {
                for rule in items(v) {
                    rules.push(self.rule(rule, kind, &inherited));
                }
            }
        }
        // A policy that inherits has the rules of its ancestors (ODRL IM
        // 2.9), each with the policy-level properties of the ancestor it
        // comes from and, where that has none, the child's.
        for (from, a) in ancestors {
            if self.inherited >= MAX_INHERITED {
                self.warn(format!(
                    "stopped copying inherited rules after {MAX_INHERITED}"
                ));
                break;
            }
            let pick = |k: &str| a.get(k).or_else(|| o.get(k));
            let inherited = Inherited {
                assigner: self.parties(pick("assigner")),
                assignee: self.parties(pick("assignee")),
                target: self.assets(pick("target")),
                actions: self.actions(pick("action")),
            };
            for (key, kind) in [
                ("permission", RuleKind::Permission),
                ("prohibition", RuleKind::Prohibition),
                ("obligation", RuleKind::Obligation),
            ] {
                for rule in a.get(key).map(items).unwrap_or_default() {
                    if self.inherited >= MAX_INHERITED {
                        self.warn(format!(
                            "stopped copying inherited rules after {MAX_INHERITED}"
                        ));
                        break;
                    }
                    self.inherited += 1;
                    let mut rule = self.rule(rule, kind, &inherited);
                    rule.sentence = format!("Inherited from {from}: {}", rule.sentence);
                    rules.push(rule);
                }
            }
        }
        if rules.is_empty() && parents.is_empty() {
            self.warn("a policy has no permission, prohibition or obligation");
        }
        // Inheriting from itself, or from policies here without rules, gives
        // a policy none: it warns as one without parents does.
        if rules.is_empty()
            && !parents.is_empty()
            && lineage.missing.is_empty()
            && !lineage.cut
            && self.inherited < MAX_INHERITED
        {
            self.warn("a policy has no permission, prohibition or obligation");
            let old = inherits_note(&parents);
            if let Some(n) = notes.iter_mut().find(|n| **n == old) {
                n.pop();
                n.push_str(", but no rule comes from there.");
            }
        }
        if rules.is_empty() && !lineage.missing.is_empty() {
            let (noun, verb) = match lineage.missing.len() {
                1 => ("policy", "is"),
                _ => ("policies", "are"),
            };
            let kin = match lineage.missing_are_parents {
                true => "parent",
                false => "ancestor",
            };
            let old = inherits_note(&parents);
            if let Some(n) = notes.iter_mut().find(|n| **n == old) {
                *n = format!(
                    "All of its rules come from the {kin} {noun} {}, which {verb} not in this document.",
                    join(&lineage.missing)
                );
            }
        }

        let label = class.as_deref().unwrap_or(label);
        Policy {
            heading: match &uid {
                Some(u) => format!("{label} {u}"),
                None => label.to_string(),
            },
            kind,
            uid,
            intro,
            notes,
            rules,
        }
    }

    fn rule(&mut self, v: &Value, kind: RuleKind, inh: &Inherited) -> Rule {
        let o = match v {
            Value::Object(o) if bare_reference(o).is_none() => o,
            _ => {
                let id = ident(v).unwrap_or_else(|| "(unnamed)".into());
                return Rule {
                    kind,
                    uid: Some(id.clone()),
                    sentence: format!("The rule {id} is referenced here but not defined."),
                    refinements: vec![],
                    conditions: vec![],
                    follow_ups: vec![],
                    references: vec![],
                };
            }
        };
        let functions: Vec<(String, String)> = o
            .keys()
            .filter(|k| !RULE_KEYS.contains(&k.as_str()) && !PARTY_FUNCTIONS.contains(&k.as_str()))
            .filter_map(|k| self.ctx.party_function(k).map(|l| (k.clone(), l)))
            .collect();
        let function_keys: Vec<String> = functions.iter().map(|(k, _)| k.clone()).collect();
        let known: Vec<&str> = RULE_KEYS.iter().chain(PARTY_FUNCTIONS).copied().collect();
        self.unknown_keys_but(o, &known, &function_keys, "a rule");
        let uid = get(o, &["uid", "@id", "id"]).and_then(ident);

        let own_assigner = self.parties(o.get("assigner"));
        let own_assignee = self.parties(o.get("assignee"));
        let own_target = self.assets(o.get("target"));
        let own_actions = self.actions(o.get("action"));
        let own = |own: &'_ Named, inherited: &'_ Named| -> Named {
            match own.names.is_empty() {
                true => inherited.clone(),
                false => own.clone(),
            }
        };
        let assigner_named = own(&own_assigner, &inh.assigner);
        let assignee_named = own(&own_assignee, &inh.assignee);
        let target_named = own(&own_target, &inh.target);
        let mut references = Vec::new();
        for r in [&assigner_named, &assignee_named, &target_named]
            .into_iter()
            .flat_map(|n| &n.refs)
        {
            if !references.contains(r) {
                references.push(r.clone());
            }
        }
        let (assigner, assignee, target) = (
            &assigner_named.names,
            &assignee_named.names,
            &target_named.names,
        );
        let actions = if own_actions.is_empty() {
            &inh.actions
        } else {
            &own_actions
        };

        let action_names: Vec<String> = actions.iter().map(|a| a.text.clone()).collect();
        let mut action = if action_names.is_empty() {
            "perform an unspecified action on".to_string()
        } else {
            join(&action_names)
        };
        let mut target_text = if target.is_empty() {
            match kind {
                RuleKind::Obligation => String::new(),
                _ => " an unspecified asset".to_string(),
            }
        } else {
            format!(" {}", join(target))
        };

        // Party functions: as the verb's object or after the target when
        // the rule's one action has a phrasing for them, else in a
        // parenthesis after the target. A refined object party goes after
        // the target, so its refinement ends the clause instead of running
        // into the target: "compensate, for T, X where a; b".
        let one_action = match actions.as_slice() {
            [a] => odrl_local(&a.term),
            _ => None,
        };
        let mut aside = Vec::new();
        for function in PARTY_FUNCTIONS {
            let Some(v) = o.get(*function) else {
                continue;
            };
            self.term(function, TermKind::PartyFunction, None);
            let parties = self.parties(Some(v));
            for r in parties.refs {
                if !references.contains(&r) {
                    references.push(r);
                }
            }
            if parties.names.is_empty() {
                continue;
            }
            let names = join(&parties.names);
            match one_action.and_then(|a| phrase(function, a)) {
                Some(Phrase::Object(prep)) => {
                    if target_text.is_empty() {
                        action = format!("{action} {names}");
                    } else if parties.refined {
                        target_text = format!(", {prep}{target_text}, {names}");
                    } else {
                        action = format!("{action} {names}");
                        target_text = format!(" {prep}{target_text}");
                    }
                }
                Some(Phrase::After(prep, untargeted)) => {
                    if target_text.is_empty() {
                        target_text = untargeted.to_string();
                    }
                    target_text.push_str(&format!(" {prep} {names}"));
                }
                None => aside.push(format!("{}: {names}", humanise(function))),
            }
        }
        if !aside.is_empty() {
            target_text.push_str(&format!(" ({})", aside.join("; ")));
        }
        let by = |verb: &str| match assigner.len() {
            0 => String::new(),
            1 => format!(", as {verb} by the assigner {}", join(assigner)),
            _ => format!(", as {verb} by the assigners {}", join(assigner)),
        };
        let mut sentence = match kind {
            RuleKind::Permission => {
                let who = if assignee.is_empty() {
                    "Anyone".into()
                } else {
                    join(assignee)
                };
                format!("{who} may {action}{target_text}{}.", by("permitted"))
            }
            RuleKind::Prohibition => match assignee.is_empty() {
                true => format!("No one may {action}{target_text}{}.", by("prohibited")),
                false => format!(
                    "{} must not {action}{target_text}{}.",
                    join(assignee),
                    by("prohibited")
                ),
            },
            RuleKind::Obligation => {
                let who = if assignee.is_empty() {
                    "The assignee".into()
                } else {
                    join(assignee)
                };
                format!("{who} must {action}{target_text}{}.", by("required"))
            }
        };
        for (key, label) in &functions {
            self.term(key, TermKind::PartyFunction, None);
            let parties = self.parties(o.get(key));
            if !parties.names.is_empty() {
                sentence.push_str(&format!(" {label}: {}.", join(&parties.names)));
            }
            for r in parties.refs {
                if !references.contains(&r) {
                    references.push(r);
                }
            }
        }

        let refinements = actions
            .iter()
            .filter(|a| !a.refinements.is_empty())
            .map(|a| Condition {
                text: format!("The action \"{}\" is limited to cases where", a.text),
                children: a.refinements.clone(),
                reference: None,
                urls: Vec::new(),
            })
            .collect();
        let conditions = o
            .get("constraint")
            .map(|c| items(c).into_iter().map(|c| self.constraint(c)).collect())
            .unwrap_or_default();

        // Duties of an obligation or follow-ups of a duty inherit nothing:
        // the parties are re-stated by the author or read as "the assignee".
        let child_inh = Inherited {
            assigner: assigner_named.clone(),
            assignee: assignee_named.clone(),
            ..Default::default()
        };
        let mut follow_ups = Vec::new();
        for (key, label) in [
            (
                "duty",
                match kind {
                    RuleKind::Permission => {
                        "Duties to fulfil before this permission can be exercised"
                    }
                    _ => "Duties attached to this rule",
                },
            ),
            (
                "remedy",
                match kind {
                    RuleKind::Prohibition => "Remedies if this prohibition is breached",
                    _ => "Remedies attached to this rule",
                },
            ),
            (
                "consequence",
                match kind {
                    RuleKind::Obligation => "Consequences if this duty is not fulfilled",
                    _ => "Consequences attached to this rule",
                },
            ),
        ] {
            if let Some(v) = o.get(key) {
                let rules: Vec<Rule> = items(v)
                    .into_iter()
                    .map(|d| self.rule(d, RuleKind::Obligation, &child_inh))
                    .collect();
                if !rules.is_empty() {
                    follow_ups.push(FollowUp {
                        label: label.to_string(),
                        rules,
                    });
                }
            }
        }

        Rule {
            kind,
            uid,
            sentence,
            refinements,
            conditions,
            follow_ups,
            references,
        }
    }

    // --- parties and assets ---------------------------------------------

    fn parties(&mut self, v: Option<&Value>) -> Named {
        self.entities(v, "party")
    }

    fn assets(&mut self, v: Option<&Value>) -> Named {
        self.entities(v, "asset")
    }

    fn entities(&mut self, v: Option<&Value>, noun: &str) -> Named {
        let Some(v) = v else {
            return Named::default();
        };
        let mut refs = Vec::new();
        let mut refined = false;
        let names = items(v)
            .into_iter()
            .filter_map(|e| match e {
                Value::String(s) => Some(self.named(s)),
                Value::Object(o) => {
                    let id = get(o, &["uid", "@id", "id"]).and_then(Value::as_str);
                    let id = id.map(|id| self.named(id));
                    let collection = o.get("source").or_else(|| o.get("partOf")).and_then(ident);
                    let collection = collection.map(|c| self.named(&c));
                    let mut text = match (id, collection) {
                        (Some(id), None) => id,
                        (Some(id), Some(c)) => format!("{id} (a member of {c})"),
                        (None, Some(c)) => format!("any {noun} in the collection {c}"),
                        (None, None) => {
                            self.warn(format!("a {noun} has no identifier"));
                            return None;
                        }
                    };
                    if let Some(r) = o.get("refinement") {
                        let parts: Vec<String> = items(r)
                            .into_iter()
                            .map(|c| {
                                let c = self.constraint(c);
                                references_in(&c, &mut refs);
                                inline(&c)
                            })
                            .collect();
                        text.push_str(&format!(" where {}", parts.join("; ")));
                        refined = true;
                    }
                    Some(text)
                }
                _ => {
                    self.warn(format!("skipped an unreadable {noun}"));
                    None
                }
            })
            .collect();
        Named {
            names,
            refs,
            refined,
        }
    }

    // --- actions ----------------------------------------------------------

    fn actions(&mut self, v: Option<&Value>) -> Vec<Act> {
        let Some(v) = v else { return vec![] };
        items(v)
            .into_iter()
            .filter_map(|a| match a {
                Value::String(s) => {
                    self.term(s, TermKind::Action, Some("action"));
                    Some(Act {
                        term: s.clone(),
                        text: self.ctx.action(s),
                        refinements: vec![],
                    })
                }
                Value::Object(o) => {
                    // Pre-0.7 profiles name the action under `type`.
                    let Some(name) =
                        get(o, &["rdf:value", "value", "@id", "type", "odrl:type"]).and_then(ident)
                    else {
                        self.warn("skipped an action object that names no action");
                        return None;
                    };
                    self.term(&name, TermKind::Action, Some("action"));
                    let refine = o
                        .get("refinement")
                        .map(|r| items(r).into_iter().map(|c| self.constraint(c)).collect())
                        .unwrap_or_default();
                    Some(Act {
                        text: self.ctx.action(&name),
                        term: name,
                        refinements: refine,
                    })
                }
                _ => {
                    self.warn("skipped an unreadable action");
                    None
                }
            })
            .collect()
    }

    // --- constraints ------------------------------------------------------

    /// The reading of one constraint, nested no deeper than [`MAX_DEPTH`]
    /// levels in all: references followed nest it as deep as JSON nesting
    /// does, and a chain of them in a flat `@graph` is not bounded by the
    /// parser's depth limit.
    fn constraint(&mut self, v: &Value) -> Condition {
        if self.nesting >= MAX_DEPTH {
            self.warn(format!(
                "constraints nested deeper than {MAX_DEPTH} levels are not read"
            ));
            return Condition::leaf("a constraint nested too deep to read holds");
        }
        self.nesting += 1;
        let read = self.constraint_here(v);
        self.nesting -= 1;
        read
    }

    fn constraint_here(&mut self, v: &Value) -> Condition {
        let reference = match v {
            Value::String(id) => Some(id.clone()),
            Value::Object(o) => bare_reference(o),
            _ => None,
        };
        if let Some(id) = reference {
            return self.referred(id);
        }
        let Value::Object(o) = v else {
            self.warn("skipped an unreadable constraint");
            return Condition::leaf("an unreadable constraint holds");
        };
        for (key, text) in [
            ("and", "all of the following hold"),
            ("or", "at least one of the following holds"),
            ("xone", "exactly one of the following holds"),
            ("andSequence", "the following hold, in this order"),
        ] {
            if let Some(list) = o.get(key) {
                return Condition {
                    text: text.to_string(),
                    children: items(list)
                        .into_iter()
                        .map(|c| self.constraint(c))
                        .collect(),
                    reference: None,
                    urls: Vec::new(),
                };
            }
        }
        let left = get(o, &["leftOperand"]).and_then(ident);
        let op = get(o, &["operator"]).and_then(ident);
        let (Some(left), Some(op)) = (left, op) else {
            self.warn("a constraint is missing its leftOperand or operator");
            return Condition::leaf("an incomplete constraint holds");
        };
        self.term(&left, TermKind::LeftOperand, Some("left operand"));
        self.term(&op, TermKind::Operator, Some("operator"));
        let right = if let Some(r) = o.get("rightOperand") {
            let conj = match local_name(&op) {
                "isAnyOf" | "isNoneOf" => "or",
                _ => "and",
            };
            let vals: Vec<String> = items(r).into_iter().map(|x| self.literal(x)).collect();
            let mut text = join_with(&vals, conj);
            if let Some(u) = o.get("unit").and_then(ident) {
                self.term(&u, TermKind::RightOperand, None);
                text.push(' ');
                text.push_str(&self.ctx.unit(&u));
            }
            text
        } else if let Some(r) = o.get("rightOperandReference").and_then(ident) {
            format!("the value at {r}")
        } else {
            self.warn("a constraint is missing its rightOperand");
            "an unspecified value".to_string()
        };
        let shape = self.value_shape(o.get("rightOperand"));
        let mut read = Condition::leaf(format!(
            "{} {} {}",
            self.ctx.operand(&left),
            self.ctx.operator_for(&left, &op, shape),
            right
        ));
        if local_name(&left) == "acceptedTerms" {
            read.urls = self.terms_urls(o.get("rightOperand"));
        }
        read
    }

    /// The `http(s)` URLs in the plain strings of an `acceptedTerms` right
    /// operand: a bare string, or an `@value` without a datatype or typed
    /// `xsd:string`. An IRI value names a node, not a document to link.
    fn terms_urls(&self, right: Option<&Value>) -> Vec<String> {
        let mut urls: Vec<String> = Vec::new();
        for v in right.map(items).unwrap_or_default() {
            let s = match v {
                Value::String(s) => s,
                Value::Object(o) => match (o.get("@value"), o.get("@type").and_then(Value::as_str))
                {
                    (Some(Value::String(s)), None) => s,
                    (Some(Value::String(s)), Some(t)) if self.iris.expand(t) == XSD_STRING => s,
                    _ => continue,
                },
                _ => continue,
            };
            for u in http_urls(s) {
                if !urls.iter().any(|x| x == u) {
                    urls.push(u.to_string());
                }
            }
        }
        urls
    }

    /// A constraint referred to by `id`: the one the document declares,
    /// read in its place, else a clause that names where it is defined.
    /// One that refers to itself, or one past the bound on references,
    /// reads by its identifier.
    fn referred(&mut self, id: String) -> Condition {
        let id = self.iri(&id);
        let Some(declared) = self.declared.get(&id) else {
            return Condition {
                text: format!("the constraint defined at {id} holds"),
                children: Vec::new(),
                reference: Some(id),
                urls: Vec::new(),
            };
        };
        if self.following.contains(&id) {
            self.warn(format!("the constraint {id} refers to itself"));
            return Condition::leaf(format!("the constraint {id} holds"));
        }
        if self.followed >= MAX_FOLLOWED {
            self.warn(format!(
                "stopped following constraint references after {MAX_FOLLOWED}"
            ));
            return Condition::leaf(format!("the constraint {id} holds"));
        }
        self.followed += 1;
        let declared = declared.clone();
        self.following.push(id);
        let read = self.constraint(&declared);
        self.following.pop();
        read
    }

    /// A constraint identifier as an IRI: a compact IRI whose prefix the
    /// document binds is expanded; anything else (a full IRI, a URN, a
    /// relative reference) is kept as written.
    fn iri(&self, id: &str) -> String {
        match id.contains(':') {
            true => self.iris.expand(id),
            false => id.to_string(),
        }
    }

    /// Whether a literal's `@type` names xsd:duration.
    fn is_duration(&self, datatype: &str) -> bool {
        is_duration_type(&self.iris.expand(datatype))
    }

    /// The shape of a constraint's right operand: that of its one literal,
    /// and [`ValueShape::Other`] for no value, several, or a non-string one.
    fn value_shape(&self, right: Option<&Value>) -> ValueShape {
        let Some(right) = right else {
            return ValueShape::Other;
        };
        match items(right).as_slice() {
            [Value::String(s)] => ValueShape::of(s, None),
            [Value::Object(o)] => match o.get("@value") {
                Some(Value::String(s)) => {
                    let datatype = o.get("@type").and_then(Value::as_str);
                    ValueShape::of(s, datatype.map(|t| self.iris.expand(t)).as_deref())
                }
                _ => ValueShape::Other,
            },
            _ => ValueShape::Other,
        }
    }

    fn literal(&mut self, v: &Value) -> String {
        match v {
            Value::String(s) => duration(s).unwrap_or_else(|| fmt_str(s)),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Object(o) => {
                if let Some(inner) = o.get("@value") {
                    let datatype = o.get("@type").and_then(Value::as_str);
                    return match inner {
                        Value::String(s) if datatype.is_some_and(|t| self.is_duration(t)) => {
                            duration(s).unwrap_or_else(|| s.clone())
                        }
                        Value::String(s) if datatype.is_some() => s.clone(),
                        // No datatype (perhaps a language tag): the same plain
                        // string literal as a bare `"P30D"`, so read alike.
                        Value::String(s) => duration(s).unwrap_or_else(|| fmt_str(s)),
                        // Sorted keys, as v0.1 printed them (it had no `preserve_order`).
                        other => sorted(other).to_string(),
                    };
                }
                match ident(v) {
                    // `id` and `uid` are the ODRL context's aliases of `@id`.
                    Some(id) => {
                        self.term(&id, TermKind::RightOperand, Some("right operand"));
                        match self.ctx.value(&id) {
                            Some(label) => label,
                            None => self.named(&id),
                        }
                    }
                    None => {
                        self.warn("skipped an unreadable right operand");
                        "an unreadable value".to_string()
                    }
                }
            }
            _ => {
                self.warn("skipped an unreadable right operand");
                "an unreadable value".to_string()
            }
        }
    }
}

/// The value with every object's keys in sorted order.
fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|k| (k.clone(), sorted(&o[k])))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

/// Bare when it reads as an IRI, a number or a date; quoted otherwise.
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The `http://` and `https://` URLs in `s`: each runs to the next space,
/// quote or angle bracket, without trailing punctuation (`.,;:!?'` and a
/// closing bracket), and has something after the scheme.
fn http_urls(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = ["http://", "https://"]
        .iter()
        .filter_map(|p| s[at..].find(p))
        .min()
    {
        let start = at + i;
        let len = s[start..]
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '<' | '>'))
            .unwrap_or(s.len() - start);
        let url = s[start..start + len]
            .trim_end_matches(['.', ',', ';', ':', '!', '?', '\'', ')', ']', '}']);
        let scheme = if url.starts_with("https://") { 8 } else { 7 };
        if url.len() > scheme {
            out.push(url);
        }
        at = start + len.max(1);
    }
    out
}

pub(crate) fn fmt_str(s: &str) -> String {
    let no_space = !s.chars().any(char::is_whitespace);
    let numeric = s.parse::<f64>().is_ok();
    let date = s.len() >= 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-';
    if no_space && (s.contains(':') || numeric || date) {
        s.to_string()
    } else {
        format!("\"{s}\"")
    }
}

pub(crate) fn join_with(items: &[String], conj: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} {conj} {last}", init.join(", ")),
    }
}

/// The IRIs `c` and its sub-clauses refer to, each once, in reading order.
/// Iterative: the depth of a reading is bounded, but not by much.
fn references_in(c: &Condition, out: &mut Vec<String>) {
    let mut stack = vec![c];
    while let Some(c) = stack.pop() {
        if let Some(r) = &c.reference
            && !out.contains(r)
        {
            out.push(r.clone());
        }
        stack.extend(c.children.iter().rev());
    }
}

/// A condition on one line, for places a nested list does not fit.
pub(crate) fn inline(c: &Condition) -> String {
    if c.children.is_empty() {
        return c.text.clone();
    }
    let kids: Vec<String> = c.children.iter().map(inline).collect();
    format!("{}: {}", c.text, kids.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deep ODRL structure (a `constraint` inside a `constraint` ...), which
    /// is walked by `node`, not merely copied. Checked on the walk alone: the
    /// reader proper recurses on such structure too, which is not this pass's
    /// to fix.
    #[test]
    fn the_structural_walk_is_depth_bounded() {
        std::thread::Builder::new()
            .stack_size(1 << 20)
            .spawn(|| {
                let mut v = Value::String("leaf".into());
                for _ in 0..3_000 {
                    let mut m = Obj::new();
                    m.insert("odrl:constraint".into(), v);
                    v = Value::Object(m);
                }
                let mut notes = Vec::new();
                let out = bare_odrl_keys(&v, true, &mut notes);
                // serde_json's Drop recurses as deep as the value: leak the
                // input; the output is cut at MAX_DEPTH and drops fine.
                std::mem::forget(v);
                assert!(notes.iter().any(|n| n.contains("deeper than")), "{notes:?}");
                drop(out);
            })
            .unwrap()
            .join()
            .expect("the walk overflowed its stack or panicked");
    }
}
