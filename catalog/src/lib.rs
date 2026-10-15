//! The site's catalog of real-world ODRL policies, read as prose.
//!
//! Nothing of the sources is copied into this repository: the build fetches
//! them (`scripts/build-catalog.sh`) and runs `compliance-catalog`, which
//! reads each source's policies, renders them with `OdrlProse` and writes
//! `site/content/catalog/`: an overview with a coverage table, one index per
//! source, one page per policy.
//!
//! - The [ds-odrl-compliance-rdf] corpus, in Turtle: each case's policies are
//!   translated to ODRL JSON-LD ([`translate`]). The build fails when a case
//!   does not translate, the renderer refuses it, or a condition reads as
//!   incomplete (a term the translation does not map).
//! - Reference policies ([`sources`]): the examples of the W3C ODRL
//!   Information Model, the EDC samples, the Dataspace Protocol examples.
//!   They are shown as written; what the renderer cannot read in them is a
//!   finding, listed on their page, not a build failure.
//!
//! A constraint a policy refers to and does not declare reads as defined at
//! its IRI; the page links the IRI when it answers HTTP 200 ([`links`]).
//!
//! [ds-odrl-compliance-rdf]: https://github.com/ds-labs-org/ds-odrl-compliance-rdf
pub mod features;
pub mod links;
pub mod sources;
pub mod translate;

use prose_yew::OdrlProse;
use prose_yew::prose_core::{Condition, Rule};
use sources::Found;
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;
use yew::AttrValue;

/// One policy document as the catalog shows it.
pub struct Entry {
    pub slug: String,
    pub title: String,
    pub about: String,
    pub link: String,
    pub json: String,
    pub html: String,
    pub warnings: Vec<String>,
    /// A condition reads as "an incomplete constraint".
    pub incomplete: bool,
    pub features: BTreeSet<&'static str>,
}

/// One source of policies, with its own index page.
pub struct Source {
    /// The directory and URL segment of its pages.
    pub id: &'static str,
    pub name: &'static str,
    /// Markdown: what the source is and how its policies were read.
    pub intro: String,
    pub license: &'static str,
    pub entries: Vec<Entry>,
}

/// Renders one policy document and checks its reading. `live` says whether
/// the IRI of a constraint the policy refers to, and does not declare,
/// answers HTTP 200 ([`links::answers_200`]); those IRIs are links.
pub fn entry(found: Found, live: &mut dyn FnMut(&str) -> bool) -> Result<Entry, String> {
    let json = serde_json::to_string_pretty(&found.policy).expect("JSON") + "\n";
    let doc = prose_yew::prose_core::render(&json).map_err(|e| format!("{}: {e:?}", found.slug))?;
    let mut referred = BTreeSet::new();
    for p in &doc.policies {
        references(&p.rules, &mut referred);
    }
    let links = referred.into_iter().filter(|iri| live(iri)).collect();
    let html = without_markers(&ssr(&json, links));
    Ok(Entry {
        incomplete: html.contains("incomplete constraint"),
        features: features::of(&found.policy),
        slug: found.slug,
        title: found.title,
        about: found.about,
        link: found.link,
        json,
        html,
        warnings: doc.warnings,
    })
}

/// The IRIs of the constraints `rules` refer to and the document does not
/// declare.
fn references(rules: &[Rule], out: &mut BTreeSet<String>) {
    fn walk(c: &[Condition], out: &mut BTreeSet<String>) {
        for c in c {
            out.extend(c.reference.clone());
            walk(&c.children, out);
        }
    }
    for r in rules {
        out.extend(r.references.iter().cloned());
        walk(&r.refinements, out);
        walk(&r.conditions, out);
        for f in &r.follow_ups {
            references(&f.rules, out);
        }
    }
}

