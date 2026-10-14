//! Which parts of ODRL a policy document uses, for the catalog's coverage
//! table. Keys and `@type` values are matched by their ODRL term, bare,
//! `odrl:`-prefixed or as a full IRI.

use crate::sources::term;
use serde_json::Value;
use std::collections::BTreeSet;

/// The features the coverage table lists, in its order, with the group each
/// belongs to.
pub const FEATURES: &[(&str, &str)] = &[
    ("Policy types", "Set"),
    ("Policy types", "Offer"),
    ("Policy types", "Agreement"),
    ("Policy types", "Request"),
    ("Policy types", "Ticket"),
    ("Policy types", "Privacy"),
    ("Policy types", "Assertion"),
    ("Policy types", "Policy"),
    ("Rules", "permission"),
    ("Rules", "prohibition"),
    ("Rules", "obligation"),
    ("Rules", "duty"),
    ("Rules", "remedy"),
    ("Rules", "consequence"),
    ("Conditions", "constraint"),
    ("Conditions", "refinement"),
    ("Conditions", "and"),
    ("Conditions", "or"),
    ("Conditions", "xone"),
    ("Conditions", "andSequence"),
    ("Conditions", "rightOperandReference"),
    ("Conditions", "unit"),
    ("Conditions", "dataType"),
    ("Policy terms", "profile"),
    ("Policy terms", "inheritFrom"),
    ("Policy terms", "conflict"),
    ("Parties and assets", "assigner"),
    ("Parties and assets", "assignee"),
    ("Parties and assets", "attributedParty"),
    ("Parties and assets", "compensatedParty"),
    ("Parties and assets", "trackingParty"),
    ("Parties and assets", "informedParty"),
    ("Parties and assets", "consentingParty"),
    ("Parties and assets", "consentedParty"),
    ("Parties and assets", "AssetCollection"),
    ("Parties and assets", "PartyCollection"),
    ("Parties and assets", "partOf"),
    ("Parties and assets", "source"),
];

const TYPES: &[&str] = &[
    "Set",
    "Offer",
    "Agreement",
    "Request",
    "Ticket",
    "Privacy",
    "Assertion",
    "Policy",
    "AssetCollection",
    "PartyCollection",
];

/// The features of [`FEATURES`] that `doc` uses.
pub fn of(doc: &Value) -> BTreeSet<&'static str> {
    fn walk(v: &Value, out: &mut BTreeSet<&'static str>) {
        match v {
            Value::Object(o) => {
                for (k, x) in o {
                    if k == "@context" {
                        continue;
                    }
                    if k == "@type" {
                        let types: Vec<&str> = match x {
                            Value::String(t) => vec![t.as_str()],
                            Value::Array(ts) => ts.iter().filter_map(Value::as_str).collect(),
                            _ => vec![],
                        };
                        for t in types {
                            if let Some(f) = TYPES.iter().find(|f| **f == term(t)) {
                                out.insert(f);
                            }
                        }
                        continue;
                    }
                    if let Some((_, f)) = FEATURES
                        .iter()
                        .find(|(_, f)| !TYPES.contains(f) && *f == term(k))
                    {
                        out.insert(f);
                    }
                    walk(x, out);
                }
            }
            Value::Array(xs) => xs.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    walk(doc, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_and_types_count_in_any_spelling() {
        let f = of(&json!({
            "@context": { "permission": "ignored" },
            "@type": "odrl:Offer",
            "http://www.w3.org/ns/odrl/2/permission": [{
                "target": { "@type": "AssetCollection", "source": "x" },
                "constraint": [{ "xone": [] }]
            }]
        }));
        let want: BTreeSet<&str> = [
            "Offer",
            "permission",
            "AssetCollection",
            "source",
            "constraint",
            "xone",
        ]
        .into();
        assert_eq!(f, want);
    }
}
