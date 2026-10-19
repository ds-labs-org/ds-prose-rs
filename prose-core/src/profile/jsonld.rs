//! JSON-LD to triples, without a JSON-LD processor.
//!
//! The `@context` subset followed: string prefix and term entries, a string
//! `@vocab`, `@base` and `@language`, term definitions `{"@id": .., "@type": "@id" |
//! "@vocab" | datatype}`, and arrays of these merged left to right. Anything
//! else gets one warning naming the key. The W3C ODRL context, named as a
//! remote context, turns on the prefixes and terms it is known to define.
//!
//! The walk is iterative with an explicit stack bounded by `max_depth`, so a
//! hostile value cannot overflow the call stack.
use super::ns;
use super::triples::{Object, Parsed, Sink};
use super::{ProfileError, ProfileLimits};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::rc::Rc;

type Obj = Map<String, Value>;

#[derive(Clone, Debug)]
enum Coerce {
    None,
    Id,
    Vocab,
    Datatype(String),
}

#[derive(Clone, Debug)]
struct Def {
    /// An absolute IRI, or a keyword (`@id`, `@type`) for an alias.
    iri: String,
    coerce: Coerce,
}

#[derive(Clone, Debug, Default)]
struct Ctx {
    /// The enclosing context: a term not defined here is looked up there.
    /// Shared, not copied, so many small contexts under a large one stay
    /// cheap. The chain is as long as the node nesting, so `max_depth`
    /// bounds it.
    parent: Option<Rc<Ctx>>,
    /// The terms this context defines; `None` is a `null` definition that
    /// removes the enclosing one.
    terms: HashMap<String, Option<Def>>,
    /// `@vocab`, `@base` and `@language` are shared with the contexts under
    /// this one, not copied: an empty nested context costs nothing.
    vocab: Option<Rc<str>>,
    base: Option<Rc<str>>,
    /// The default language of plain strings (`@language`), lower-cased.
    language: Option<Rc<str>>,
    /// The W3C ODRL context was named.
    odrl: bool,
}

impl Ctx {
    /// The definition of `k` here or in an enclosing context.
    fn term(&self, k: &str) -> Option<&Def> {
        let mut c = self;
        loop {
            if let Some(d) = c.terms.get(k) {
                return d.as_ref();
            }
            c = c.parent.as_deref()?;
        }
    }
}

/// What a node key means here.
enum Key {
    Keyword(String),
    Prop(String, Coerce),
    Drop,
}

/// A piece of the document still to read.
enum Work<'a> {
    /// A value at the top level or under `@graph`: a node, or an array.
    Top(&'a Value, Rc<Ctx>, usize),
    /// A node object whose subject is already named.
    Node(&'a Obj, Rc<Ctx>, String, usize),
}

/// `base`: the document's own address, if the host knows it; the initial
/// `@base`.
pub(crate) fn read(
    v: &Value,
    base: Option<&str>,
    limits: &ProfileLimits,
) -> Result<Parsed, ProfileError> {
    shape(v)?;
    let mut w = Walker {
        sink: Sink::new(limits),
        max_depth: limits.max_depth,
        blank: 0,
        cut: false,
        base: None,
    };
    match base {
        Some(b) if absolute(b) => w.base = Some(Rc::from(b)),
        Some(b) => w.warn(format!(
            "the base IRI \"{b}\" is not absolute; relative IRIs are skipped"
        )),
        None => {}
    }
    let root = Ctx {
        base: w.base.clone(),
        ..Ctx::default()
    };
    let mut stack = vec![Work::Top(v, Rc::new(root), 0)];
    while let Some(work) = stack.pop() {
        let mut next = Vec::new();
        match work {
            Work::Top(v, ctx, depth) => w.top(v, ctx, depth, &mut next),
            Work::Node(o, ctx, s, depth) => w.node(o, &ctx, &s, depth, &mut next),
        }
        if w.sink.over() {
            return Err(Sink::too_expanded());
        }
        // Reversed, so the stack reads in document order.
        stack.extend(next.into_iter().rev());
    }
    Ok(w.sink.parsed)
}

/// The specific errors for documents that are something else than a
/// profile, before the generic `NoTerms`.
fn shape(v: &Value) -> Result<(), ProfileError> {
    let Value::Object(o) = v else { return Ok(()) };
    if o.contains_key("$schema") {
        return Err(ProfileError::JsonSchema);
    }
    if !o.is_empty() && o.keys().all(|k| k == "@context") {
        return Err(ProfileError::ContextOnly);
    }
    let rule_key = |k: &str| {
        let local = k
            .strip_prefix(ns::ODRL)
            .or_else(|| k.strip_prefix("odrl:"))
            .unwrap_or(k);
        matches!(local, "permission" | "prohibition" | "obligation")
    };
    if o.keys().any(|k| rule_key(k)) {
        return Err(ProfileError::LooksLikePolicy);
    }
    Ok(())
}

struct Walker<'l> {
    sink: Sink<'l>,
    max_depth: usize,
    blank: usize,
    cut: bool,
    /// The base the host gave, kept by a `null` context.
    base: Option<Rc<str>>,
}

