//! What README.md says about editing under strict validation (UX-9), against
//! the code: the claims a host acts on are pinned here, so the README cannot
//! drift from what the editor does.
#![cfg(not(target_arch = "wasm32"))]
use prose_core::edit::{EditDoc, EditEvent, EditRules, ListPath, NewItem, read_model};
use prose_core::profile::{ProfileVocab, Wording};
use prose_core::validate::{Options, Validator};
use prose_yew::EditConfig;
use std::rc::Rc;
use std::sync::Arc;

const README: &str = include_str!("../../README.md");

/// The text under the heading `# {title}` up to the next heading of that depth.
fn section(heading: &str) -> &'static str {
    let at = README
        .find(&format!("\n{heading}\n"))
        .unwrap_or_else(|| panic!("no section {heading}"));
    let rest = &README[at + heading.len() + 2..];
    let depth = heading.split(' ').next().unwrap();
    let end = rest.find(&format!("\n{depth} ")).unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn the_editor_section_does_not_promise_that_nothing_typed_is_lost() {
    let s = section("### In the editor");
    for claim in [
        "never loses typed text",
        "every edit commits",
        "trapped by partial input",
    ] {
        assert!(!s.contains(claim), "still says {claim:?}");
    }
    // What the closed comboboxes do, in the README's words.
    for fact in ["closed combobox", "Enter", "Escape", "given up"] {
        assert!(s.contains(fact), "does not say {fact:?}");
    }
}

#[test]
fn the_editor_section_tells_a_host_to_hand_the_reducer_the_configs_rules() {
    let s = section("### In the editor");
    assert!(s.contains("cfg.rules"), "no mention of cfg.rules");
    assert!(s.contains("use_prose_editor"), "no mention of the hook");
}

/// Every field the strict work added to `Labels`, as `config.rs` has it.
const NEW_LABELS: [&str; 14] = [
    "not_in_profile",
    "kept_previous",
    "kept_empty",
    "allowed_group",
    "not_allowed_group",
    "not_allowed_option",
    "not_allowed_term",
    "did_you_mean",
    "problems_heading",
    "go_to",
    "status_conforms",
    "status_violations",
    "status_incomplete",
    "counts",
];

#[test]
fn the_new_labels_are_real_fields_and_all_are_listed_in_the_readme() {
    let config = include_str!("../src/edit/config.rs");
    let changed = section("Changed in 0.4 (profiles; additive except for struct literals):");
    for name in NEW_LABELS {
        assert!(
            config.contains(&format!("pub {name}: AttrValue")),
            "{name} is not a Labels field"
        );
        assert!(
            changed.contains(&format!("`{name}`")),
            "README omits {name}"
        );
    }
}

#[test]
fn the_struct_literal_breaks_are_all_listed() {
    let changed = section("Changed in 0.4 (profiles; additive except for struct literals):");
    assert!(changed.contains("`OdrlProseViewProps` gained `refusal`"));
    assert!(changed.contains("refusal: None"));
    assert!(changed.contains("validator: None, violations: false"));
}

fn research_closed() -> Wording {
    let p = ProfileVocab::parse(include_str!(
        "../../prose-core/tests/profiles/research-closed.jsonld"
    ))
    .unwrap();
    Wording::new().with_profile(p)
}

/// The README's claim that the reducer reads the rules it is given: a host
/// that keeps its own `EditRules` gets the defaults it set, not the ones
/// `with_strict` made admissible.
#[test]
fn the_reducer_adds_with_the_rules_it_is_given() {
    let mut host = EditRules::default();
    host.defaults.action = "https://example.net/foo/bar".into();
    let mut cfg = EditConfig::default().with_wording(Rc::new(research_closed()));
    cfg.rules = host.clone();
    let cfg = cfg.with_strict(Options::strict());
    assert_ne!(cfg.rules.defaults.action, host.defaults.action);

    let validator = Validator::new(Arc::new(research_closed()), Options::strict());
    let base = read_model(include_str!("../../prose-core/tests/strict/clean.json")).unwrap();
    let add = |doc: &EditDoc, rules: &EditRules| {
        let list: ListPath = "policy[0]@permission".parse().unwrap();
        let ev = EditEvent::Add {
            expect: doc.list_owner_id(&list),
            list,
            index: 1,
            item: NewItem::Default,
        };
        let mut doc = doc.clone();
        doc.apply(&ev, rules).unwrap();
        validator.validate(&doc).verdict
    };
    assert_ne!(add(&base, &host), add(&base, &cfg.rules));
    assert_eq!(
        add(&base, &cfg.rules),
        validator.validate(&base).verdict,
        "an add with cfg.rules leaves the verdict as it was"
    );
}

// The README's example, as it is printed there (In the editor), compiled and
// rendered once: `wording` and `POLICY` are the host's.
const POLICY: &str = include_str!("../../prose-core/tests/strict/clean.json");

#[yew::function_component(ReadmeHost)]
fn readme_host() -> yew::Html {
    use prose_yew::{EditMode, OdrlProseView, StrictEdit, use_prose_editor_with};
    use yew::prelude::*;
    let wording = Rc::new(research_closed());
    let cfg = Rc::new(
        EditConfig::default()
            .with_wording(wording)
            .with_strict(Options::strict()),
    );
    let editor = use_prose_editor_with(
        || read_model(POLICY).unwrap_or_default(),
        Rc::new(cfg.rules.clone()),
        cfg.validator.clone(),
        StrictEdit::Flag,
    );
    html! {
        <OdrlProseView doc={editor.editor.doc.clone()} mode={EditMode::Edit} config={cfg}
                       onedit={editor.editor.onedit.clone()}
                       refusal={editor.refused.as_ref().and_then(|(ev, err)| err.issue(ev))} />
    }
}

#[tokio::test]
async fn the_readme_host_compiles_and_renders() {
    let local = tokio::task::LocalSet::new();
    let html = local
        .run_until(yew::LocalServerRenderer::<ReadmeHost>::new().render())
        .await;
    assert!(html.contains("prose"), "{html}");
}

#[test]
fn the_readme_prints_the_example_this_test_compiles() {
    let s = section("### In the editor");
    for line in [
        "Rc::new(cfg.rules.clone()),",
        "cfg.validator.clone(),",
        "StrictEdit::Flag,",
        "refusal={editor.refused.as_ref().and_then(|(ev, err)| err.issue(ev))} />",
    ] {
        assert!(s.contains(line), "README lacks {line:?}");
    }
}

/// Every line of the README's example is in the component this file compiles
/// (a changed line there that was not changed here would print code nobody
/// compiled).
#[test]
fn every_line_of_the_readme_example_is_compiled_here() {
    let s = section("### In the editor");
    let at = s.find("```rust").expect("the example");
    let body = &s[at + 7..];
    let block = &body[..body.find("```").unwrap()];
    // Layout and trailing commas are rustfmt's, not the example's.
    let squash = |t: &str| {
        t.chars()
            .filter(|c| !c.is_whitespace() && *c != ',')
            .collect::<String>()
    };
    let this = squash(include_str!("readme_strict.rs"));
    for line in block.lines().map(str::trim).filter(|l| !l.is_empty()) {
        assert!(
            this.contains(&squash(line)),
            "README example line not compiled: {line:?}"
        );
    }
}
