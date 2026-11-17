//! Limits as the scope of an incoming document. A list limited to `max: 0`
//! is not offered by an editor; here it is also what a document may not
//! hold: [`out_of_scope`] names the sections of a JSON-LD document that sit
//! in such a list, and [`strip_out_of_scope`] removes them.
//!
//! The pass works on the JSON, not on the model: the reader takes bare keys
//! only and the writer keeps the spelling of every key it did not read, so
//! `odrl:assigner` and the full IRI would survive a pass over the model. A
//! key counts as ODRL's when it is bare, or when the document's `@context`
//! (or the `odrl:` prefix) expands it into the ODRL namespace.
use super::rules::{EditRules, Limit};
use crate::profile::Prefixes;
use crate::read::ODRL_NS;
use serde_json::{Map, Value};

/// One section of a document that its limits leave out of scope.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct OutOfScope {
    /// Where it sits: keys and indexes from the document root, such as
    /// `permission[0].duty[1].assigner`; a key is written as in the document.
    pub path: String,
    /// The key as written (`assigner`, `odrl:assigner`, a full IRI).
    pub key: String,
    /// The `EditLimits` name of the list (`ruleAssigner`).
    pub limit: &'static str,
}

/// The sections of `doc` held in a list whose limit is `max: 0`, in document
/// order. Policies themselves are never reported: a document needs one.
pub fn out_of_scope(doc: &Value, rules: &EditRules) -> Vec<OutOfScope> {
    let mut copy = doc.clone();
    strip_out_of_scope(&mut copy, rules)
}

/// Removes what [`out_of_scope`] finds from `doc` and returns it. Everything
/// else, key order included, is kept.
pub fn strip_out_of_scope(doc: &mut Value, rules: &EditRules) -> Vec<OutOfScope> {
    let mut w = Walk {
        rules,
        prefixes: Prefixes::of_document(doc),
        hits: Vec::new(),
    };
    match doc {
        Value::Array(items) => {
            for (i, v) in items.iter_mut().enumerate() {
                w.node(v, &format!("[{i}]"));
            }
        }
        other => w.node(other, ""),
    }
    w.hits
}

/// Whether a limit leaves nothing in the list.
fn none(l: Limit) -> bool {
    l.max == Some(0)
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Owner {
    Policy,
    Rule,
}

struct Walk<'r> {
    rules: &'r EditRules,
    prefixes: Prefixes,
    hits: Vec<OutOfScope>,
}

