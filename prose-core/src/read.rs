//! JSON-LD in, [`Document`] out.
use crate::ProseError;
use crate::model::*;
use crate::profile::{Budget, Prefixes, TermKind, Wording};
use crate::words::{humanise, join, local_name};
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

/// The most constraint references one document's reading follows.
const MAX_FOLLOWED: usize = 1000;

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
    let nodes = policy_nodes(value);
    r.iris = Prefixes::of_document(value);
    for v in declared_constraints(value) {
        if let Some(id) = ident(v) {
            let id = r.iri(&id);
            r.declared.entry(id).or_insert_with(|| v.clone());
        }
    }
    for (i, v) in nodes.into_iter().enumerate() {
        match v {
            Value::Object(o) if looks_like_policy(o) => {
                if let Some(p) = scopes.get(i) {
                    r.ctx.set_prefixes(Prefixes::default().inside(p.clone()));
                }
                doc.policies.push(r.policy(o));
                r.glossed.clear();
                glossary.push(PolicyGlossary {
                    terms: std::mem::take(&mut r.glossary),
                });
            }
            // Read where a rule refers to it.
            Value::Object(o) if looks_like_constraint(o) && ident(v).is_some() => {}
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
    })
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
    /// How deep the constraint being read is nested, references included.
    nesting: usize,
    /// The document's prefixes, to tell two spellings of one constraint
    /// identifier (`ex:c1`, `http://ex.com/c1`) for the same IRI.
    iris: Prefixes,
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
            nesting: 0,
            iris: Prefixes::default(),
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

    fn warn(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
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

    fn unknown_keys(&mut self, o: &Obj, known: &[&str], what: &str) {
        self.unknown_keys_but(o, known, &[], what);
    }

    /// As `unknown_keys`, with `also` known too.
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

    fn policy(&mut self, o: &Obj) -> Policy {
        self.unknown_keys(o, POLICY_KEYS, "a policy");
        let kind = match types(o)
            .iter()
            .map(|t| local_name(t))
            .find(|t| !matches!(*t, "Policy"))
        {
            Some("Set") => PolicyKind::Set,
            Some("Offer") => PolicyKind::Offer,
            Some("Agreement") => PolicyKind::Agreement,
            Some(other) => PolicyKind::Other(other.to_string()),
            None => PolicyKind::Other("Policy".into()),
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
        if let Some(p) = o.get("inheritFrom") {
            let ids: Vec<String> = items(p).into_iter().filter_map(ident).collect();
            if !ids.is_empty() {
                notes.push(format!("Inherits the rules of {}.", join(&ids)));
            }
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
        if rules.is_empty() {
            self.warn("a policy has no permission, prohibition or obligation");
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
        let by = |verb: &str| match assigner.is_empty() {
            true => String::new(),
            false => format!(", as {verb} by {}", join(assigner)),
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
                Value::String(s) => Some(s.clone()),
                Value::Object(o) => {
                    let id = get(o, &["uid", "@id", "id"]).and_then(Value::as_str);
                    let collection = o.get("source").or_else(|| o.get("partOf")).and_then(ident);
                    let mut text = match (id, collection) {
                        (Some(id), None) => id.to_string(),
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
        Condition::leaf(format!(
            "{} {} {}",
            self.ctx.operand(&left),
            self.ctx.operator_for(&left, &op),
            right
        ))
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

    fn literal(&mut self, v: &Value) -> String {
        match v {
            Value::String(s) => fmt_str(s),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Object(o) => {
                if let Some(inner) = o.get("@value") {
                    let typed = o.get("@type").and_then(Value::as_str).is_some();
                    return match inner {
                        Value::String(s) if typed => s.clone(),
                        Value::String(s) => fmt_str(s),
                        // Sorted keys, as v0.1 printed them (it had no `preserve_order`).
                        other => sorted(other).to_string(),
                    };
                }
                match ident(v) {
                    // `id` and `uid` are the ODRL context's aliases of `@id`.
                    Some(id) => {
                        self.term(&id, TermKind::RightOperand, Some("right operand"));
                        self.ctx.value(&id).unwrap_or(id)
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
