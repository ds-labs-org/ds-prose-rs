//! A policy whose rules all come from `inheritFrom` (#44). ODRL IM 2.9:
//! the parent's rules are replicated into the child, so a parent in the
//! same document lends its rules to the child, marked as inherited; a
//! parent elsewhere is named as where the rules come from. Neither case is
//! a policy without rules.
use prose_core::{Document, Policy, render};
use serde_json::{Value, json};

const NO_RULES: &str = "a policy has no permission, prohibition or obligation";

fn read(doc: Value) -> Document {
    render(&doc.to_string()).unwrap()
}

fn policy<'a>(doc: &'a Document, uid: &str) -> &'a Policy {
    doc.policies
        .iter()
        .find(|p| p.uid.as_deref() == Some(uid))
        .unwrap_or_else(|| panic!("no policy {uid}"))
}

fn sentences(p: &Policy) -> Vec<&str> {
    p.rules.iter().map(|r| r.sentence.as_str()).collect()
}

/// The compliance corpus's `diamond-inherits-from-grandparent-once-01`, as
/// the catalog translates it: the grandparent's rules reach the child once.
fn diamond() -> Value {
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {
            "case": "https://ds-labs-org.github.io/ds-odrl-compliance-rdf/cases/diamond-inherits-from-grandparent-once-01#"
        }],
        "@graph": [
            {
                "@type": "Set",
                "uid": "case:policy-grandparent",
                "assigner": "did:web:root-provider.example",
                "permission": [{ "uid": "case:perm-use", "action": "use" }],
                "obligation": [{ "uid": "case:duty-notify", "action": "notify" }]
            },
            {
                "@type": "Set",
                "uid": "case:policy-parent-1",
                "inheritFrom": "case:policy-grandparent",
                "assigner": "did:web:root-provider.example"
            },
            {
                "@type": "Set",
                "uid": "case:policy-parent-2",
                "inheritFrom": "case:policy-grandparent",
                "assigner": "did:web:root-provider.example"
            },
            {
                "@type": "Agreement",
                "uid": "case:policy-child",
                "inheritFrom": ["case:policy-parent-1", "case:policy-parent-2"],
                "assignee": "did:web:caller.example"
            }
        ]
    })
}

#[test]
fn a_child_shows_the_rules_of_its_parent_in_the_document() {
    let doc = read(json!({ "@graph": [
        {
            "@type": "Set",
            "uid": "urn:policy:parent",
            "permission": [{ "action": "use", "target": "urn:asset:1" }],
            "prohibition": [{ "action": "distribute", "target": "urn:asset:1" }]
        },
        { "@type": "Set", "uid": "urn:policy:child", "inheritFrom": "urn:policy:parent" }
    ]}));
    assert!(
        !doc.warnings.iter().any(|w| w == NO_RULES),
        "{:?}",
        doc.warnings
    );
    let parent = policy(&doc, "urn:policy:parent");
    let child = policy(&doc, "urn:policy:child");
    assert_eq!(
        sentences(child),
        [
            "Inherited from urn:policy:parent: Anyone may use urn:asset:1.",
            "Inherited from urn:policy:parent: No one may distribute urn:asset:1.",
        ]
    );
    // The replicated rules are the parent's, kind and all.
    let kinds = |p: &Policy| p.rules.iter().map(|r| r.kind).collect::<Vec<_>>();
    assert_eq!(kinds(child), kinds(parent));
    assert_eq!(child.notes, ["Inherits the rules of urn:policy:parent."]);
    // The parent is read as it was.
    assert_eq!(
        sentences(parent),
        [
            "Anyone may use urn:asset:1.",
            "No one may distribute urn:asset:1."
        ]
    );
}

#[test]
fn a_parent_after_its_child_lends_its_rules_too() {
    let doc = read(json!([
        { "@type": "Set", "uid": "urn:policy:child", "inheritFrom": ["urn:policy:parent"] },
        {
            "@type": "Set",
            "uid": "urn:policy:parent",
            "permission": [{ "action": "use", "target": "urn:asset:1" }]
        }
    ]));
    assert_eq!(doc.warnings, Vec::<String>::new());
    assert_eq!(
        sentences(policy(&doc, "urn:policy:child")),
        ["Inherited from urn:policy:parent: Anyone may use urn:asset:1."]
    );
}