impl Walk<'_> {
    /// The ODRL property a key stands for, as its bare name.
    fn term(&self, key: &str) -> Option<String> {
        if key.starts_with('@') {
            return None;
        }
        if !key.contains(':') {
            return Some(key.to_string()).filter(|_| {
                // A term the context maps elsewhere is not ODRL's.
                let e = self.prefixes.expand(key);
                e == key || e.starts_with(ODRL_NS)
            });
        }
        let full = match key.strip_prefix("odrl:") {
            Some(rest) => format!("{ODRL_NS}{rest}"),
            None => self.prefixes.expand(key),
        };
        full.strip_prefix(ODRL_NS)
            .filter(|l| !l.is_empty())
            .map(String::from)
    }

    /// A policy node: itself, or the `@graph` that holds the policies.
    fn node(&mut self, v: &mut Value, path: &str) {
        let Value::Object(o) = v else { return };
        if let Some(g) = o.get_mut("@graph") {
            let at = join(path, "@graph");
            match g {
                Value::Array(items) => {
                    for (i, p) in items.iter_mut().enumerate() {
                        if let Value::Object(p) = p {
                            self.object(p, Owner::Policy, &format!("{at}[{i}]"));
                        }
                    }
                }
                Value::Object(p) => self.object(p, Owner::Policy, &at),
                _ => {}
            }
        } else {
            self.object(o, Owner::Policy, path);
        }
    }

    /// The name of the limit that governs `term` under `owner`, when there
    /// is one and it leaves nothing.
    fn banned(&self, owner: Owner, term: &str) -> Option<&'static str> {
        let r = self.rules;
        let (name, limit) = match (owner, term) {
            (Owner::Policy, "permission") => ("permission", r.permission),
            (Owner::Policy, "prohibition") => ("prohibition", r.prohibition),
            (Owner::Policy, "obligation") => ("obligation", r.obligation),
            (Owner::Policy, "assigner") => ("policyAssigner", r.policy_assigner),
            (Owner::Policy, "assignee") => ("policyAssignee", r.policy_assignee),
            (Owner::Policy, "target") => ("policyTarget", r.policy_target),
            (Owner::Policy, "action") => ("policyAction", r.policy_action),
            (Owner::Policy, "uid") => ("policyUid", r.policy_uid),
            (Owner::Policy, "profile") => ("profile", r.profile),
            (Owner::Policy, "inheritFrom") => ("inheritFrom", r.inherit_from),
            (Owner::Rule, "assigner") => ("ruleAssigner", r.rule_assigner),
            (Owner::Rule, "assignee") => ("ruleAssignee", r.rule_assignee),
            (Owner::Rule, "target") => ("ruleTarget", r.rule_target),
            (Owner::Rule, "action") => ("ruleAction", r.rule_action),
            (Owner::Rule, "constraint") => ("constraints", r.constraints),
            (Owner::Rule, "duty") => ("duty", r.duty),
            (Owner::Rule, "remedy") => ("remedy", r.remedy),
            (Owner::Rule, "consequence") => ("consequence", r.consequence),
            _ => return None,
        };
        none(limit).then_some(name)
    }

    /// A policy or a rule: its banned keys go, the rest is walked.
    fn object(&mut self, o: &mut Map<String, Value>, owner: Owner, path: &str) {
        let mut drop: Vec<String> = Vec::new();
        let keys: Vec<String> = o.keys().cloned().collect();
        for key in keys {
            let at = join(path, &key);
            // ODRL's context maps `uid` to `@id`, so a policy's `@id` is its
            // uid. A rule's `@id`, and the bare key `id` (ODRL does not
            // define it), are not read as one here.
            if key == "@id" && owner == Owner::Policy && none(self.rules.policy_uid) {
                self.hits.push(OutOfScope {
                    path: at,
                    key: key.clone(),
                    limit: "policyUid",
                });
                drop.push(key);
                continue;
            }
            let Some(term) = self.term(&key) else {
                continue;
            };
            if let Some(limit) = self.banned(owner, &term) {
                self.hits.push(OutOfScope {
                    path: at,
                    key: key.clone(),
                    limit,
                });
                drop.push(key);
                continue;
            }
            let Some(v) = o.get_mut(&key) else { continue };
            match (owner, term.as_str()) {
                (Owner::Policy, "permission" | "prohibition" | "obligation")
                | (Owner::Rule, "duty" | "remedy" | "consequence") => {
                    self.each(v, &at, |w, x, p| {
                        if let Value::Object(r) = x {
                            w.object(r, Owner::Rule, p);
                        }
                    });
                }
                (_, "assigner" | "assignee" | "target") => {
                    self.each(v, &at, |w, x, p| w.entity(x, p));
                }
                (_, "action") => self.each(v, &at, |w, x, p| w.action(x, p)),
                (Owner::Rule, "constraint") => self.each(v, &at, |w, x, p| w.constraint(x, p)),
                _ => {}
            }
        }
        if !drop.is_empty() {
            o.retain(|k, _| !drop.contains(k));
        }
    }

    /// `f` on each item of a value that is one item or an array of them.
    fn each(&mut self, v: &mut Value, path: &str, f: fn(&mut Self, &mut Value, &str)) {
        match v {
            Value::Array(items) => {
                for (i, x) in items.iter_mut().enumerate() {
                    f(self, x, &format!("{path}[{i}]"));
                }
            }
            other => f(self, other, path),
        }
    }

    /// The `refinement` of an action or a party, when its limit is 0.
    fn refinement(&mut self, v: &mut Value, path: &str, limit: Limit, name: &'static str) {
        let Value::Object(o) = v else { return };
        let keys: Vec<String> = o.keys().cloned().collect();
        let mut drop = Vec::new();
        for key in keys {
            if self.term(&key).as_deref() != Some("refinement") {
                continue;
            }
            if none(limit) {
                self.hits.push(OutOfScope {
                    path: join(path, &key),
                    key: key.clone(),
                    limit: name,
                });
                drop.push(key);
            } else if let Some(c) = o.get_mut(&key) {
                self.each(c, &join(path, &key), |w, x, p| w.constraint(x, p));
            }
        }
        o.retain(|k, _| !drop.contains(k));
    }

    fn entity(&mut self, v: &mut Value, path: &str) {
        let limit = self.rules.entity_refinement;
        self.refinement(v, path, limit, "entityRefinement");
    }

    fn action(&mut self, v: &mut Value, path: &str) {
        let limit = self.rules.action_refinement;
        self.refinement(v, path, limit, "actionRefinement");
    }

    /// A constraint: its logical children, when their limit is 0.
    fn constraint(&mut self, v: &mut Value, path: &str) {
        let Value::Object(o) = v else { return };
        let keys: Vec<String> = o.keys().cloned().collect();
        let mut drop = Vec::new();
        for key in keys {
            let logical = self
                .term(&key)
                .is_some_and(|t| super::LOGICAL_KEYS.contains(&t.as_str()));
            if !logical {
                continue;
            }
            if none(self.rules.logical_children) {
                self.hits.push(OutOfScope {
                    path: join(path, &key),
                    key: key.clone(),
                    limit: "logicalChildren",
                });
                drop.push(key);
            } else if let Some(c) = o.get_mut(&key) {
                self.each(c, &join(path, &key), |w, x, p| w.constraint(x, p));
            }
        }
        o.retain(|k, _| !drop.contains(k));
    }
}