fn ssr(json: &str, links: BTreeSet<String>) -> String {
    let props = yew::props!(prose_yew::OdrlProseProps {
        json: AttrValue::from(json.to_string()),
        links: Rc::new(links),
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(
        &rt,
        yew::LocalServerRenderer::<OdrlProse>::with_props(props).render(),
    )
}

/// The renderer's output as the site embeds it: without Yew's markers.
fn without_markers(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(at) = rest.find("<!--") {
        out.push_str(&rest[..at]);
        let end = rest[at..].find("-->").map_or(rest.len(), |e| e + at + 3);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The ds-odrl-compliance-rdf corpus: every `*.ttl` in `cases_dir`, in name
/// order. `link` is where the case files are linked under.
pub fn compliance(cases_dir: &Path, link: &str) -> Result<Source, String> {
    let mut names: Vec<String> = std::fs::read_dir(cases_dir)
        .map_err(|e| format!("{}: {e}", cases_dir.display()))?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".ttl"))
        .collect();
    names.sort();
    if names.is_empty() {
        return Err(format!("{}: no .ttl case", cases_dir.display()));
    }
    let mut entries = Vec::new();
    for n in &names {
        let text = std::fs::read_to_string(cases_dir.join(n)).map_err(|e| format!("{n}: {e}"))?;
        let case = translate::translate(n.trim_end_matches(".ttl"), &text)?;
        // The translation writes every constraint inline: nothing to link.
        let e = entry(
            Found {
                link: format!("{link}/{}.ttl", case.slug),
                title: label(&case.slug),
                about: case.title,
                slug: case.slug,
                policy: case.policies,
            },
            &mut |_| false,
        )?;
        if e.incomplete {
            return Err(format!(
                "{}: a constraint reads as incomplete; the translation misses a term",
                e.slug
            ));
        }
        entries.push(e);
    }
    Ok(Source {
        id: "compliance",
        name: "Compliance corpus",
        intro: "[ds-odrl-compliance-rdf](https://github.com/ds-labs-org/ds-odrl-compliance-rdf) \
is a corpus of ODRL test cases for policy engines, written in Turtle. Each case pairs a request \
with the compliance report an engine must produce. The pages show the policies of each case, \
translated to ODRL JSON-LD. The claims, the profile and the expected report are left out: they \
are inputs of an engine, not of a reader.\n\n\
Some terms of the corpus are not ODRL. A claim key (`dsc:ClaimKey`) is a blank node in Turtle. \
The translation gives it an IRI of the case, so it reads as \"the claim\" followed by its key. \
Inheritance (`dsc:inheritsFrom`) becomes ODRL's `inheritFrom`.\n"
            .into(),
        license: "Apache-2.0",
        entries,
    })
}

/// Renders the policies of a reference source. A document the renderer
/// refuses is reported on stderr and left out. `live` is as for [`entry`].
pub fn reference(
    id: &'static str,
    name: &'static str,
    intro: &str,
    license: &'static str,
    found: Vec<Found>,
    live: &mut dyn FnMut(&str) -> bool,
) -> Source {
    let entries = found
        .into_iter()
        .filter_map(|f| {
            entry(f, live)
                .map_err(|e| eprintln!("warning: {id}: left out: {e}"))
                .ok()
        })
        .collect();
    Source {
        id,
        name,
        intro: intro.to_string(),
        license,
        entries,
    }
}

/// "xone-exactly-one-factor-01" reads as "Xone exactly one factor 01".
fn label(slug: &str) -> String {
    let text = slug.replace('-', " ");
    let mut c = text.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// One line, for front matter and table cells.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cell(text: &str) -> String {
    one_line(text).replace('|', "/")
}

fn reading(e: &Entry) -> String {
    match (e.incomplete, e.warnings.len()) {
        (true, _) => "incomplete condition".into(),
        (false, 0) => "complete".into(),
        (false, 1) => "1 warning".into(),
        (false, n) => format!("{n} warnings"),
    }
}

/// Writes the catalog under `out_dir`, which is emptied first.
pub fn write_site(sources: &[Source], out_dir: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", out_dir.display());
    if out_dir.exists() {
        std::fs::remove_dir_all(out_dir).map_err(io)?;
    }
    for (i, s) in sources.iter().enumerate() {
        let dir = out_dir.join(s.id);
        std::fs::create_dir_all(&dir).map_err(io)?;
        for e in &s.entries {
            std::fs::write(dir.join(format!("{}.jsonld", e.slug)), &e.json).map_err(io)?;
            std::fs::write(dir.join(format!("{}.html", e.slug)), &e.html).map_err(io)?;
            std::fs::write(dir.join(format!("{}.md", e.slug)), page(s, e)).map_err(io)?;
        }
        std::fs::write(dir.join("index.md"), source_index(i + 1, s)).map_err(io)?;
    }
    std::fs::write(out_dir.join("index.md"), overview(sources)).map_err(io)
}

fn page(s: &Source, e: &Entry) -> String {
    let (id, slug, title) = (s.id, &e.slug, one_line(&e.title));
    let desc = one_line(&format!(
        "{} policy {title}, read as prose. {}",
        s.name, e.about
    ));
    let mut notes = String::new();
    if e.incomplete {
        notes.push_str("A condition of this policy reads as \"an incomplete constraint\": the renderer could not read it.\n\n");
    }
    if e.warnings.is_empty() {
        notes.push_str("The renderer raised no warning for this policy.\n");
    } else {
        let rows: String = e
            .warnings
            .iter()
            .map(|w| format!("| {} |\n", cell(w)))
            .collect();
        notes.push_str(&format!(
            "The renderer raised these warnings.\n\n| Warning |\n|---|\n{rows}"
        ));
    }
    let about = if e.about.is_empty() {
        "No description.".into()
    } else {
        cell(&e.about)
    };
    format!(
        "---
title: {title}
description: {desc}
nav: catalog
subnav: no
stylesheet: examples/prose.css
---
# {title}

A policy from the {name}. The table gives the source and what the policy shows, in the source's words.

| Source | What it shows |
|---|---|
| [{link_text}]({link}) | {about} |

## Policy

{{{{file catalog/{id}/{slug}.jsonld}}}}

## Reading

The sentences `OdrlProse` writes for this policy, without a profile.

{{{{rendered catalog/{id}/{slug}.html}}}}

{notes}
Back to the [{name}](/catalog/{id}/) or the [catalog](/catalog/).
",
        name = s.name,
        link = e.link,
        link_text = cell(e.link.rsplit('/').next().unwrap_or(&e.link)),
    )
}

fn source_index(order: usize, s: &Source) -> String {
    let rows: String = s
        .entries
        .iter()
        .map(|e| {
            format!(
                "| [{}](/catalog/{}/{}/) | {} | {} |\n",
                cell(&e.title),
                s.id,
                e.slug,
                cell(&e.about),
                reading(e)
            )
        })
        .collect();
    format!(
        "---
title: {name}
label: {name}
description: The policies of the {name}, as JSON-LD and as the sentences the renderer writes for them.
nav: catalog
order: {order}
stylesheet: examples/prose.css
---
# {name}

{intro}
The Reading column says whether every condition of a policy reads, and how many warnings the renderer raised. License of the source: {license}.

| Policy | What it shows | Reading |
|---|---|---|
{rows}",
        name = s.name,
        intro = s.intro,
        license = s.license,
    )
}

fn overview(sources: &[Source]) -> String {
    let mut src_rows = String::new();
    for s in sources {
        let n = s.entries.len();
        let complete = s
            .entries
            .iter()
            .filter(|e| !e.incomplete && e.warnings.is_empty())
            .count();
        let incomplete = s.entries.iter().filter(|e| e.incomplete).count();
        src_rows.push_str(&format!(
            "| [{}](/catalog/{}/) | {n} | {complete} | {} | {incomplete} | {} |\n",
            s.name,
            s.id,
            n - complete - incomplete,
            s.license
        ));
    }
    let head: String = sources.iter().map(|s| format!(" {} |", s.name)).collect();
    let rule: String = sources.iter().map(|_| "---|").collect();
    let mut cov = String::new();
    let mut gaps = String::new();
    for (group, f) in features::FEATURES {
        let counts: Vec<usize> = sources
            .iter()
            .map(|s| s.entries.iter().filter(|e| e.features.contains(f)).count())
            .collect();
        let cells: String = counts.iter().map(|c| format!(" {c} |")).collect();
        cov.push_str(&format!("| {group} | `{f}` |{cells}\n"));
        let in_corpus = sources
            .iter()
            .zip(&counts)
            .any(|(s, c)| s.id == "compliance" && *c > 0);
        let users: Vec<&str> = sources
            .iter()
            .zip(&counts)
            .filter(|(s, c)| s.id != "compliance" && **c > 0)
            .map(|(s, _)| s.name)
            .collect();
        if !in_corpus && !users.is_empty() {
            gaps.push_str(&format!("| `{f}` | {} |\n", users.join(", ")));
        }
    }
    let gaps = if gaps.is_empty() {
        "Every feature the reference policies use is also used by a case of the compliance corpus.\n".to_string()
    } else {
        format!(
            "These features are used by reference policies and by no case of the compliance corpus. The corpus is measured on its policies as translated: an asset or party it describes outside a policy is not counted.\n\n| Feature | Used by |\n|---|---|\n{gaps}"
        )
    };
    format!(
        "---
title: Policy catalog
label: Overview
description: Real-world ODRL policies, from the ds-odrl-compliance-rdf corpus and from W3C, EDC and the Dataspace Protocol, as JSON-LD and as the sentences the renderer writes for them.
nav: catalog
order: 0
---
# Policy catalog

Each page of the catalog shows one ODRL policy and the sentences ds-prose-rs writes for it. The policies come from a test corpus for policy engines and from published references. None of them is kept in this repository. The pages are generated from the sources when the site is built.

## Sources

A policy reads completely when every condition reads and the renderer raises no warning.

| Source | Policies | Complete | With warnings | Incomplete condition | License |
|---|---|---|---|---|---|
{src_rows}
## Coverage

The number of policies of each source that use a part of ODRL.

| Group | Feature |{head}
|---|---|{rule}
{cov}
## Gaps in the compliance corpus

{gaps}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A case in the corpus's shape, with the terms the translation maps.
    const CASE: &str = r#"
@base <https://ds-labs-org.github.io/ds-odrl-compliance-rdf/cases/sample-01> .
@prefix :      <#> .
@prefix dsc:   <https://ds-labs-org.github.io/ds-odrl-compliance-rdf/ns#> .
@prefix odrl:  <http://www.w3.org/ns/odrl/2/> .
@prefix dct:   <http://purl.org/dc/terms/> .
@prefix rdf:   <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

:testcase a dsc:TestCase ; dct:title "A sample" ; dsc:request :request .

:perm-print a odrl:Permission ;
    odrl:target :asset ;
    odrl:action [ rdf:value odrl:print ;
        odrl:refinement [ a odrl:Constraint ; odrl:leftOperand odrl:count ;
                          odrl:operator odrl:lteq ; odrl:rightOperand 2 ] ] ;
    odrl:constraint [ a odrl:LogicalConstraint ; odrl:xone (
        [ a odrl:Constraint ; odrl:leftOperand [ a dsc:ClaimKey ; dsc:key "mfaVerified" ] ;
          odrl:operator odrl:eq ; odrl:rightOperand "true" ]
        [ a odrl:Constraint ; odrl:leftOperand odrl:recipient ;
          odrl:operator odrl:isAnyOf ; odrl:rightOperand :alice, :bob ] ) ] ;
    odrl:duty :duty-inform .

:duty-inform a odrl:Duty ; odrl:action odrl:inform ; odrl:target :log .

:parent a odrl:Set ; odrl:assigner <did:web:provider.example> ;
    odrl:permission :perm-print ; odrl:conflict odrl:perm .
:child a odrl:Agreement ; dsc:inheritsFrom ( :parent ) ; odrl:assignee :alice ;
    odrl:prohibition [ a odrl:Prohibition ; odrl:target :asset ; odrl:action odrl:print ] .

:request a dsc:Request ; dsc:policy :child .
"#;

    fn sample() -> translate::Case {
        translate::translate("sample-01", CASE).unwrap()
    }

    #[test]
    fn the_request_policy_comes_first_then_its_parent() {
        let p = sample().policies;
        let graph = p["@graph"].as_array().unwrap();
        assert_eq!(graph[0]["uid"], "case:child");
        assert_eq!(graph[0]["@type"], "Agreement");
        assert_eq!(graph[0]["inheritFrom"], "case:parent");
        assert_eq!(graph[1]["uid"], "case:parent");
        assert_eq!(graph[1]["conflict"], "perm");
        assert_eq!(
            p["@context"][1]["case"],
            "https://ds-labs-org.github.io/ds-odrl-compliance-rdf/cases/sample-01#"
        );
    }

    #[test]
    fn rules_keep_refinements_logic_duties_and_claim_keys() {
        let p = sample().policies;
        let perm = &p["@graph"][1]["permission"][0];
        assert_eq!(perm["action"]["rdf:value"]["@id"], "odrl:print");
        assert_eq!(perm["action"]["refinement"][0]["rightOperand"], 2);
        let xone = perm["constraint"][0]["xone"].as_array().unwrap();
        assert_eq!(xone[0]["leftOperand"]["@id"], "case:claim-mfaVerified");
        assert_eq!(xone[0]["leftOperand"]["dsc:key"], "mfaVerified");
        assert_eq!(xone[1]["rightOperand"][1]["@id"], "case:bob");
        assert_eq!(perm["duty"][0]["action"], "inform");
    }

    #[test]
    fn a_case_without_a_policy_is_an_error() {
        let ttl = CASE.replace(":request a dsc:Request ; dsc:policy :child .", "");
        assert!(translate::translate("sample-01", &ttl).is_err());
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("compliance-catalog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_corpus_reads_as_complete_sentences() {
        let cases = temp("cases");
        std::fs::write(cases.join("sample-01.ttl"), CASE).unwrap();
        let s = compliance(&cases, "https://example.org/cases").unwrap();
        let e = &s.entries[0];
        assert!(
            e.html.contains("the claim mfa verified is equal to"),
            "{}",
            e.html
        );
        assert!(!e.html.contains("<!--"));
        assert!(e.warnings.is_empty(), "{:?}", e.warnings);
        assert_eq!(e.link, "https://example.org/cases/sample-01.ttl");
        assert!(e.features.contains("xone") && e.features.contains("inheritFrom"));
        std::fs::remove_dir_all(cases).unwrap();
    }

    #[test]
    fn a_corpus_condition_that_reads_as_incomplete_fails_the_build() {
        let cases = temp("incomplete");
        let ttl = CASE.replace("dsc:key \"mfaVerified\"", "dsc:other \"x\"");
        std::fs::write(cases.join("sample-01.ttl"), ttl).unwrap();
        let err = compliance(&cases, "https://example.org/cases")
            .err()
            .unwrap();
        assert!(err.contains("incomplete"), "{err}");
        std::fs::remove_dir_all(cases).unwrap();
    }

    #[test]
    fn a_reference_keeps_an_incomplete_reading_as_a_finding() {
        let found = sources::Found {
            slug: "eg1".into(),
            title: "A constraint without an operator (eg1)".into(),
            about: String::new(),
            link: "https://w3c.github.io/poe/model/#x".into(),
            policy: serde_json::json!({ "@type": "Set", "uid": "http://x/p",
                "permission": [{ "target": "http://x/a", "action": "use",
                                 "constraint": [{ "leftOperand": "count" }] }] }),
        };
        let s = reference(
            "w3c",
            "W3C ODRL examples",
            "",
            "W3C",
            vec![found],
            &mut |_| false,
        );
        assert!(s.entries[0].incomplete);
    }

    #[test]
    fn the_site_has_an_overview_and_per_source_an_index_and_three_files_per_policy() {
        let cases = temp("site-cases");
        std::fs::write(cases.join("sample-01.ttl"), CASE).unwrap();
        let s = compliance(&cases, "https://example.org/cases").unwrap();
        let dir = temp("site");
        write_site(&[s], &dir).unwrap();
        let mut names: Vec<_> = std::fs::read_dir(dir.join("compliance"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "index.md",
                "sample-01.html",
                "sample-01.jsonld",
                "sample-01.md"
            ]
        );
        let page = std::fs::read_to_string(dir.join("compliance/sample-01.md")).unwrap();
        assert!(page.contains("{{file catalog/compliance/sample-01.jsonld}}"));
        assert!(page.contains("{{rendered catalog/compliance/sample-01.html}}"));
        assert!(page.contains("\nsubnav: no\n"));
        let overview = std::fs::read_to_string(dir.join("index.md")).unwrap();
        assert!(overview.contains("| `xone` | 1 |"), "{overview}");
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::remove_dir_all(cases).unwrap();
    }
}