#[test]
fn a_parent_written_as_a_compact_iri_is_found_by_its_full_iri() {
    let doc = read(json!({
        "@context": { "ex": "http://example.com/" },
        "@graph": [
            {
                "@type": "Set",
                "uid": "http://example.com/parent",
                "permission": [{ "action": "use", "target": "urn:asset:1" }]
            },
            { "@type": "Set", "uid": "urn:policy:child", "inheritFrom": "ex:parent" }
        ]
    }));
    assert_eq!(
        sentences(policy(&doc, "urn:policy:child")),
        ["Inherited from http://example.com/parent: Anyone may use urn:asset:1."]
    );
}

#[test]
fn own_rules_come_first_then_the_inherited_ones() {
    let doc = read(json!({ "@graph": [
        {
            "@type": "Set",
            "uid": "urn:policy:parent",
            "permission": [{ "action": "use", "target": "urn:asset:1" }]
        },
        {
            "@type": "Set",
            "uid": "urn:policy:child",
            "inheritFrom": "urn:policy:parent",
            "prohibition": [{ "action": "sell", "target": "urn:asset:1" }]
        }
    ]}));
    assert_eq!(
        sentences(policy(&doc, "urn:policy:child")),
        [
            "No one may sell urn:asset:1.",
            "Inherited from urn:policy:parent: Anyone may use urn:asset:1.",
        ]
    );
}

#[test]
fn a_diamond_brings_the_grandparents_rules_once() {
    let doc = read(diamond());
    assert!(
        !doc.warnings.iter().any(|w| w == NO_RULES),
        "{:?}",
        doc.warnings
    );
    let inherited = [
        "Inherited from case:policy-grandparent: Anyone may use an unspecified asset, as permitted by did:web:root-provider.example.",
        "Inherited from case:policy-grandparent: The assignee must notify, as required by did:web:root-provider.example.",
    ];
    for uid in [
        "case:policy-parent-1",
        "case:policy-parent-2",
        "case:policy-child",
    ] {
        assert_eq!(sentences(policy(&doc, uid)), inherited, "{uid}");
    }
    assert_eq!(
        policy(&doc, "case:policy-child").notes,
        ["Inherits the rules of case:policy-parent-1 and case:policy-parent-2."]
    );
}

#[test]
fn a_parent_elsewhere_is_named_as_where_the_rules_come_from() {
    let doc = read(json!({
        "@type": "Set",
        "uid": "urn:policy:child",
        "inheritFrom": "urn:policy:parent"
    }));
    assert_eq!(doc.warnings, Vec::<String>::new());
    let child = &doc.policies[0];
    assert!(child.rules.is_empty());
    assert_eq!(
        child.notes,
        [
            "All of its rules come from the parent policy urn:policy:parent, which is not in this document."
        ]
    );
}

#[test]
fn parents_elsewhere_are_named_together() {
    let doc = read(json!({
        "@type": "Agreement",
        "uid": "urn:policy:child",
        "inheritFrom": ["urn:policy:a", "urn:policy:b"]
    }));
    assert_eq!(doc.warnings, Vec::<String>::new());
    assert_eq!(
        doc.policies[0].notes,
        [
            "All of its rules come from the parent policies urn:policy:a and urn:policy:b, which are not in this document."
        ]
    );
}

#[test]
fn a_policy_without_rules_or_parent_still_warns() {
    let doc = read(json!({
        "@type": "Set",
        "uid": "urn:policy:a",
        "assigner": "did:web:provider.example"
    }));
    assert_eq!(doc.warnings, [NO_RULES]);
    assert!(doc.policies[0].notes.is_empty());
}

#[test]
fn a_cycle_ends_and_never_brings_a_policy_its_own_rules() {
    let doc = read(json!({ "@graph": [
        {
            "@type": "Set",
            "uid": "urn:policy:a",
            "inheritFrom": "urn:policy:b",
            "permission": [{ "action": "use", "target": "urn:asset:1" }]
        },
        {
            "@type": "Set",
            "uid": "urn:policy:b",
            "inheritFrom": "urn:policy:a",
            "permission": [{ "action": "play", "target": "urn:asset:1" }]
        }
    ]}));
    assert_eq!(
        sentences(policy(&doc, "urn:policy:a")),
        [
            "Anyone may use urn:asset:1.",
            "Inherited from urn:policy:b: Anyone may play urn:asset:1.",
        ]
    );
    assert_eq!(
        sentences(policy(&doc, "urn:policy:b")),
        [
            "Anyone may play urn:asset:1.",
            "Inherited from urn:policy:a: Anyone may use urn:asset:1.",
        ]
    );
}
