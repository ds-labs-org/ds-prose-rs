//! The reference sources the catalog reads besides the compliance corpus:
//! the examples of the W3C ODRL Information Model, and policies found in
//! JSON files of a repository (EDC samples, Dataspace Protocol examples).
//! None of them is copied into this repository; the build fetches them.

use serde_json::{Map, Value};
use std::path::Path;

/// One policy document found in a source, before it is rendered.
pub struct Found {
    pub slug: String,
    pub title: String,
    /// A sentence or two on what the policy shows, from the source.
    pub about: String,
    /// Where the policy is read from, as a link.
    pub link: String,
    pub policy: Value,
}

const POLICY_TYPES: &[&str] = &[
    "Policy",
    "Set",
    "Offer",
    "Agreement",
    "Request",
    "Ticket",
    "Privacy",
    "Assertion",
];

/// A key or a type value without its ODRL prefix: `odrl:permission` and
/// `http://www.w3.org/ns/odrl/2/permission` both read as `permission`.
pub fn term(name: &str) -> &str {
    name.strip_prefix("odrl:")
        .or_else(|| name.strip_prefix(crate::translate::ODRL))
        .unwrap_or(name)
}

fn has_rules(o: &Map<String, Value>) -> bool {
    o.keys()
        .any(|k| matches!(term(k), "permission" | "prohibition" | "obligation"))
}

fn is_policy(o: &Map<String, Value>) -> bool {
    let typed = match o.get("@type") {
        Some(Value::String(t)) => POLICY_TYPES.contains(&term(t)),
        Some(Value::Array(ts)) => ts
            .iter()
            .any(|t| t.as_str().is_some_and(|t| POLICY_TYPES.contains(&term(t)))),
        _ => false,
    };
    has_rules(o) || typed
}

