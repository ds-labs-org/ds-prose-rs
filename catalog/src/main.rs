//! `compliance-catalog <out-dir> [--terms-timeout <ms>]
//! [--compliance <cases-dir> <link>] [--w3c <model.html> <link>]
//! [--edc <dir> <link>] [--dsp <dir> <link>]`: builds the site's catalog
//! from local copies of its sources. Each `link` is where that source's
//! files are linked under. `--terms-timeout` is how long the check of a URL
//! of accepted terms may take, in milliseconds (200 by default).
use compliance_catalog::{Check, Source, compliance, links, reference, sources, write_site};
use std::path::Path;
use std::time::Duration;

const W3C_INTRO: &str = "The examples of the [ODRL Information Model 2.2](https://www.w3.org/TR/odrl-model/), \
read from the editors' draft. An example written as several JSON documents, a policy and the \
constraints it refers to, is shown as one `@graph`. Examples that are not JSON or not a policy \
are left out.\n";
const EDC_INTRO: &str = "The policies in the JSON files of the [Eclipse EDC samples](https://github.com/eclipse-edc/Samples). \
A policy definition of EDC's management API wraps the ODRL policy in a `policy` member; the page \
shows that policy.\n";
const DSP_INTRO: &str = "The policies in the examples of the [Eclipse Dataspace Protocol](https://github.com/eclipse-dataspace-protocol-base/DataspaceProtocol): \
the offers and agreements inside its catalog and negotiation messages. A policy without a \
`@context` of its own gets the message's.\n";

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let usage = || -> ! {
        eprintln!(
            "usage: compliance-catalog <out-dir> [--terms-timeout <ms>] \
             [--compliance <cases-dir> <link>] [--w3c <model.html> <link>] \
             [--edc <dir> <link>] [--dsp <dir> <link>]"
        );
        std::process::exit(2);
    };
    let mut terms_timeout = links::TERMS_TIMEOUT;
    if let Some(at) = args.iter().position(|a| a == "--terms-timeout") {
        let ms = args.get(at + 1).and_then(|ms| ms.parse().ok());
        let Some(ms) = ms else { usage() };
        terms_timeout = Duration::from_millis(ms);
        args.drain(at..at + 2);
    }
    let Some((out, rest)) = args.split_first() else {
        usage()
    };
    if rest.len() % 3 != 0 {
        usage();
    }
    // A constraint declared outside its policy, or the URL of accepted
    // terms, is linked when it answers 200; each is checked once per build.
    let mut reference_live = links::cached(|iri| links::answers_200(iri, links::TIMEOUT));
    let mut terms_live = links::cached(|url| links::answers_200(url, terms_timeout));
    let mut live = |iri: &str, kind: Check| match kind {
        Check::Reference => reference_live(iri),
        Check::AcceptedTerms => terms_live(iri),
    };
    let mut run = || -> Result<Vec<Source>, String> {
        let mut out = Vec::new();
        for opt in rest.chunks(3) {
            let (path, link) = (Path::new(&opt[1]), opt[2].trim_end_matches('/'));
            out.push(match opt[0].as_str() {
                "--compliance" => compliance(path, link)?,
                "--w3c" => {
                    let html = std::fs::read_to_string(path)
                        .map_err(|e| format!("{}: {e}", path.display()))?;
                    reference(
                        "w3c",
                        "W3C ODRL examples",
                        W3C_INTRO,
                        "W3C Software and Document License",
                        sources::w3c_model(&html, link),
                        &mut live,
                    )
                }
                "--edc" => reference(
                    "edc",
                    "EDC samples",
                    EDC_INTRO,
                    "Apache-2.0",
                    sources::json_dir(path, link)?,
                    &mut live,
                ),
                "--dsp" => reference(
                    "dsp",
                    "Dataspace Protocol examples",
                    DSP_INTRO,
                    "Apache-2.0",
                    sources::json_dir(path, link)?,
                    &mut live,
                ),
                _ => usage(),
            });
        }
        if out.iter().all(|s| s.entries.is_empty()) {
            return Err("no policy read".into());
        }
        Ok(out)
    };
    match run().and_then(|s| write_site(&s, Path::new(out)).map(|()| s)) {
        Ok(sources) => {
            for s in &sources {
                for e in &s.entries {
                    let tag = if e.incomplete {
                        " (incomplete condition)"
                    } else {
                        ""
                    };
                    for w in &e.warnings {
                        println!("warning: {}/{}: {w}", s.id, e.slug);
                    }
                    if e.incomplete {
                        println!("finding: {}/{}{tag}", s.id, e.slug);
                    }
                }
                println!("catalog: {} policies from {}", s.entries.len(), s.name);
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