impl Walker<'_> {
    fn warn(&mut self, msg: impl Into<String>) {
        self.sink.warn(msg);
    }

    /// `false`, with one warning, when `depth` is past the limit.
    fn within(&mut self, depth: usize) -> bool {
        if depth <= self.max_depth {
            return true;
        }
        if !self.cut {
            self.cut = true;
            let max = self.max_depth;
            self.warn(format!(
                "content nested deeper than {max} levels is not read"
            ));
        }
        false
    }

    fn top<'a>(&mut self, v: &'a Value, ctx: Rc<Ctx>, depth: usize, next: &mut Vec<Work<'a>>) {
        if !self.within(depth) {
            return;
        }
        match v {
            Value::Array(a) => next.extend(a.iter().map(|x| Work::Top(x, ctx.clone(), depth + 1))),
            Value::Object(o) => {
                let ctx = self.local(&ctx, o);
                let is_node = o.keys().any(|k| k != "@context" && k != "@graph");
                if is_node {
                    if let Some(s) = self.subject(o, &ctx) {
                        self.node(o, &ctx, &s, depth, next);
                    }
                } else if let Some(g) = o.get("@graph") {
                    next.push(Work::Top(g, ctx, depth + 1));
                }
            }
            Value::Null => {}
            _ => self.warn("skipped a top-level value that is not a node object"),
        }
    }

    /// `ctx` with the node's own `@context`, if it has one.
    fn local(&mut self, ctx: &Rc<Ctx>, o: &Obj) -> Rc<Ctx> {
        match o.get("@context") {
            Some(c) => Rc::new(self.context(ctx, c)),
            None => ctx.clone(),
        }
    }

    /// The node's subject: its `@id` expanded, or a new blank node. `None`
    /// when the `@id` cannot be made absolute; the node is then skipped.
    fn subject(&mut self, o: &Obj, ctx: &Ctx) -> Option<String> {
        let id = o
            .iter()
            .find(|(k, _)| matches!(self.key(ctx, k), Key::Keyword(kw) if kw == "@id"))
            .map(|(_, v)| v);
        match id {
            None => {
                self.blank += 1;
                Some(format!("_:g{}", self.blank))
            }
            Some(Value::String(s)) => {
                let iri = self.expand(ctx, s, false);
                if iri.is_none() {
                    self.warn(format!(
                        "skipped the node with the relative @id \"{s}\": there is no absolute @base"
                    ));
                }
                iri
            }
            Some(_) => {
                self.warn("skipped a node whose @id is not a string");
                None
            }
        }
    }

    fn node<'a>(
        &mut self,
        o: &'a Obj,
        ctx: &Rc<Ctx>,
        s: &str,
        depth: usize,
        next: &mut Vec<Work<'a>>,
    ) {
        for (k, v) in o {
            if self.sink.over() {
                return;
            }
            match self.key(ctx, k) {
                Key::Keyword(kw) => match kw.as_str() {
                    "@context" | "@id" => {}
                    "@type" => {
                        for t in flat(v) {
                            let Some(t) = t.as_str() else { continue };
                            match self.expand(ctx, t, true) {
                                Some(iri) => self.sink.push(s, ns::RDF_TYPE, Object::Iri(iri)),
                                None => self.warn(format!(
                                    "ignored the type \"{t}\": no @vocab or term definition expands it"
                                )),
                            }
                        }
                    }
                    "@graph" => next.push(Work::Top(v, ctx.clone(), depth + 1)),
                    other => self.warn(format!("the key \"{other}\" is not followed")),
                },
                Key::Prop(p, coerce) => self.values(ctx, s, &p, &coerce, v, depth + 1, next),
                Key::Drop => self.warn(format!(
                    "ignored the key \"{k}\": no @vocab or term definition expands it"
                )),
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn values<'a>(
        &mut self,
        ctx: &Rc<Ctx>,
        s: &str,
        p: &str,
        coerce: &Coerce,
        v: &'a Value,
        depth: usize,
        next: &mut Vec<Work<'a>>,
    ) {
        // Arrays, `@list` and `@set` nest; walk them without recursion. Items
        // go on the stack reversed, so they are read in document order.
        let mut todo = vec![(v, depth)];
        while let Some((v, depth)) = todo.pop() {
            if self.sink.over() {
                return;
            }
            if !self.within(depth) {
                continue;
            }
            match v {
                Value::Null => {}
                Value::Array(a) => todo.extend(a.iter().rev().map(|x| (x, depth + 1))),
                Value::String(text) => match coerce {
                    Coerce::Id => self.iri_value(ctx, s, p, text, false),
                    Coerce::Vocab => self.iri_value(ctx, s, p, text, true),
                    Coerce::Datatype(dt) => self.sink.push(s, p, literal(text, None, Some(dt))),
                    Coerce::None if iri_valued(p) => self.iri_value(ctx, s, p, text, false),
                    Coerce::None => {
                        self.sink
                            .push(s, p, literal(text, ctx.language.as_deref(), None))
                    }
                },
                Value::Number(n) => {
                    let dt = if n.is_f64() { "double" } else { "integer" };
                    let dt = format!("{}{dt}", ns::XSD);
                    self.sink
                        .push(s, p, literal(&n.to_string(), None, Some(&dt)));
                }
                Value::Bool(b) => {
                    let dt = format!("{}boolean", ns::XSD);
                    self.sink
                        .push(s, p, literal(&b.to_string(), None, Some(&dt)));
                }
                Value::Object(m) => {
                    if let Some(inner) = m.get("@value") {
                        self.value_object(ctx, s, p, m, inner);
                    } else if let Some(inner) = m.get("@list").or_else(|| m.get("@set")) {
                        todo.push((inner, depth + 1));
                    } else {
                        let local = self.local(ctx, m);
                        let only_id = m
                            .keys()
                            .all(|k| matches!(self.key(&local, k), Key::Keyword(kw) if kw == "@id" || kw == "@context"));
                        let Some(child) = self.subject(m, &local) else {
                            continue;
                        };
                        self.sink.push(s, p, Object::Iri(child.clone()));
                        if !only_id {
                            next.push(Work::Node(m, local, child, depth + 1));
                        }
                    }
                }
            }
        }
    }

    /// `{"@value": .., "@language" | "@type": ..}`.
    fn value_object(&mut self, ctx: &Ctx, s: &str, p: &str, m: &Obj, inner: &Value) {
        let text = match inner {
            Value::String(t) => t.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => return,
            _ => {
                self.warn("skipped a JSON literal (@value holding an object or array)");
                return;
            }
        };
        let lang = m.get("@language").and_then(Value::as_str);
        let dt = m
            .get("@type")
            .and_then(Value::as_str)
            .and_then(|t| self.expand(ctx, t, true));
        let dt = dt.or_else(|| match inner {
            Value::Number(n) if n.is_f64() => Some(format!("{}double", ns::XSD)),
            Value::Number(_) => Some(format!("{}integer", ns::XSD)),
            Value::Bool(_) => Some(format!("{}boolean", ns::XSD)),
            _ => None,
        });
        self.sink.push(s, p, literal(&text, lang, dt.as_deref()));
    }

    fn iri_value(&mut self, ctx: &Ctx, s: &str, p: &str, text: &str, vocab: bool) {
        match self.expand(ctx, text, vocab) {
            Some(iri) => self.sink.push(s, p, Object::Iri(iri)),
            None => self.warn(format!(
                "skipped the relative IRI \"{text}\": there is no absolute @base"
            )),
        }
    }

    /// What the node key `k` means under `ctx`.
    fn key(&mut self, ctx: &Ctx, k: &str) -> Key {
        if k.starts_with('@') {
            return Key::Keyword(k.to_string());
        }
        if let Some(def) = ctx.term(k) {
            return match def.iri.starts_with('@') {
                true => Key::Keyword(def.iri.clone()),
                false => Key::Prop(self.charged(def.iri.clone()), def.coerce.clone()),
            };
        }
        if ctx.odrl {
            match k {
                "uid" => return Key::Keyword("@id".into()),
                "type" => return Key::Keyword("@type".into()),
                _ => {}
            }
        }
        match self.expand(ctx, k, true) {
            Some(iri) => Key::Prop(iri, Coerce::None),
            None => Key::Drop,
        }
    }

    /// `s` as an absolute IRI. `vocab`: `s` sits where a term or `@vocab`
    /// applies (a key, a type); otherwise a bare word is resolved against
    /// `@base`. `None` when nothing makes it absolute.
    fn expand(&mut self, ctx: &Ctx, s: &str, vocab: bool) -> Option<String> {
        if self.sink.over() {
            // The profile is refused; expand nothing more.
            return Some(String::new());
        }
        if vocab
            && let Some(def) = ctx.term(s)
            && !def.iri.starts_with('@')
        {
            return Some(self.charged(def.iri.clone()));
        }
        if let Some((prefix, suffix)) = s.split_once(':') {
            return Some(self.prefixed(ctx, s, prefix, suffix));
        }
        if vocab {
            if let Some(v) = &ctx.vocab {
                return Some(self.charged(format!("{v}{s}")));
            }
            if ctx.odrl {
                return Some(ns::odrl(s));
            }
        }
        let iri = ctx.base.as_deref().map(|b| resolve(b, s))?;
        Some(self.charged(iri))
    }

    /// `iri`, counted against `max_expanded_bytes`.
    fn charged(&mut self, iri: String) -> String {
        self.sink.charge(iri.len());
        iri
    }

    /// `prefix:suffix`: a compact IRI when the prefix is bound (by the
    /// document, the ODRL context, or the well-known fallback), else an
    /// absolute IRI with that scheme.
    fn prefixed(&mut self, ctx: &Ctx, s: &str, prefix: &str, suffix: &str) -> String {
        if self.sink.over() {
            // The profile is refused; expand nothing more.
            return String::new();
        }
        if prefix == "_" || suffix.starts_with("//") {
            return s.to_string();
        }
        if let Some(def) = ctx.term(prefix)
            && !def.iri.starts_with('@')
        {
            return self.charged(format!("{}{suffix}", def.iri));
        }
        if ctx.odrl
            && let Some((_, ns)) = ns::ODRL_CONTEXT.iter().find(|(p, _)| *p == prefix)
        {
            return format!("{ns}{suffix}");
        }
        if let Some((_, ns)) = ns::WELL_KNOWN.iter().find(|(p, _)| *p == prefix) {
            self.warn(format!(
                "the prefix \"{prefix}\" is not declared in the @context; read as {ns}"
            ));
            return format!("{ns}{suffix}");
        }
        s.to_string()
    }

    /// `ctx` with the context value `c` applied.
    fn context(&mut self, ctx: &Rc<Ctx>, c: &Value) -> Ctx {
        let mut out = Ctx {
            parent: Some(ctx.clone()),
            terms: HashMap::new(),
            vocab: ctx.vocab.clone(),
            base: ctx.base.clone(),
            language: ctx.language.clone(),
            odrl: ctx.odrl,
        };
        let list: Vec<&Value> = match c {
            Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for c in list {
            match c {
                Value::Null => {
                    out = Ctx {
                        base: self.base.clone(),
                        ..Ctx::default()
                    }
                }
                Value::String(s) if ns::is_odrl_context(s) => {
                    out.odrl = true;
                    for (p, iri) in ns::ODRL_CONTEXT {
                        self.sink.bind(p, iri);
                    }
                }
                Value::String(s) => self.warn(format!(
                    "the remote context \"{s}\" is not fetched; only inline definitions are read"
                )),
                Value::Object(o) => self.context_object(&mut out, o),
                _ => self.warn("skipped a @context entry that is not an object or a string"),
            }
        }
        out
    }

    fn context_object(&mut self, out: &mut Ctx, o: &Obj) {
        for (k, v) in o.iter().filter(|(k, _)| k.starts_with('@')) {
            match (k.as_str(), v) {
                // Counted once here, whether it is used or not.
                ("@vocab", Value::String(s)) => {
                    out.vocab = self.expand(out, s, true).map(|v| Rc::from(self.charged(v)))
                }
                ("@vocab", Value::Null) => out.vocab = None,
                ("@base", Value::String(s)) => {
                    let base = match absolute(s) {
                        true => Some(s.clone()),
                        false => out.base.as_deref().map(|b| resolve(b, s)),
                    };
                    out.base = base.map(|b| Rc::from(self.charged(b)))
                }
                ("@base", Value::Null) => out.base = None,
                ("@language", Value::String(l)) => {
                    out.language = Some(Rc::from(l.to_ascii_lowercase()))
                }
                ("@language", Value::Null) => out.language = None,
                ("@version", _) => {}
                (k, _) => self.warn(format!("the context key \"{k}\" is not followed")),
            }
        }
        // Term definitions, raw first: one may use a prefix another defines.
        let mut raw: Vec<(&str, &str, Option<&str>)> = Vec::new();
        for (k, v) in o.iter().filter(|(k, _)| !k.starts_with('@')) {
            match v {
                Value::Null => {
                    out.terms.insert(k.clone(), None);
                }
                Value::String(s) => raw.push((k, s, None)),
                Value::Object(d) => {
                    if d.contains_key("@reverse") {
                        self.warn(format!(
                            "the context key \"@reverse\" in the definition of \"{k}\" is not followed"
                        ));
                        continue;
                    }
                    for dk in d
                        .keys()
                        .filter(|dk| !matches!(dk.as_str(), "@id" | "@type"))
                    {
                        self.warn(format!(
                            "the context key \"{dk}\" in the definition of \"{k}\" is not followed"
                        ));
                    }
                    let id = d.get("@id").and_then(Value::as_str).unwrap_or(k);
                    raw.push((k, id, d.get("@type").and_then(Value::as_str)));
                }
                _ => self.warn(format!("skipped the context entry \"{k}\"")),
            }
        }
        for (k, id, _) in &raw {
            out.terms.insert(
                k.to_string(),
                Some(Def {
                    iri: id.to_string(),
                    coerce: Coerce::None,
                }),
            );
        }
        // A few passes resolve chains such as "train": "ex:train".
        for _ in 0..4 {
            for (k, id, ty) in &raw {
                let iri = match id.starts_with('@') {
                    true => id.to_string(),
                    false => self.def_iri(out, k, id),
                };
                let coerce = match ty {
                    None => Coerce::None,
                    Some("@id") => Coerce::Id,
                    Some("@vocab") => Coerce::Vocab,
                    Some(t) => Coerce::Datatype(self.def_iri(out, k, t)),
                };
                out.terms.insert(k.to_string(), Some(Def { iri, coerce }));
            }
        }
        for (k, _, _) in &raw {
            if let Some(Some(d)) = out.terms.get(*k)
                && d.iri.ends_with(['/', '#', ':'])
            {
                let iri = d.iri.clone();
                self.sink.bind(k, &iri);
            }
        }
    }

    /// A term definition's IRI, expanded without looking the term up in
    /// itself.
    fn def_iri(&mut self, ctx: &Ctx, term: &str, id: &str) -> String {
        if let Some((prefix, suffix)) = id.split_once(':') {
            if prefix == term {
                return id.to_string();
            }
            return self.prefixed(ctx, id, prefix, suffix);
        }
        match (&ctx.vocab, ctx.odrl) {
            (Some(v), _) => {
                let iri = format!("{v}{id}");
                self.charged(iri)
            }
            (None, true) => ns::odrl(id),
            (None, false) => id.to_string(),
        }
    }
}

/// `v` as a list of items: an array's items, or `v` alone.
fn flat(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        other => vec![other],
    }
}

fn literal(text: &str, lang: Option<&str>, datatype: Option<&str>) -> Object {
    Object::Literal {
        value: text.to_string(),
        lang: lang.map(str::to_ascii_lowercase),
        datatype: datatype
            .filter(|d| *d != ns::XSD_STRING && lang.is_none())
            .map(String::from),
    }
}

/// Properties whose string values are IRIs even without a term definition
/// that says so.
fn iri_valued(p: &str) -> bool {
    if let Some(local) = p.strip_prefix(ns::PROSE) {
        // `prose:closed` and `prose:plural` are booleans; the other hints
        // name terms.
        return !matches!(local, "closed" | "plural");
    }
    let local =
        |base: &str, names: &[&str]| p.strip_prefix(base).is_some_and(|l| names.contains(&l));
    local(ns::ODRL, &["includedIn", "implies"])
        || local(
            ns::SKOS,
            &[
                "broader",
                "narrower",
                "exactMatch",
                "closeMatch",
                "member",
                "inScheme",
            ],
        )
        || local(
            ns::RDFS,
            &[
                "subClassOf",
                "subPropertyOf",
                "isDefinedBy",
                "seeAlso",
                "domain",
                "range",
            ],
        )
        || local(
            ns::OWL,
            &["imports", "sameAs", "equivalentClass", "equivalentProperty"],
        )
        || local(ns::PROF, &["isProfileOf"])
        || p == ns::RDF_TYPE
}

/// Whether `s` has a scheme: `https://..`, `urn:..`.
fn absolute(s: &str) -> bool {
    match s.split_once(':') {
        Some((scheme, _)) => {
            !scheme.is_empty()
                && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        None => false,
    }
}

/// `rel` against the absolute `base`, enough for the references profiles
/// use (`#x`, `x`, `/x`, `//host/x`). Dot segments are not normalised.
fn resolve(base: &str, rel: &str) -> String {
    let no_frag = base.split('#').next().unwrap_or(base);
    let no_query = no_frag.split('?').next().unwrap_or(no_frag);
    let (scheme, rest) = no_query.split_once(':').unwrap_or(("", no_query));
    if rel.is_empty() {
        return no_frag.to_string();
    }
    if rel.starts_with('#') {
        return format!("{no_frag}{rel}");
    }
    if rel.starts_with('?') {
        return format!("{no_query}{rel}");
    }
    if rel.starts_with("//") {
        return format!("{scheme}:{rel}");
    }
    if rel.starts_with('/') {
        let authority_end = rest
            .strip_prefix("//")
            .map(|r| r.find('/').map_or(no_query.len(), |i| scheme.len() + 3 + i))
            .unwrap_or(scheme.len() + 1);
        return format!("{}{rel}", &no_query[..authority_end]);
    }
    match no_query.rfind('/') {
        Some(i) if i > scheme.len() + 2 => format!("{}{rel}", &no_query[..=i]),
        _ => format!("{no_query}/{rel}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_references() {
        let b = "https://example.org/p/q?x#f";
        assert_eq!(resolve(b, "#t"), "https://example.org/p/q?x#t");
        assert_eq!(resolve(b, "t"), "https://example.org/p/t");
        assert_eq!(resolve(b, "/t"), "https://example.org/t");
        assert_eq!(resolve(b, "//other.org/t"), "https://other.org/t");
        assert_eq!(resolve("https://example.org", "t"), "https://example.org/t");
        assert!(absolute("urn:x:y"));
        assert!(!absolute("train"));
        assert!(!absolute("#x"));
    }
}
