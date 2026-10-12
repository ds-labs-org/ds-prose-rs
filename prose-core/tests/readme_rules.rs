//! The rule table in README.md ("Strict validation, Rules") against the
//! catalogue `RULES`: a rule id, its levels, its basis, whether an editor may
//! refuse it and whether it is a gap. The README is what a reader trusts, so
//! the table is checked, not reviewed by eye.
use prose_core::validate::{Basis, Level, RULES};

fn readme() -> &'static str {
    include_str!("../../README.md")
}

fn level(l: Level) -> &'static str {
    match l {
        Level::Off => "off",
        Level::Info => "info",
        Level::Warning => "warning",
        Level::Error => "error",
    }
}

fn basis(b: Basis) -> &'static str {
    match b {
        Basis::Standard => "standard",
        Basis::StandardDerived => "standard-derived",
        Basis::Convention => "convention",
        Basis::Heuristic => "heuristic",
        Basis::Input => "input",
        _ => "?",
    }
}

/// `(id, what, levels, basis, refusable)` for each row of the table.
fn rows() -> Vec<[String; 5]> {
    readme()
        .lines()
        .filter(|l| l.starts_with("| <a id=\"rule-"))
        .map(|l| {
            let cells: Vec<&str> = l.trim_matches('|').split(" | ").collect();
            assert_eq!(cells.len(), 5, "{l}");
            let id = cells[0].rsplit('`').nth(1).unwrap().to_string();
            [
                id,
                cells[1].trim().to_string(),
                cells[2].trim().to_string(),
                cells[3].trim().to_string(),
                cells[4].trim().to_string(),
            ]
        })
        .collect()
}

#[test]
fn the_readme_table_lists_every_rule_once_in_catalogue_order() {
    let ids: Vec<String> = rows().into_iter().map(|r| r[0].clone()).collect();
    let want: Vec<String> = RULES.iter().map(|r| r.id.as_str().to_string()).collect();
    assert_eq!(ids, want);
}

#[test]
fn the_readme_table_states_each_rules_levels_basis_and_refusal() {
    for (row, info) in rows().iter().zip(RULES) {
        let id = &row[0];
        let levels = match info.strict_closed {
            Some(c) => format!(
                "{}, {}; closed: {}",
                level(info.advisory),
                level(info.strict),
                level(c)
            ),
            None => format!("{}, {}", level(info.advisory), level(info.strict)),
        };
        assert_eq!(row[2], levels, "{id}: levels");
        assert_eq!(row[3], basis(info.basis), "{id}: basis");
        let refusable = row[4].starts_with("yes");
        let closed_only = row[4].starts_with("closed only");
        assert_eq!(
            refusable || closed_only,
            info.refusable,
            "{id}: refusable ({})",
            row[4]
        );
        assert_eq!(
            closed_only,
            info.refusable && info.strict_closed.is_some() && info.strict != Level::Error,
            "{id}: closed only"
        );
        assert_eq!(row[1].contains("a gap"), info.gap, "{id}: gap ({})", row[1]);
    }
}

/// The CLI options that the script refuses without `--strict`
/// (scripts/ds-prose-validate.mjs, "needs --strict"): the README must say so,
/// or a reader's first CI run exits 2.
#[test]
fn the_readme_says_which_cli_options_need_strict() {
    let script = include_str!("../../scripts/ds-prose-validate.mjs");
    let readme = readme();
    let at = readme
        .find("Options: `--profile`")
        .expect("the options paragraph");
    let para = &readme[at..readme[at..].find("\n\n").map_or(readme.len(), |e| at + e)];
    let flat = para.split_whitespace().collect::<Vec<_>>().join(" ");
    for flag in [
        "--structural",
        "--fail-closed",
        "--deny-deprecated",
        "--odrl",
    ] {
        assert!(
            script.contains(&format!("['{flag}',")),
            "{flag} no longer needs --strict"
        );
    }
    assert!(
        flat.contains(
            "`--structural`, `--fail-closed`, `--deny-deprecated` and `--odrl` need `--strict`"
        ),
        "README does not say which options need --strict: {flat}"
    );
}
