//! JSON-LD in, [`Document`] out.
use crate::ProseError;
use crate::model::*;
use crate::words::{self, join, local_name};
use serde_json::{Map, Value};

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
const ODRL_NS: &str = "http://www.w3.org/ns/odrl/2/";
/// The ODRL properties the reader looks up by their bare name.
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

pub fn document(value: &Value) -> Result<Document, ProseError> {
    let value = &bare_odrl_keys(value, odrl_prefix_is_odrl(value));
    let mut r = Reader::default();
    let mut doc = Document::default();
    for v in policy_nodes(value) {
        match v {
            Value::Object(o) if looks_like_policy(o) => doc.policies.push(r.policy(o)),
            _ => r.warn("skipped a node that is not an ODRL policy"),
        }
    }
    if doc.policies.is_empty() {
        return Err(ProseError::NoPolicy);
    }
    doc.warnings = r.warnings;
    Ok(doc)
}

/// `v` with ODRL property keys written as compact IRIs (`odrl:permission`)
/// or full IRIs renamed to the bare term, as a JSON-LD processor compacting
/// against a context that declares the `odrl` prefix but not the terms
/// writes them (EDC does). A bare key already on the object wins: its
/// prefixed twin keeps its name and is reported as unknown. `@context`
/// values are left as they are.
fn bare_odrl_keys(v: &Value, odrl_prefix: bool) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(|x| bare_odrl_keys(x, odrl_prefix)).collect()),
        Value::Object(o) => {
            let mut out = Obj::new();
            for (k, x) in o {
                let x = if k == "@context" {
                    x.clone()
                } else {
                    bare_odrl_keys(x, odrl_prefix)
                };
                let term = k
                    .strip_prefix(ODRL_NS)
                    .or_else(|| k.strip_prefix("odrl:").filter(|_| odrl_prefix))
                    .filter(|t| {
                        ODRL_PROPERTIES.contains(t) && !o.contains_key(*t) && !out.contains_key(*t)
                    });
                out.insert(term.map_or_else(|| k.clone(), String::from), x);
            }
            Value::Object(out)
        }
        _ => v.clone(),
    }
}

/// Whether `odrl:` means ODRL here: no `@context` in the document binds the
/// prefix to another IRI. There is no JSON-LD processing, so an `odrl:` key
/// under a context that rebinds it is reported, not guessed at.
fn odrl_prefix_is_odrl(v: &Value) -> bool {
    fn binds_elsewhere(ctx: &Value) -> bool {
        match ctx {
            Value::Array(a) => a.iter().any(binds_elsewhere),
            Value::Object(c) => c.get("odrl").is_some_and(|b| {
                let iri = match b {
                    Value::Object(d) => d.get("@id").and_then(Value::as_str),
                    _ => b.as_str(),
                };
                iri != Some(ODRL_NS)
            }),
            _ => false,
        }
    }
    match v {
        Value::Array(a) => a.iter().all(odrl_prefix_is_odrl),
        Value::Object(o) => {
            !o.get("@context").is_some_and(binds_elsewhere)
                && o.iter()
                    .filter(|(k, _)| *k != "@context")
                    .all(|(_, x)| odrl_prefix_is_odrl(x))
        }
        _ => true,
    }
}

pub(crate) fn policy_nodes(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().flat_map(policy_nodes).collect(),
        Value::Object(o) if o.contains_key("@graph") => policy_nodes(&o["@graph"]),
        _ => vec![v],
    }
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

#[derive(Default)]
struct Reader {
    warnings: Vec<String>,
    /// The same messages, for de-duplication in constant time.
    seen: std::collections::HashSet<String>,
}

/// What a rule inherits from its policy when it states none of its own.
#[derive(Default, Clone)]
struct Inherited {
    assigner: Vec<String>,
    assignee: Vec<String>,
    target: Vec<String>,
    actions: Vec<(String, Vec<Condition>)>,
}

impl Reader {
    fn warn(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if self.seen.insert(msg.clone()) {
            self.warnings.push(msg);
        }
    }