/// The policies of a JSON document: the document itself when it is one,
/// else every policy nested in it (an EDC policy definition's `policy`, a
/// Dataspace Protocol message's `offer` or `agreement`, a catalog's
/// `hasPolicy`). A nested policy without a `@context` gets the nearest one
/// above it, so it reads as it did in place.
pub fn policies_in(doc: &Value) -> Vec<Value> {
    fn walk(v: &Value, ctx: Option<&Value>, out: &mut Vec<Value>) {
        match v {
            Value::Object(o) => {
                let ctx = o.get("@context").or(ctx);
                if is_policy(o) {
                    let mut p = o.clone();
                    if !p.contains_key("@context")
                        && let Some(c) = ctx
                    {
                        let mut with = Map::new();
                        with.insert("@context".into(), c.clone());
                        with.extend(p);
                        p = with;
                    }
                    out.push(Value::Object(p));
                    return;
                }
                for (k, x) in o {
                    if k != "@context" {
                        walk(x, ctx, out);
                    }
                }
            }
            Value::Array(xs) => xs.iter().for_each(|x| walk(x, ctx, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(doc, None, &mut out);
    out
}

/// "policy/policy-01-policy-enforcement/resources/create-policy.json" reads
/// as "policy-01-policy-enforcement-create-policy".
fn slug_of(rel: &str) -> String {
    let parts: Vec<&str> = rel
        .trim_end_matches(".jsonld")
        .trim_end_matches(".json")
        .split('/')
        .filter(|p| !matches!(*p, "resources" | "src" | "main" | "example" | "examples"))
        .collect();
    let tail = &parts[parts.len().saturating_sub(2)..];
    tail.join("-")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

/// Every policy in the `*.json` and `*.jsonld` files under `dir`, in path
/// order, skipping JSON schemas and build output. `link` is the URL the
/// files are linked under, without the trailing `/`.
pub fn json_dir(dir: &Path, link: &str) -> Result<Vec<Found>, String> {
    fn files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
        for e in std::fs::read_dir(dir)? {
            let p = e?.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if p.is_dir() {
                if !matches!(
                    name,
                    ".git" | "node_modules" | "build" | "target" | "schema" | "schemas"
                ) {
                    files(&p, out)?;
                }
            } else if (name.ends_with(".json") || name.ends_with(".jsonld"))
                && !name.contains("schema")
            {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    files(dir, &mut paths).map_err(|e| format!("{}: {e}", dir.display()))?;
    paths.sort();
    let mut out: Vec<Found> = Vec::new();
    for p in paths {
        let rel = p
            .strip_prefix(dir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(doc) =
            serde_json::from_str::<Value>(&std::fs::read_to_string(&p).unwrap_or_default())
        else {
            continue;
        };
        let found = policies_in(&doc);
        let n = found.len();
        for (i, policy) in found.into_iter().enumerate() {
            let mut slug = slug_of(&rel);
            if n > 1 {
                slug = format!("{slug}-{}", i + 1);
            }
            let wrapper = doc
                .get("@type")
                .and_then(Value::as_str)
                .filter(|t| !POLICY_TYPES.contains(&term(t)));
            let about = match wrapper {
                Some(t) => format!("A policy inside a `{t}` document."),
                None if doc.get("policy").is_some() => {
                    "The `policy` of a policy definition.".to_string()
                }
                None => "A policy document.".to_string(),
            };
            out.push(Found {
                slug: slug.clone(),
                // A path does not wrap on a narrow screen; the slug's words do.
                title: {
                    let words = slug.replace('-', " ");
                    let mut c = words.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                },
                about,
                link: format!("{link}/{rel}"),
                policy,
            });
        }
    }
    Ok(out)
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' if inside => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    unescape(&out)
}

/// Removes a comma before a closing bracket, outside strings: the spec's
/// examples have a few, which JSON does not allow.
fn without_trailing_commas(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let (mut in_str, mut esc) = (false, false);
    for (i, &c) in chars.iter().enumerate() {
        if in_str {
            out.push(c);
            match (esc, c) {
                (true, _) => esc = false,
                (false, '\\') => esc = true,
                (false, '"') => in_str = false,
                _ => {}
            }
            continue;
        }
        if c == '"' {
            in_str = true;
        }
        if c == ',' {
            let next = chars[i + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(next, Some('}' | ']')) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// The JSON values of one example, in order; `None` when it is not JSON
/// (the spec also shows fragments of Turtle and of asset descriptions).
fn json_values(text: &str) -> Option<Vec<Value>> {
    let text = without_trailing_commas(text);
    serde_json::Deserializer::from_str(&text)
        .into_iter::<Value>()
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .filter(|v| !v.is_empty())
}

/// The examples of the ODRL Information Model's HTML that are policies.
/// An example written as several JSON documents (a policy and the
/// constraints it refers to) becomes one `@graph`. `link` is the URL of the
/// document; each example links to its section there.
pub fn w3c_model(html: &str, link: &str) -> Vec<Found> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = html[from..].find("<pre id=\"").map(|i| i + from) {
        let open_end = html[at..].find('>').map_or(html.len(), |i| i + at + 1);
        let close = html[open_end..]
            .find("</pre>")
            .map_or(html.len(), |i| i + open_end);
        let tag = &html[at..open_end];
        let before = &html[from..at];
        from = close;
        if !tag.contains("class=\"example") {
            continue;
        }
        let id = tag["<pre id=\"".len()..]
            .split('"')
            .next()
            .unwrap_or_default()
            .to_string();
        let Some(values) = json_values(strip_tags(&html[open_end..close]).trim()) else {
            continue;
        };
        let policy = if values.len() == 1 {
            values.into_iter().next().unwrap()
        } else {
            let ctx = values[0].get("@context").cloned();
            let graph: Vec<Value> = values
                .into_iter()
                .map(|mut v| {
                    if let Some(o) = v.as_object_mut() {
                        o.remove("@context");
                    }
                    v
                })
                .collect();
            let mut m = Map::new();
            if let Some(c) = ctx {
                m.insert("@context".into(), c);
            }
            m.insert("@graph".into(), Value::Array(graph));
            Value::Object(m)
        };
        if policies_in(&policy).is_empty() {
            continue;
        }
        let head = &html[..at];
        let (section, title) = nearest_section(head);
        let about = before
            .rfind("<blockquote")
            .and_then(|b| {
                let end = before[b..].find("</blockquote>")? + b;
                Some(one_line(&strip_tags(&before[b..end])))
            })
            .unwrap_or_default();
        out.push(Found {
            slug: id.clone(),
            title: format!("{title} ({id})"),
            about,
            link: format!("{link}#{section}"),
            policy,
        });
    }
    out
}

/// The id and heading of the last `<section id>` before `head` that has a
/// heading.
fn nearest_section(head: &str) -> (String, String) {
    let mut at = head.len();
    while let Some(s) = head[..at].rfind("<section id=\"") {
        let rest = &head[s + "<section id=\"".len()..];
        let id = rest.split('"').next().unwrap_or_default();
        let heading = ["<h2", "<h3", "<h4", "<h5"].iter().find_map(|h| {
            let i = rest.find(h)?;
            let inner = &rest[i..];
            let open = inner.find('>')? + 1;
            let close = inner.find("</h")?;
            Some(one_line(&strip_tags(&inner[open..close])))
        });
        if let Some(h) = heading.filter(|h| !h.is_empty()) {
            return (id.to_string(), h);
        }
        at = s;
    }
    (String::new(), "Example".into())
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_policy_inside_a_message_takes_the_message_context() {
        let doc = json!({
            "@context": "https://w3id.org/dspace/2025/1/context.jsonld",
            "@type": "ContractAgreementMessage",
            "agreement": { "@type": "Agreement", "permission": [{ "action": "use" }] }
        });
        let found = policies_in(&doc);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0]["@context"],
            "https://w3id.org/dspace/2025/1/context.jsonld"
        );
        assert_eq!(found[0]["@type"], "Agreement");
    }

    #[test]
    fn a_policy_definition_yields_its_policy_with_its_own_context() {
        let doc = json!({
            "@context": { "@vocab": "https://w3id.org/edc/v0.0.1/ns/" },
            "@id": "eu-policy",
            "policy": { "@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set",
                        "odrl:permission": [] }
        });
        let found = policies_in(&doc);
        assert_eq!(found[0]["@context"], "http://www.w3.org/ns/odrl.jsonld");
    }

    #[test]
    fn the_model_examples_are_read_with_their_section() {
        let html = r#"<section id="duty"><h3>Duty <em>Class</em></h3>
<blockquote><p>Example Use Case: a duty.</p></blockquote>
<pre id="eg9" class="example hljs json">{ "@type": "Offer", "permission": [{ "action": "play", }] }</pre>
<pre id="eg10" class="example hljs json">{ "@type": "dc:Document", "@id": http://x }</pre>
<pre id="eg11" class="example">{ "@type": "Offer", "uid": "p",
  "permission": [{ "action": { "refinement": { "@id": "c1" } } }] }
{ "@id": "c1", "leftOperand": "count", "operator": "lt", "rightOperand": 2 }</pre></section>"#;
        let found = w3c_model(html, "https://w3c.github.io/poe/model/");
        assert_eq!(found.len(), 2, "eg10 is not JSON");
        assert_eq!(found[0].slug, "eg9");
        assert_eq!(found[0].title, "Duty Class (eg9)");
        assert_eq!(found[0].about, "Example Use Case: a duty.");
        assert_eq!(found[0].link, "https://w3c.github.io/poe/model/#duty");
        assert_eq!(found[1].policy["@graph"][1]["@id"], "c1");
    }

    #[test]
    fn a_comma_inside_a_string_stays() {
        assert_eq!(
            without_trailing_commas(r#"{"a": ",}", }"#),
            r#"{"a": ",}" }"#
        );
    }

    #[test]
    fn slugs_drop_the_resource_folders() {
        assert_eq!(
            slug_of("policy/policy-01-policy-enforcement/resources/create-policy.json"),
            "policy-01-policy-enforcement-create-policy"
        );
    }
}
