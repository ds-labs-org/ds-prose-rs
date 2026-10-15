//! The editor's reading of a policy whose rules all come from `inheritFrom`
//! (#44) warns as the rendering does: not when its parents lend it rules or
//! are not in the document, but when inheriting gives it none.
use prose_core::{edit::read_model, render};
use serde_json::{Value, json};

const NO_RULES: &str = "a policy has no permission, prohibition or obligation";

fn edit_warns(doc: &Value) -> bool {
    let warnings = read_model(&doc.to_string()).unwrap().warnings;
    warnings.iter().any(|w| w == NO_RULES)
}

fn render_warns(doc: &Value) -> bool {
    let warnings = render(&doc.to_string()).unwrap().warnings;
    warnings.iter().any(|w| w == NO_RULES)
}

#[test]
fn the_editor_warns_as_the_rendering_does() {
    let cases = [
        (
            "parent here",
            json!({ "@graph": [
                {
                    "@type": "Set",
                    "uid": "urn:p:parent",
                    "permission": [{ "action": "use", "target": "urn:a" }]
                },
                { "@type": "Set", "uid": "urn:p:child", "inheritFrom": "urn:p:parent" }
            ]}),
            false,
        ),
        (
            "parent elsewhere",
            json!({ "@type": "Set", "uid": "http://example.com/c", "inheritFrom": "http://example.com/p" }),
            false,
        ),
        (
            "grandparent here",
            json!([
                { "@type": "Set", "uid": "urn:c", "inheritFrom": "http://example.com/p" },
                { "@type": "Set", "uid": "http://example.com/p", "inheritFrom": "urn:g" },
                {
                    "@type": "Set",
                    "uid": "urn:g",
                    "obligation": [{ "action": "notify" }]
                }
            ]),
            false,
        ),
        ("no parent", json!({ "@type": "Set", "uid": "urn:c" }), true),
        (
            "itself",
            json!({ "@type": "Set", "uid": "urn:c", "inheritFrom": "urn:c" }),
            true,
        ),
        (
            "each other",
            json!({ "@graph": [
                { "@type": "Set", "uid": "urn:p", "inheritFrom": "urn:c" },
                { "@type": "Set", "uid": "urn:c", "inheritFrom": "urn:p" }
            ]}),
            true,
        ),
    ];
    for (name, doc, warns) in cases {
        assert_eq!(render_warns(&doc), warns, "render: {name}");
        assert_eq!(edit_warns(&doc), warns, "edit: {name}");
    }
}
