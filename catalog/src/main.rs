//! `compliance-catalog <out-dir> [--compliance <cases-dir> <link>]
//! [--w3c <model.html> <link>] [--edc <dir> <link>] [--dsp <dir> <link>]`:
//! builds the site's catalog from local copies of its sources. Each `link`
//! is where that source's files are linked under.
use compliance_catalog::{Source, compliance, reference, sources, write_site};
use std::path::Path;

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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = || -> ! {
        eprintln!(
            "usage: compliance-catalog <out-dir> [--compliance <cases-dir> <link>] \
             [--w3c <model.html> <link>] [--edc <dir> <link>] [--dsp <dir> <link>]"
        );
        std::process::exit(2);
    };
    let Some((out, rest)) = args.split_first() else {
        usage()
    };
    if rest.len() % 3 != 0 {
        usage();
    }
    let run = || -> Result<Vec<Source>, String> {
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
                    )
                }
                "--edc" => reference(
                    "edc",
                    "EDC samples",
                    EDC_INTRO,
                    "Apache-2.0",
                    sources::json_dir(path, link)?,
                ),
                "--dsp" => reference(
                    "dsp",
                    "Dataspace Protocol examples",
                    DSP_INTRO,
                    "Apache-2.0",
                    sources::json_dir(path, link)?,
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
