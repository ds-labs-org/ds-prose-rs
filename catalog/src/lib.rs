//! The site's catalog of the [ds-odrl-compliance-rdf] corpus.
//!
//! The corpus is not copied into this repository: CI checks it out at build
//! time and runs `compliance-catalog <cases-dir> site/content/catalog`. Each
//! case's policies are translated from Turtle to ODRL JSON-LD
//! ([`translate`]), rendered by `OdrlProse`, and written as one site page
//! with its fragments; an index page lists the cases.
//!
//! The build fails when a case does not translate, when the renderer refuses
//! its policies, or when a sentence holds "an incomplete constraint" (a term
//! of the corpus the translation does not map). Renderer warnings do not
//! fail it: they are listed on the case's page.
//!
//! [ds-odrl-compliance-rdf]: https://github.com/ds-labs-org/ds-odrl-compliance-rdf
pub mod translate;

use prose_yew::OdrlProse;
use std::path::Path;
use translate::Case;
use yew::AttrValue;

/// Where a case's Turtle file is linked from, without the trailing `/`.
pub const DEFAULT_SOURCE: &str =
    "https://github.com/ds-labs-org/ds-odrl-compliance-rdf/blob/main/cases";

/// One case as the catalog shows it.
pub struct Entry {
    pub case: Case,
    pub json: String,
    pub html: String,
    pub warnings: Vec<String>,
}

/// Reads every `*.ttl` in `cases_dir`, in name order, and reads it as prose.
pub fn read_corpus(cases_dir: &Path) -> Result<Vec<Entry>, String> {
    let mut names: Vec<String> = std::fs::read_dir(cases_dir)
        .map_err(|e| format!("{}: {e}", cases_dir.display()))?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".ttl"))
        .collect();
    names.sort();
    if names.is_empty() {
        return Err(format!("{}: no .ttl case", cases_dir.display()));
    }
    names
        .iter()
        .map(|n| {
            let text =
                std::fs::read_to_string(cases_dir.join(n)).map_err(|e| format!("{n}: {e}"))?;
            entry(translate::translate(n.trim_end_matches(".ttl"), &text)?)
        })
        .collect()
}

/// Renders a translated case and checks its reading.
pub fn entry(case: Case) -> Result<Entry, String> {
    let json = serde_json::to_string_pretty(&case.policies).expect("JSON") + "\n";
    let doc = prose_yew::prose_core::render(&json).map_err(|e| format!("{}: {e:?}", case.slug))?;
    let html = without_markers(&ssr(&json));
    if html.contains("incomplete constraint") {
        return Err(format!(
            "{}: a constraint reads as incomplete; the translation misses a term",
            case.slug
        ));
    }
    Ok(Entry {
        case,
        json,
        html,
        warnings: doc.warnings,
    })
}