    fn unknown_keys(&mut self, o: &Obj, known: &[&str], what: &str) {
        // Sorted, not in document order: `serde_json` keeps document order when
        // its `preserve_order` feature is on (the edit module needs it), and
        // the v0.1 warnings were always alphabetical.
        let mut keys: Vec<&String> = o.keys().collect();
        keys.sort();
        for k in keys {
            if known.contains(&k.as_str()) || QUIET_PREFIXES.iter().any(|p| k.starts_with(p)) {
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
        let assigner = self.parties(o.get("assigner"));
        let assignee = self.parties(o.get("assignee"));

        let (label, intro) = match &kind {
            PolicyKind::Set => ("Policy set", "A set of rules".to_string()),
            PolicyKind::Offer => (
                "Offer",
                match assigner.is_empty() {
                    true => "An offer".to_string(),
                    false => format!("An offer made by {}", join(&assigner)),
                },
            ),
            PolicyKind::Agreement => ("Agreement", "An agreement".to_string()),
            PolicyKind::Other(t) => ("Policy", format!("A policy of type {t}")),
        };
        let mut intro = intro;
        match (&kind, assigner.is_empty(), assignee.is_empty()) {
            (PolicyKind::Offer, _, false) => {
                intro.push_str(&format!(", addressed to {}", join(&assignee)))
            }
            (PolicyKind::Offer, _, true) => {}
            (_, false, false) => intro.push_str(&format!(
                " between {} (assigner) and {} (assignee)",
                join(&assigner),
                join(&assignee)
            )),
            (_, false, true) => intro.push_str(&format!(" from {} (assigner)", join(&assigner))),
            (_, true, false) => intro.push_str(&format!(" for {} (assignee)", join(&assignee))),
            (_, true, true) => {}
        }
        intro.push('.');

        let mut notes = Vec::new();
        if let Some(p) = o.get("profile") {
            let ids: Vec<String> = items(p).into_iter().filter_map(ident).collect();
            if !ids.is_empty() {
                notes.push(format!("Written to the profile {}.", join(&ids)));
            }
        }
        if let Some(c) = o.get("conflict").and_then(ident) {
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
            assigner,
            assignee,
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
                };
            }
        };
        self.unknown_keys(o, RULE_KEYS, "a rule");
        let uid = get(o, &["uid", "@id", "id"]).and_then(ident);

        let own_assigner = self.parties(o.get("assigner"));
        let own_assignee = self.parties(o.get("assignee"));
        let own_target = self.assets(o.get("target"));
        let own_actions = self.actions(o.get("action"));
        let assigner = if own_assigner.is_empty() {
            &inh.assigner
        } else {
            &own_assigner
        };
        let assignee = if own_assignee.is_empty() {
            &inh.assignee
        } else {
            &own_assignee
        };
        let target = if own_target.is_empty() {
            &inh.target
        } else {
            &own_target
        };
        let actions = if own_actions.is_empty() {
            &inh.actions
        } else {
            &own_actions
        };

        let action_names: Vec<String> = actions.iter().map(|(a, _)| a.clone()).collect();
        let action = if action_names.is_empty() {
            "perform an unspecified action on".to_string()
        } else {
            join(&action_names)
        };
        let target_text = if target.is_empty() {
            match kind {
                RuleKind::Obligation => String::new(),
                _ => " an unspecified asset".to_string(),
            }
        } else {
            format!(" {}", join(target))
        };
        let by = |verb: &str| match assigner.is_empty() {
            true => String::new(),
            false => format!(", as {verb} by {}", join(assigner)),
        };
        let sentence = match kind {
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

        let refinements = actions
            .iter()
            .filter(|(_, c)| !c.is_empty())
            .map(|(a, c)| Condition {
                text: format!("The action \"{a}\" is limited to cases where"),
                children: c.clone(),
            })
            .collect();
        let conditions = o
            .get("constraint")
            .map(|c| items(c).into_iter().map(|c| self.constraint(c)).collect())
            .unwrap_or_default();

        // Duties of an obligation or follow-ups of a duty inherit nothing:
        // the parties are re-stated by the author or read as "the assignee".
        let child_inh = Inherited {
            assigner: assigner.clone(),
            assignee: assignee.clone(),
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
        }
    }

    // --- parties and assets ---------------------------------------------

    fn parties(&mut self, v: Option<&Value>) -> Vec<String> {
        self.entities(v, "party")
    }

    fn assets(&mut self, v: Option<&Value>) -> Vec<String> {
        self.entities(v, "asset")
    }

    fn entities(&mut self, v: Option<&Value>, noun: &str) -> Vec<String> {
        let Some(v) = v else { return vec![] };
        items(v)
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
                            .map(|c| inline(&self.constraint(c)))
                            .collect();
                        text.push_str(&format!(" where {}", parts.join("; ")));
                    }
                    Some(text)
                }
                _ => {
                    self.warn(format!("skipped an unreadable {noun}"));
                    None
                }
            })
            .collect()
    }

    // --- actions ----------------------------------------------------------

    fn actions(&mut self, v: Option<&Value>) -> Vec<(String, Vec<Condition>)> {
        let Some(v) = v else { return vec![] };
        items(v)
            .into_iter()
            .filter_map(|a| match a {
                Value::String(s) => Some((words::action(s), vec![])),
                Value::Object(o) => {
                    let name = get(o, &["rdf:value", "value", "@id"]).and_then(ident)?;
                    let refine = o
                        .get("refinement")
                        .map(|r| items(r).into_iter().map(|c| self.constraint(c)).collect())
                        .unwrap_or_default();
                    Some((words::action(&name), refine))
                }
                _ => {
                    self.warn("skipped an unreadable action");
                    None
                }
            })
            .collect()
    }

    // --- constraints ------------------------------------------------------

    fn constraint(&mut self, v: &Value) -> Condition {
        let Value::Object(o) = v else {
            return Condition::leaf(match ident(v) {
                Some(id) => format!("the constraint {id} holds"),
                None => {
                    self.warn("skipped an unreadable constraint");
                    "an unreadable constraint holds".to_string()
                }
            });
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
                };
            }
        }
        let left = get(o, &["leftOperand"]).and_then(ident);
        let op = get(o, &["operator"]).and_then(ident);
        let (Some(left), Some(op)) = (left, op) else {
            self.warn("a constraint is missing its leftOperand or operator");
            return Condition::leaf("an incomplete constraint holds");
        };
        let right = if let Some(r) = o.get("rightOperand") {
            let conj = match local_name(&op) {
                "isAnyOf" | "isNoneOf" => "or",
                _ => "and",
            };
            let vals: Vec<String> = items(r).into_iter().map(|x| self.literal(x)).collect();
            let mut text = join_with(&vals, conj);
            if let Some(u) = o.get("unit").and_then(ident) {
                text.push(' ');
                text.push_str(&words::humanise(local_name(&u)));
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
            words::operand(&left),
            words::operator_for(&left, &op),
            right
        ))
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
                    Some(id) => id,
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

/// A condition on one line, for places a nested list does not fit.
pub(crate) fn inline(c: &Condition) -> String {
    if c.children.is_empty() {
        return c.text.clone();
    }
    let kids: Vec<String> = c.children.iter().map(inline).collect();
    format!("{}: {}", c.text, kids.join("; "))
}
