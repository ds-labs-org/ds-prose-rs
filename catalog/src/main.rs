//! `compliance-catalog <cases-dir> <out-dir> [<source-url>]`: builds the
//! site's catalog from a checkout of ds-odrl-compliance-rdf. `source-url` is
//! where the case files are linked from (default: the corpus's `main`).
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cases, out, rest @ ..] = args.as_slice() else {
        eprintln!("usage: compliance-catalog <cases-dir> <out-dir> [<source-url>]");
        std::process::exit(2);
    };
    let source = rest
        .first()
        .map_or(compliance_catalog::DEFAULT_SOURCE, String::as_str)
        .trim_end_matches('/');
    let run = || -> Result<usize, String> {
        let entries = compliance_catalog::read_corpus(Path::new(cases))?;
        compliance_catalog::write_site(&entries, Path::new(out), source)?;
        for e in &entries {
            for w in &e.warnings {
                println!("warning: {}: {w}", e.case.slug);
            }
        }
        Ok(entries.len())
    };
    match run() {
        Ok(n) => println!("catalog: {n} cases into {out}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
