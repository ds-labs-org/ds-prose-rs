//! Pins what `check_profile` says, the advisory output, over every policy
//! fixture and several loaded wordings. The goldens were written by the
//! advisory checker before strict validation existed; strict mode must not
//! change a byte of them. Written only with `PROSE_BLESS_GOLDEN`, and only
//! when missing, like the v0.1 goldens.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;

#[test]
fn check_profile_matches_advisory_goldens() {
    let mut seen = 0;
    for (name, text) in policy_corpus() {
        let Ok(doc) = read_model(&text) else {
            continue;
        };
        let mut got = String::new();
        for (label, wording) in wordings() {
            got.push_str(&format!("## {label}\n"));
            for i in check_profile(&doc, &wording) {
                got.push_str(&issue_line(&i));
                got.push('\n');
            }
        }
        compare_golden(&format!("advisory/{name}.txt"), &got);
        seen += 1;
    }
    assert!(seen >= 40, "expected the whole corpus, saw {seen}");
}