fn ssr(json: &str) -> String {
    let props = yew::props!(prose_yew::OdrlProseProps {
        json: AttrValue::from(json.to_string()),
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

/// Writes the catalog: `index.md`, and per case `<slug>.md`, `.jsonld` and
/// `.html`. `out_dir` is emptied first.
pub fn write_site(entries: &[Entry], out_dir: &Path, source: &str) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", out_dir.display());
    if out_dir.exists() {
        std::fs::remove_dir_all(out_dir).map_err(io)?;
    }
    std::fs::create_dir_all(out_dir).map_err(io)?;
    for (i, e) in entries.iter().enumerate() {
        let slug = &e.case.slug;
        std::fs::write(out_dir.join(format!("{slug}.jsonld")), &e.json).map_err(io)?;
        std::fs::write(out_dir.join(format!("{slug}.html")), &e.html).map_err(io)?;
        std::fs::write(out_dir.join(format!("{slug}.md")), page(i + 1, e, source)).map_err(io)?;
    }
    std::fs::write(out_dir.join("index.md"), index(entries)).map_err(io)
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

fn page(order: usize, e: &Entry, source: &str) -> String {
    let slug = &e.case.slug;
    let name = label(slug);
    let desc = one_line(&format!(
        "Compliance test case {slug}, read as prose: {}",
        e.case.title
    ));
    let what = cell(&e.case.title);
    let warnings = if e.warnings.is_empty() {
        "The renderer raised no warning for these policies.\n".to_string()
    } else {
        let rows: String = e
            .warnings
            .iter()
            .map(|w| format!("| {} |\n", cell(w)))
            .collect();
        format!(
            "The renderer raised these warnings for these policies.\n\n| Warning |\n|---|\n{rows}"
        )
    };
    format!(
        "---
title: {name}
label: {name}
description: {desc}
nav: catalog
order: {order}
stylesheet: examples/prose.css
---
# {name}

A test case of the ds-odrl-compliance-rdf corpus. The table gives what the case tests, in the words of its authors.

| Case | What it tests |
|---|---|
| [{slug}.ttl]({source}/{slug}.ttl) | {what} |

## Policies

The policies the case's request names, translated from Turtle to ODRL JSON-LD. The claims, the profile and the expected compliance report of the case are left out: they are inputs of an engine, not of a reader.

{{{{file catalog/{slug}.jsonld}}}}

## Reading

The sentences `OdrlProse` writes for these policies, without a profile.

{{{{rendered catalog/{slug}.html}}}}

{warnings}
Back to the [catalog](/catalog/).
"
    )
}

fn index(entries: &[Entry]) -> String {
    let rows: String = entries
        .iter()
        .map(|e| {
            format!(
                "| [{}](/catalog/{}/) | {} |\n",
                label(&e.case.slug),
                e.case.slug,
                cell(&e.case.title)
            )
        })
        .collect();
    format!(
        "---
title: Compliance corpus catalog
label: All cases
description: Every test case of the ds-odrl-compliance-rdf corpus, its policies as ODRL JSON-LD and the sentences the renderer writes for them.
nav: catalog
order: 0
---
# Compliance corpus catalog

[ds-odrl-compliance-rdf](https://github.com/ds-labs-org/ds-odrl-compliance-rdf) is a corpus of ODRL test cases for policy engines, written in Turtle. Each page below shows the policies of one case and the sentences ds-prose-rs writes for them.

The pages are built from the corpus at the time the site is built. The build fails when a case does not translate, when the renderer refuses its policies, or when a condition reads as incomplete.

Some terms of the corpus are not ODRL. A claim key (`dsc:ClaimKey`) is a blank node in Turtle. The translation gives it an IRI of the case, so it reads as \"the claim\" followed by its key. Inheritance (`dsc:inheritsFrom`) becomes ODRL's `inheritFrom`.

| Case | What it tests |
|---|---|
{rows}"
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

    fn sample() -> Case {
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
    fn the_sample_reads_as_complete_sentences() {
        let e = entry(sample()).unwrap();
        assert!(
            e.html.contains("the claim mfa verified is equal to"),
            "{}",
            e.html
        );
        assert!(!e.html.contains("<!--"));
        assert!(e.warnings.is_empty(), "{:?}", e.warnings);
    }

    #[test]
    fn a_case_without_a_policy_is_an_error() {
        let ttl = CASE.replace(":request a dsc:Request ; dsc:policy :child .", "");
        assert!(translate::translate("sample-01", &ttl).is_err());
    }

    #[test]
    fn the_site_has_an_index_and_three_files_per_case() {
        let dir = std::env::temp_dir().join(format!("compliance-catalog-{}", std::process::id()));
        write_site(&[entry(sample()).unwrap()], &dir, DEFAULT_SOURCE).unwrap();
        let mut names: Vec<_> = std::fs::read_dir(&dir)
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
        let page = std::fs::read_to_string(dir.join("sample-01.md")).unwrap();
        assert!(page.contains("{{file catalog/sample-01.jsonld}}"));
        assert!(page.contains("{{rendered catalog/sample-01.html}}"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
