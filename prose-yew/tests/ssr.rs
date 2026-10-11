#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::field_reassign_with_default)]
use prose_yew::{OdrlProse, OdrlProseProps};
use yew::prelude::*;

async fn ssr<C: BaseComponent>(props: C::Properties) -> String {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(yew::LocalServerRenderer::<C>::with_props(props).render())
        .await
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../prose-core/tests/fixtures/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[tokio::test]
async fn v01_snapshots() {
    for name in ["offer", "agreement", "set"] {
        let html = ssr::<OdrlProse>(OdrlProseProps {
            json: fixture(name).into(),
            class: Classes::new(),
            wording: None,
            glossary: true,
        })
        .await;
        let path = format!("{}/tests/snapshots/{name}.html", env!("CARGO_MANIFEST_DIR"));
        if std::env::var("WRITE_SNAPSHOTS").is_ok() {
            std::fs::write(&path, &html).unwrap();
        }
        let want = std::fs::read_to_string(&path).unwrap();
        assert_eq!(html, want, "snapshot {name}");
    }
}

// --- the editing view -----------------------------------------------------------

use prose_core::edit::{EditDoc, Issue, IssueTarget, Limit, Severity, SlotPath, read_model};
use prose_yew::{
    Decoration, DecorationAt, EditConfig, EditMode, OdrlProseView, OdrlProseViewProps, Reveal,
};
use std::rc::Rc;

fn doc_of(name: &str) -> Rc<EditDoc> {
    Rc::new(read_model(&fixture(name)).unwrap())
}

fn props(doc: Rc<EditDoc>, mode: EditMode, config: EditConfig) -> OdrlProseViewProps {
    OdrlProseViewProps {
        doc,
        mode,
        config: Rc::new(config),
        onedit: Callback::noop(),
        on_focus_slot: Callback::noop(),
        issues: Rc::new(vec![]),
        decorate: None,
        class: Classes::new(),
    }
}

async fn render_view(p: OdrlProseViewProps) -> String {
    ssr::<OdrlProseView>(p).await
}

fn spans_with<'a>(html: &'a str, needle: &str) -> Vec<&'a str> {
    html.split("<span ")
        .skip(1)
        .filter(|s| s.split('>').next().is_some_and(|tag| tag.contains(needle)))
        .collect()
}

#[tokio::test]
async fn read_mode_has_no_controls() {
    let html = render_view(props(
        doc_of("offer"),
        EditMode::Read,
        EditConfig::default(),
    ))
    .await;
    assert!(html.contains("prose-view"));
    assert!(!html.contains("prose-edit"));
    for bad in ["contenteditable", "<button", "<select", "<style"] {
        assert!(!html.contains(bad), "{bad} in read mode:\n{html}");
    }
    assert!(html.contains("data-node=\"policy[0].permission[0]\""));
    assert!(html.contains("class=\"prose-term\""));
}

#[tokio::test]
async fn edit_mode_slots_are_contenteditable_spans() {
    let html = render_view(props(
        doc_of("offer"),
        EditMode::Edit,
        EditConfig::default(),
    ))
    .await;
    let slots = spans_with(&html, "contenteditable");
    assert!(slots.len() >= 8, "only {} slots:\n{html}", slots.len());
    for s in &slots {
        let tag = s.split('>').next().unwrap();
        for attr in [
            "data-slot=",
            "aria-label=",
            "data-placeholder=",
            "aria-placeholder=",
            "contenteditable=\"plaintext-only\"",
            "spellcheck=\"false\"",
        ] {
            assert!(tag.contains(attr), "{attr} missing in <span {tag}>");
        }
    }
    // Suggestions on (the default): the slot is a combobox, which is where
    // aria-expanded and the popup relationship are defined.
    for s in &slots {
        let tag = s.split('>').next().unwrap();
        assert!(tag.contains("role=\"combobox\""), "<span {tag}>");
        assert!(
            tag.contains("aria-expanded=") && tag.contains("aria-haspopup=\"listbox\""),
            "<span {tag}>"
        );
        assert!(!tag.contains("aria-multiline"), "<span {tag}>");
    }
    assert!(html.contains("data-slot=\"policy[0].permission[0].constraint[0]#leftOperand\""));
    assert!(html.contains("<style>"));
    assert!(html.contains("role=\"status\""));
    assert!(html.contains("<select"));
    assert!(html.contains("data-add=\"policy[0].permission[0]@constraint\""));
    assert!(html.contains("aria-label=\"Add condition of permission 1 of policy 1\""));
    assert!(html.contains("aria-label=\"Remove condition 1 of permission 1 of policy 1\""));
}

#[tokio::test]
async fn config_classes_and_styles_land_next_to_builtins() {
    let mut cfg = EditConfig::default();
    cfg.classes.slot = classes!("my-slot");
    cfg.classes.add = classes!("my-add", "pf-button");
    cfg.classes.rule = classes!("my-rule");
    cfg.classes.root = classes!("my-root");
    cfg.styles.slot = Some("color: red".into());
    cfg.styles.remove = Some("opacity: .5".into());
    cfg.reveal = Reveal::OnHoverOrFocus;
    let html = render_view(props(doc_of("offer"), EditMode::Edit, cfg)).await;
    assert!(html.contains("class=\"prose-slot my-slot"), "{html}");
    assert!(html.contains("prose-add my-add pf-button"));
    assert!(html.contains("prose-rule my-rule"));
    assert!(html.contains("ds-prose prose-view my-root prose-edit prose-reveal-hover"));
    assert!(html.contains("style=\"color: red\""));
    assert!(html.contains("style=\"opacity: .5\""));
}

#[tokio::test]
async fn label_overrides_render() {
    let mut cfg = EditConfig::default();
    cfg.labels.conditions = "Only when".into();
    cfg.labels.add_symbol = "(+)".into();
    cfg.labels.may = "is allowed to".into();
    cfg.labels.remove = "Delete {noun} no. {n}".into();
    cfg.labels.placeholders.left_operand = "what is limited".into();
    let html = render_view(props(doc_of("offer"), EditMode::Edit, cfg.clone())).await;
    assert!(html.contains("Only when:"));
    assert!(html.contains("(+) condition"));
    assert!(html.contains("data-placeholder=\"what is limited\""));
    assert!(html.contains("aria-label=\"Delete condition no. 1 of permission 1 of policy 1\""));
    assert!(html.contains("is allowed to"));
    let read = render_view(props(doc_of("offer"), EditMode::Read, cfg)).await;
    assert!(read.contains("Only when:") && read.contains("is allowed to"));
}

#[tokio::test]
async fn limits_hide_controls() {
    let mut cfg = EditConfig::default();
    cfg.rules.permission = Limit::NONE;
    cfg.rules.constraints = Limit::NONE;
    let html = render_view(props(doc_of("offer"), EditMode::Edit, cfg)).await;
    assert!(
        !html.contains("data-add=\"policy[0]@permission\""),
        "{html}"
    );
    assert!(!html.contains("@constraint\""));
    assert!(html.contains("data-add=\"policy[0]@prohibition\""));
    // One policy, minimum one: it cannot be removed. One action, minimum
    // one: it cannot be removed either.
    assert!(!html.contains("Remove policy 1"));
    assert!(!html.contains("Remove action 1 of permission 1"));
    // A rule can.
    assert!(html.contains("Remove permission 1 of policy 1"));

    let mut cfg = EditConfig::default();
    cfg.base_css = false;
    cfg.rules.allow_reorder = false;
    cfg.rules.allow_logical = false;
    cfg.rules.allow_rule_kind_change = false;
    let html = render_view(props(doc_of("agreement"), EditMode::Edit, cfg)).await;
    assert!(!html.contains("prose-reorder"));
    assert!(!html.contains("prose-wrap"));
    assert!(!html.contains("prose-rule-kind"));
    assert!(!html.contains("condition group"));
}

#[tokio::test]
async fn unknown_operator_is_an_extra_selected_option() {
    let json = r#"{"@type":"Set","permission":[{"action":"use","constraint":[{"leftOperand":"count","operator":"odrl:weird","rightOperand":"1"}]}]}"#;
    let doc = Rc::new(read_model(json).unwrap());
    let html = render_view(props(doc, EditMode::Edit, EditConfig::default())).await;
    assert!(html.contains("odrl:weird (not in the list)"), "{html}");
    let first = html
        .find("<option value=\"odrl:weird\"")
        .expect("extra option");
    let eq = html.find("<option value=\"eq\"").expect("eq option");
    assert!(first < eq, "extra option is first");
    assert!(html[first..eq].contains("selected"));
    assert!(html.contains("is equal to (eq)") || html.contains("(eq)"));
}

#[tokio::test]
async fn error_issue_marks_the_slot_and_shows_its_message() {
    let slot: SlotPath = "policy[0]#uid".parse().unwrap();
    let mut p = props(doc_of("offer"), EditMode::Edit, EditConfig::default());
    p.issues = Rc::new(vec![
        Issue {
            target: IssueTarget::Slot(slot),
            severity: Severity::Error,
            message: "The id is already used".into(),
        },
        Issue {
            target: IssueTarget::Doc,
            severity: Severity::Info,
            message: "Doc level note".into(),
        },
    ]);
    let html = render_view(p).await;
    assert!(html.contains("aria-invalid=\"true\""));
    assert!(html.contains("The id is already used"));
    assert!(html.contains("prose-slot-invalid"));
    assert!(html.contains("prose-issue prose-issue-error"));
    assert!(html.contains("aria-describedby=\"prose-issue-slot:policy[0]#uid-0\""));
    assert!(html.contains("id=\"prose-issue-slot:policy[0]#uid-0\""));
    assert!(html.contains("Doc level note"));
    assert!(html.contains("prose-warnings"));
}

#[tokio::test]
async fn locked_nodes_show_their_json_and_reason() {
    let doc = Rc::new(
        read_model(
            r#"{"@type":"Set","permission":[{"action":"use","constraint":[{"and":[],"or":[]}]}]}"#,
        )
        .unwrap(),
    );
    let html = render_view(props(doc, EditMode::Edit, EditConfig::default())).await;
    assert!(html.contains("<code class=\"prose-locked\""), "{html}");
    assert!(html.contains("&quot;and&quot;") || html.contains("\"and\""));
    assert!(html.contains("Kept exactly as written; not editable here"));
    // A locked node can still be removed.
    assert!(html.contains("Remove condition 1 of permission 1 of policy 1"));
}

#[tokio::test]
async fn base_css_can_be_turned_off() {
    let mut cfg = EditConfig::default();
    cfg.base_css = false;
    let html = render_view(props(doc_of("offer"), EditMode::Edit, cfg)).await;
    assert!(!html.contains("<style"));
}

#[tokio::test]
async fn decorations_follow_rule_sentences() {
    let mut p = props(doc_of("offer"), EditMode::Edit, EditConfig::default());
    p.decorate = Some(Callback::from(|d: Decoration| {
        html! { <b class="badge">{ format!("{:?}:{}", d.at, d.path) }</b> }
    }));
    let html = render_view(p).await;
    let sentence = html.find("prose-rule-sentence").unwrap();
    let badge = html
        .find("Rule:policy[0].permission[0]<")
        .expect("rule badge");
    assert!(badge > sentence);
    assert!(html.contains("Policy:policy[0]<"));
    assert!(html.contains("Condition:policy[0].permission[0].constraint[0]<"));
    let _ = DecorationAt::Rule;
}

#[tokio::test]
async fn empty_documents_say_so() {
    let doc = Rc::new(EditDoc::new(vec![]));
    let html = render_view(props(doc, EditMode::Edit, EditConfig::default())).await;
    assert!(html.contains("No policies yet."));
    assert!(html.contains("data-add=\"policies\""));
}

#[tokio::test]
async fn without_suggestions_a_slot_is_a_plain_textbox() {
    let config = EditConfig {
        suggestions: false,
        ..EditConfig::default()
    };
    let html = render_view(props(doc_of("offer"), EditMode::Edit, config)).await;
    let slots = spans_with(&html, "contenteditable");
    assert!(!slots.is_empty());
    for s in &slots {
        let tag = s.split('>').next().unwrap();
        assert!(
            tag.contains("role=\"textbox\"") && tag.contains("aria-multiline=\"false\""),
            "<span {tag}>"
        );
        assert!(!tag.contains("aria-expanded"), "<span {tag}>");
    }
}

#[tokio::test]
async fn host_state_styles_are_applied_after_the_plain_ones() {
    let slot: SlotPath = "policy[0]#uid".parse().unwrap();
    let mut cfg = EditConfig::default();
    cfg.styles.slot = Some("border-bottom: 1px dashed grey".into());
    cfg.styles.slot_invalid = Some("border-bottom: 2px solid red".into());
    cfg.styles.issue = Some("margin: 0".into());
    cfg.styles.issue_error = Some("color: red".into());
    cfg.styles.rule = Some("padding: 1px".into());
    cfg.styles.permission = Some("border-left: 3px solid green".into());
    cfg.styles.suggestion = Some("cursor: pointer".into());
    let mut p = props(doc_of("offer"), EditMode::Edit, cfg);
    p.issues = Rc::new(vec![Issue {
        target: IssueTarget::Slot(slot),
        severity: Severity::Error,
        message: "bad".into(),
    }]);
    let html = render_view(p).await;
    assert!(
        html.contains("style=\"border-bottom: 1px dashed grey; border-bottom: 2px solid red\""),
        "{html}"
    );
    // Slots without an issue keep only the plain style.
    assert!(html.contains("style=\"border-bottom: 1px dashed grey\""));
    assert!(html.contains("style=\"margin: 0; color: red\""), "{html}");
    assert!(
        html.contains("style=\"padding: 1px; border-left: 3px solid green\""),
        "{html}"
    );
}

#[tokio::test]
async fn text_and_carried_styles_apply_only_when_set() {
    let plain = render_view(props(
        doc_of("offer"),
        EditMode::Edit,
        EditConfig::default(),
    ))
    .await;
    assert!(!plain.contains("prose-text"));
    let mut cfg = EditConfig::default();
    cfg.classes.text = classes!("my-text");
    let html = render_view(props(doc_of("offer"), EditMode::Edit, cfg)).await;
    assert!(html.contains("prose-text my-text"), "{html}");
}

#[tokio::test]
async fn visible_text_of_group_and_move_buttons_is_configurable() {
    let mut cfg = EditConfig::default();
    cfg.labels.wrap_text = "Regrouper".into();
    cfg.labels.unwrap_text = "Degrouper".into();
    cfg.labels.move_up_symbol = "UP".into();
    cfg.labels.move_down_symbol = "DOWN".into();
    let html = render_view(props(doc_of("agreement"), EditMode::Edit, cfg)).await;
    assert!(html.contains(">Regrouper<"), "{html}");
    assert!(!html.contains(">Group<"));
    assert!(html.contains(">DOWN<") || html.contains(">UP<"));
    let default = render_view(props(
        doc_of("agreement"),
        EditMode::Edit,
        EditConfig::default(),
    ))
    .await;
    assert!(default.contains(">Group<"));
}

#[tokio::test]
async fn a_prefixed_operator_flagged_as_an_error_is_shown_as_it_is() {
    let json = r#"{"@type":"Set","permission":[{"action":"use","constraint":[{"leftOperand":"count","operator":"odrl:eq","rightOperand":1}]}]}"#;
    let doc = Rc::new(read_model(json).unwrap());
    // Not flagged: the compact IRI selects the listed option, as before.
    let ok = render_view(props(doc.clone(), EditMode::Edit, EditConfig::default())).await;
    assert!(!ok.contains("<option value=\"odrl:eq\""), "{ok}");
    // Flagged by the host: the raw value is its own selected option, so
    // choosing the listed one afterwards is a change the browser reports.
    let slot: SlotPath = "policy[0].permission[0].constraint[0]#operator"
        .parse()
        .unwrap();
    let mut p = props(doc, EditMode::Edit, EditConfig::default());
    p.issues = Rc::new(vec![Issue {
        target: IssueTarget::Slot(slot),
        severity: Severity::Error,
        message: "the engine rejects this".into(),
    }]);
    let html = render_view(p).await;
    let at = html.find("<option value=\"odrl:eq\"").expect("raw option");
    assert!(
        html[at..].split('>').next().unwrap().contains("selected"),
        "{html}"
    );
}

#[tokio::test]
async fn empty_notes_the_host_cannot_fill_are_not_shown_in_edit_mode() {
    // `agreement` has no policy-level target, action or profile.
    let doc = doc_of("agreement");
    let html = render_view(props(doc.clone(), EditMode::Edit, EditConfig::default())).await;
    assert!(
        html.contains("Applies to "),
        "default rules offer them: {html}"
    );

    let mut cfg = EditConfig::default();
    cfg.rules.policy_target = Limit::NONE;
    cfg.rules.policy_action = Limit::NONE;
    cfg.rules.profile = Limit::NONE;
    let html = render_view(props(doc, EditMode::Edit, cfg)).await;
    assert!(!html.contains("Applies to "), "{html}");
    assert!(!html.contains("Covers the action "), "{html}");
    assert!(!html.contains("Written to the profile "), "{html}");
    // The ones the host still allows stay.
    assert!(html.contains("Inherits the rules of "), "{html}");
}

// --- audit findings -------------------------------------------------------------

fn node_issue(path: &str, message: &str) -> Issue {
    Issue {
        target: IssueTarget::Node(path.parse().unwrap()),
        severity: Severity::Error,
        message: message.into(),
    }
}

#[tokio::test]
async fn issue_on_a_locked_rule_is_rendered() {
    let doc = Rc::new(read_model(r#"{"@type":"Set","permission":[42]}"#).unwrap());
    let mut p = props(doc, EditMode::Edit, EditConfig::default());
    p.issues = Rc::new(vec![node_issue("policy[0].permission[0]", "RULE-ISSUE-X")]);
    let html = render_view(p).await;
    assert!(html.contains("RULE-ISSUE-X"), "{html}");
    assert!(html.contains("prose-issue"), "{html}");
}

#[tokio::test]
async fn issue_on_a_locked_policy_is_rendered() {
    let doc = Rc::new(read_model(r#"[42,{"@type":"Set","uid":"urn:q"}]"#).unwrap());
    let mut p = props(doc, EditMode::Edit, EditConfig::default());
    p.issues = Rc::new(vec![node_issue("policy[0]", "POLICY-ISSUE-X")]);
    let html = render_view(p).await;
    assert!(html.contains("POLICY-ISSUE-X"), "{html}");
}

#[tokio::test]
async fn rule_kind_select_does_not_offer_a_kind_the_rules_forbid() {
    let doc = Rc::new(
        read_model(
            r#"{"@type":"Set","uid":"urn:p","permission":[{"action":"use","target":"urn:a"}]}"#,
        )
        .unwrap(),
    );
    let mut cfg = EditConfig::default();
    cfg.rules.prohibition = Limit::NONE;
    let html = render_view(props(doc.clone(), EditMode::Edit, cfg)).await;
    assert!(
        !html.contains("data-add=\"policy[0]@prohibition\""),
        "{html}"
    );
    assert!(
        !html.contains("<option value=\"prohibition\""),
        "the + is hidden, so the kind is not offered either: {html}"
    );
    assert!(html.contains("<option value=\"permission\""), "{html}");
    assert!(html.contains("<option value=\"obligation\""), "{html}");
    // With the default rules all three are offered.
    let html = render_view(props(doc, EditMode::Edit, EditConfig::default())).await;
    assert!(html.contains("<option value=\"prohibition\""), "{html}");
}

#[tokio::test]
async fn issues_on_action_and_entity_nodes_are_rendered() {
    let doc = Rc::new(
        read_model(r#"{"@type":"Set","permission":[{"action":"use","target":"urn:a"}]}"#).unwrap(),
    );
    for mode in [EditMode::Edit, EditMode::Read] {
        let mut p = props(doc.clone(), mode, EditConfig::default());
        p.issues = Rc::new(vec![
            node_issue("policy[0].permission[0].action[0]", "ACTION-NODE-ISSUE"),
            node_issue("policy[0].permission[0].target[0]", "TARGET-NODE-ISSUE"),
        ]);
        let html = render_view(p).await;
        assert!(html.contains("ACTION-NODE-ISSUE"), "{mode:?}: {html}");
        assert!(html.contains("TARGET-NODE-ISSUE"), "{mode:?}: {html}");
    }
}

#[tokio::test]
async fn doc_issues_are_list_items() {
    let mut p = props(doc_of("offer"), EditMode::Edit, EditConfig::default());
    p.issues = Rc::new(vec![Issue {
        target: IssueTarget::Doc,
        severity: Severity::Info,
        message: "DOC-NOTE".into(),
    }]);
    let html = render_view(p).await;
    let aside = &html[html.find("<aside").expect("the notes block")..];
    let ul = &aside[aside.find("<ul>").unwrap()..aside.find("</ul>").unwrap()];
    assert!(
        ul.starts_with("<ul><li><span id=\"prose-issue-doc-0\""),
        "{ul}"
    );
    assert!(ul.contains("DOC-NOTE"), "{ul}");
}

#[tokio::test]
async fn toolbar_is_outside_the_policy_heading() {
    let mut p = props(doc_of("graph-two"), EditMode::Edit, EditConfig::default());
    p.issues = Rc::new(vec![]);
    let html = render_view(p).await;
    let mut rest = html.as_str();
    let mut n = 0;
    while let Some(at) = rest.find("<h3") {
        let end = rest[at..].find("</h3>").unwrap();
        let heading = &rest[at..at + end];
        assert!(
            !heading.contains("<button"),
            "buttons inside a heading: {heading}"
        );
        n += 1;
        rest = &rest[at + end..];
    }
    assert!(n >= 2, "{html}");
    assert!(
        html.contains("Remove policy 2"),
        "the buttons are still there"
    );
}

#[test]
fn refinement_noun_is_used() {
    use prose_core::edit::{ListKind, Step};
    let mut cfg = EditConfig::default();
    assert_eq!(
        cfg.labels.nouns.list(ListKind::Refinements).as_str(),
        "limit"
    );
    cfg.labels.nouns.refinement = "LIMITNOUN".into();
    assert_eq!(
        cfg.labels.nouns.list(ListKind::Refinements).as_str(),
        "LIMITNOUN"
    );
    assert_eq!(
        cfg.labels.nouns.step(Step::Refinement(0)).as_str(),
        "LIMITNOUN"
    );
    // Constraints and children stay "condition".
    assert_eq!(
        cfg.labels.nouns.list(ListKind::Constraints).as_str(),
        "condition"
    );
    assert_eq!(cfg.labels.nouns.step(Step::Child(0)).as_str(), "condition");
}

// R4: the view labels follow-ups outside ODRL's own positions without
// naming the wrong kind of rule, in Read and Edit mode alike.
#[tokio::test]
async fn follow_up_labels_outside_odrl_positions_do_not_misname_the_rule() {
    let doc = Rc::new(
        read_model(
            r#"{"@type":"Set","permission":[{"action":"use","remedy":[{"action":"delete"}],"consequence":[{"action":"delete"}]}]}"#,
        )
        .unwrap(),
    );
    for mode in [EditMode::Read, EditMode::Edit] {
        let html = render_view(props(doc.clone(), mode, EditConfig::default())).await;
        assert!(!html.contains("if this prohibition is breached"), "{html}");
        // A remedy is a duty, so in Edit mode its own empty consequence slot
        // is correctly labelled; only Read mode has none.
        if mode == EditMode::Read {
            assert!(!html.contains("if this duty is not fulfilled"), "{html}");
        }
        assert!(html.contains("Remedies attached to this rule"), "{html}");
        assert!(
            html.contains("Consequences attached to this rule"),
            "{html}"
        );
    }
    let on_prohibition = Rc::new(
        read_model(
            r#"{"@type":"Set","prohibition":[{"action":"use","remedy":[{"action":"delete"}]}]}"#,
        )
        .unwrap(),
    );
    let html = render_view(props(on_prohibition, EditMode::Read, EditConfig::default())).await;
    assert!(
        html.contains("Remedies if this prohibition is breached"),
        "{html}"
    );
}

// R5: the operator select does not offer a switch the reducer refuses.
#[tokio::test]
async fn operator_select_does_not_offer_a_switch_the_reducer_refuses() {
    let many = Rc::new(
        read_model(
            r#"{"@type":"Set","permission":[{"action":"use","constraint":[{"leftOperand":"count","operator":"isAnyOf","rightOperand":[1,2,3]}]}]}"#,
        )
        .unwrap(),
    );
    let html = render_view(props(many, EditMode::Edit, EditConfig::default())).await;
    assert!(!html.contains("<option value=\"eq\""), "{html}");
    assert!(html.contains("<option value=\"isAllOf\""), "{html}");
    assert!(
        html.contains("<option value=\"isAnyOf\" selected"),
        "{html}"
    );
    // Single-valued, or already several values under a single-value
    // operator: nothing is lost by a switch among single-value operators.
    for json in [
        r#"{"@type":"Set","permission":[{"action":"use","constraint":[{"leftOperand":"count","operator":"isAnyOf","rightOperand":[1]}]}]}"#,
        r#"{"@type":"Set","permission":[{"action":"use","constraint":[{"leftOperand":"count","operator":"eq","rightOperand":[1,2]}]}]}"#,
    ] {
        let doc = Rc::new(read_model(json).unwrap());
        let html = render_view(props(doc, EditMode::Edit, EditConfig::default())).await;
        assert!(html.contains("<option value=\"eq\""), "{html}");
        assert!(html.contains("<option value=\"neq\""), "{html}");
    }
}

// D4: a host that plans the switch itself offers every operator.
#[tokio::test]
async fn a_host_that_plans_operator_switches_gets_every_operator_offered() {
    let many = Rc::new(
        read_model(
            r#"{"@type":"Set","permission":[{"action":"use","constraint":[{"leftOperand":"count","operator":"isAnyOf","rightOperand":[1,2,3]}]}]}"#,
        )
        .unwrap(),
    );
    let cfg = EditConfig {
        host_plans_operator_switches: true,
        ..EditConfig::default()
    };
    let html = render_view(props(many, EditMode::Edit, cfg)).await;
    assert!(html.contains("<option value=\"eq\""), "{html}");
    assert!(html.contains("<option value=\"neq\""), "{html}");
}

// --- profiles: the read-only glossary and the read-mode titles ------------------

use prose_core::profile::{ProfileVocab, Wording};
use prose_yew::SharedWording;

fn profile_file(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../prose-core/tests/profiles/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn research_wording() -> Rc<Wording> {
    let p = ProfileVocab::parse(&profile_file("research.jsonld")).unwrap();
    Rc::new(Wording::new().with_profile(p))
}

fn prose_props(json: String, wording: Option<Rc<Wording>>, glossary: bool) -> OdrlProseProps {
    OdrlProseProps {
        json: json.into(),
        class: Classes::new(),
        wording: wording.map(SharedWording),
        glossary,
    }
}

/// Compares with, or (with WRITE_SNAPSHOTS set and the file missing) writes,
/// `tests/snapshots/{name}.html`.
fn snapshot(name: &str, html: &str) {
    let path = format!("{}/tests/snapshots/{name}.html", env!("CARGO_MANIFEST_DIR"));
    if std::env::var("WRITE_SNAPSHOTS").is_ok() && !std::path::Path::new(&path).exists() {
        std::fs::write(&path, html).unwrap();
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("no snapshot {path}; got:\n{html}"));
    assert_eq!(html, want, "snapshot {name}");
}

#[tokio::test]
async fn no_wording_snapshots_unchanged() {
    for name in ["offer", "agreement", "set"] {
        let want = std::fs::read_to_string(format!(
            "{}/tests/snapshots/{name}.html",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        for wording in [None, Some(Rc::new(Wording::default()))] {
            let html = ssr::<OdrlProse>(prose_props(fixture(name), wording, true)).await;
            assert_eq!(html, want, "snapshot {name}");
        }
    }
    // The editing view: an empty wording changes nothing either.
    for name in ["offer", "agreement", "set"] {
        for mode in [EditMode::Read, EditMode::Edit] {
            let plain = render_view(props(doc_of(name), mode, EditConfig::default())).await;
            let empty = EditConfig::default().with_wording(Rc::new(Wording::default()));
            let worded = render_view(props(doc_of(name), mode, empty)).await;
            assert_eq!(plain, worded, "{name} {mode:?}");
        }
    }
}

#[tokio::test]
async fn odrl_prose_with_wording_renders_glossary_details() {
    let json = profile_file("research-policy.json");
    let html = ssr::<OdrlProse>(prose_props(json.clone(), Some(research_wording()), true)).await;
    // The sentences use the profile's labels.
    assert!(html.contains("may fine-tune"), "{html}");
    for needle in [
        // Yew writes a boolean attribute as open="open", attributes sorted.
        "<details open=\"open\" class=\"prose-glossary\">",
        "<summary>Terms from the profile</summary>",
        "<dt><span>Fine-tune</span> <code>ex:fineTune</code></dt>",
        "<dd>To adapt an already trained model using the Asset.</dd>",
        "<dd class=\"prose-note\">Usage: Fine-tuning implies that the Asset may be read.</dd>",
        "<dd class=\"prose-kind-of\">A kind of: Train</dd>",
        "<dd class=\"prose-implies\">Implies: ",
        "<dd class=\"prose-from\">From Research Data Profile</dd>",
        "<code>ex:researchPurpose</code>",
        "<code>ex:clinical</code>",
    ] {
        assert!(html.contains(needle), "{needle} missing in:\n{html}");
    }
    // One glossary per policy, inside it, after the rules.
    assert_eq!(html.matches("prose-glossary").count(), 1);
    let rules = html.find("prose-rules").unwrap();
    let glossary = html.find("prose-glossary").unwrap();
    let section_end = html.find("</section>").unwrap();
    assert!(rules < glossary && glossary < section_end);
    // Not declared by the profile: no entry.
    assert!(!html.contains("<code>ex:distill</code>"));
    snapshot("profile_research", &html);

    // Turned off: the worded sentences without the glossary.
    let off = ssr::<OdrlProse>(prose_props(json, Some(research_wording()), false)).await;
    assert!(off.contains("may fine-tune"));
    assert!(!off.contains("prose-glossary"), "{off}");
}

#[tokio::test]
async fn glossary_is_capped_per_policy() {
    // 40 declared actions, all used by one policy.
    let mut graph = vec![];
    let mut actions = vec![];
    for i in 0..40 {
        graph.push(format!(
            r#"{{"@id": "ex:a{i}", "@type": "odrl:Action", "rdfs:label": "Action {i}"}}"#
        ));
        actions.push(format!(r#""ex:a{i}""#));
    }
    let profile = format!(
        r#"{{"@context": {{"odrl": "http://www.w3.org/ns/odrl/2/", "ex": "https://example.org/many/",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#"}}, "@graph": [{}]}}"#,
        graph.join(",")
    );
    let w = Rc::new(Wording::new().with_profile(ProfileVocab::parse(&profile).unwrap()));
    let policy = format!(
        r#"{{"@context": {{"ex": "https://example.org/many/"}}, "@type": "Set",
            "permission": [{{"action": [{}]}}]}}"#,
        actions.join(",")
    );
    let html = ssr::<OdrlProse>(prose_props(policy, Some(w), true)).await;
    assert_eq!(html.matches("<dt>").count(), 30, "{html}");
}

#[tokio::test]
async fn glossary_escapes_markup() {
    let profile = r#"{
        "@context": {"odrl": "http://www.w3.org/ns/odrl/2/", "ex": "https://example.org/x/",
                     "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                     "skos": "http://www.w3.org/2004/02/skos/core#"},
        "@graph": [{"@id": "ex:act", "@type": "odrl:Action",
                    "rdfs:label": "Act &lt;b&gt;",
                    "skos:definition": "&lt;script&gt;alert(1)&lt;/script&gt; <img src=x onerror=alert(2)>"}]
    }"#;
    let w = Rc::new(Wording::new().with_profile(ProfileVocab::parse(profile).unwrap()));
    let policy = r#"{"@context": {"ex": "https://example.org/x/"}, "@type": "Set",
        "permission": [{"action": "ex:act"}]}"#;
    let html = ssr::<OdrlProse>(prose_props(policy.to_string(), Some(w.clone()), true)).await;
    assert!(html.contains("prose-glossary"), "{html}");
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains("<img"), "{html}");
    assert!(!html.contains("<b>"), "{html}");
    assert!(
        html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
        "{html}"
    );

    // The same text as a read-mode title is an attribute value, escaped.
    let doc = Rc::new(read_model(policy).unwrap());
    let cfg = EditConfig::default().with_wording(w);
    let html = render_view(props(doc, EditMode::Read, cfg)).await;
    assert!(!html.contains("<script"), "{html}");
    assert!(html.contains("&lt;script&gt;"), "{html}");
}

#[tokio::test]
async fn read_mode_term_title_has_definition() {
    let doc = Rc::new(read_model(&profile_file("research-policy.json")).unwrap());
    let cfg = EditConfig::default().with_wording(research_wording());
    let html = render_view(props(doc.clone(), EditMode::Read, cfg)).await;
    let terms = spans_with(
        &html,
        "data-iri=\"https://example.org/odrl/profile/research/fineTune\"",
    );
    assert_eq!(terms.len(), 1, "{html}");
    let tag = terms[0].split('>').next().unwrap();
    assert!(
        tag.contains(
            "title=\"Fine-tune: To adapt an already trained model using the Asset. (ex:fineTune)\""
        ),
        "{tag}"
    );
    let id = tag
        .split("aria-describedby=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .expect("aria-describedby");
    let target = html
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("no element {id}"));
    assert!(
        html[target..].starts_with(&format!("id=\"{id}\" hidden"))
            || html[..target].ends_with("hidden "),
        "{html}"
    );
    assert!(html[target..].contains("To adapt an already trained model using the Asset."));
    // The sentence itself reads with the label.
    assert!(
        terms[0].starts_with(tag) && terms[0][tag.len()..].starts_with(">fine-tune"),
        "{}",
        terms[0]
    );
    // A term no profile declares keeps the raw title and no description.
    let distill = spans_with(&html, "title=\"ex:distill\"");
    assert_eq!(distill.len(), 1, "{html}");
    assert!(
        !distill[0]
            .split('>')
            .next()
            .unwrap()
            .contains("aria-describedby")
    );
    // Without a profile the same view is unchanged.
    let plain = render_view(props(doc, EditMode::Read, EditConfig::default())).await;
    assert!(!plain.contains("data-iri"));
}

// --- profiles: the edit dropdowns ----------------------------------------------

const REFINEMENT_OP: &str = "policy[0].permission[0].action[0].refinement[0]#operator";

async fn research_edit(json: &str, issues: Vec<Issue>) -> String {
    let doc = Rc::new(read_model(json).unwrap());
    let cfg = EditConfig::default().with_wording(research_wording());
    let mut p = props(doc, EditMode::Edit, cfg);
    p.issues = Rc::new(issues);
    render_view(p).await
}

/// The `<select>` element for `slot`, from its opening tag to `</select>`.
fn select_of<'a>(html: &'a str, slot: &str) -> &'a str {
    let at = html
        .find(&format!("data-slot=\"{slot}\""))
        .unwrap_or_else(|| panic!("no {slot} in:\n{html}"));
    let start = html[..at].rfind("<select").unwrap();
    let end = at + html[at..].find("</select>").unwrap();
    &html[start..end]
}

fn with_refinement(op: &str) -> String {
    profile_file("research-policy.json")
        .replace(r#""operator": "gteq""#, &format!(r#""operator": "{op}""#))
}

#[tokio::test]
async fn operator_select_suggested_and_other_optgroups() {
    let html = research_edit(&profile_file("research-policy.json"), vec![]).await;
    let sel = select_of(&html, REFINEMENT_OP);
    let suggested = sel
        .find("<optgroup label=\"Suggested by the profile\">")
        .unwrap_or_else(|| panic!("{sel}"));
    let other = sel
        .find("<optgroup label=\"Other operators\">")
        .unwrap_or_else(|| panic!("{sel}"));
    assert!(suggested < other);
    let (first, second) = (&sel[suggested..other], &sel[other..]);
    // prose:operator of ex:dataSubjectCount: gteq and gt.
    assert!(
        first.contains("<option value=\"gteq\" selected=\"selected\">"),
        "{first}"
    );
    assert!(first.contains("<option value=\"gt\">"), "{first}");
    assert_eq!(first.matches("<option").count(), 2, "{first}");
    assert!(second.contains("<option value=\"eq\">"), "{second}");
    assert!(!second.contains("value=\"gteq\""), "{second}");
    // The profile's own operator, written as the policy binds ex:.
    assert!(
        second.contains(
            "<option value=\"ex:withinRegion\">is within region (ex:withinRegion)</option>"
        ),
        "{second}"
    );
    // ex:retentionPeriod suggests lteq and lt.
    let sel = select_of(&html, "policy[0].permission[0].constraint[1]#operator");
    let other = sel.find("Other operators").unwrap();
    assert!(sel[..other].contains("value=\"lteq\"") && sel[..other].contains("value=\"lt\""));
    assert!(!sel[..other].contains("value=\"eq\""), "{sel}");
}

#[tokio::test]
async fn operator_select_flat_without_hints_includes_profile_operator() {
    let json = r#"{"@context": {"ex": "https://example.org/odrl/profile/research/"},
        "@type": "Set", "permission": [{"action": "use",
        "constraint": [{"leftOperand": "count", "operator": "eq", "rightOperand": 1}]}]}"#;
    let html = research_edit(json, vec![]).await;
    let sel = select_of(&html, "policy[0].permission[0].constraint[0]#operator");
    assert!(!sel.contains("<optgroup"), "{sel}");
    assert!(
        sel.contains(
            "<option value=\"ex:withinRegion\">is within region (ex:withinRegion)</option>"
        ),
        "{sel}"
    );
    // A policy that does not bind ex: gets the full IRI.
    let json = r#"{"@type": "Set", "permission": [{"action": "use",
        "constraint": [{"leftOperand": "count", "operator": "eq", "rightOperand": 1}]}]}"#;
    let html = research_edit(json, vec![]).await;
    let sel = select_of(&html, "policy[0].permission[0].constraint[0]#operator");
    assert!(
        sel.contains("<option value=\"https://example.org/odrl/profile/research/withinRegion\">"),
        "{sel}"
    );
    // The same label without the raw term when show_raw_terms is off.
    let doc = Rc::new(read_model(json).unwrap());
    let mut cfg = EditConfig::default().with_wording(research_wording());
    cfg.show_raw_terms = false;
    let html = render_view(props(doc, EditMode::Edit, cfg)).await;
    assert!(html.contains(">is within region</option>"), "{html}");
}

#[tokio::test]
async fn current_operator_outside_list_kept() {
    // eq is not among the suggested ones: it stays selected under "Other".
    let html = research_edit(&with_refinement("eq"), vec![]).await;
    let sel = select_of(&html, REFINEMENT_OP);
    let other = sel.find("Other operators").unwrap();
    assert!(
        sel[other..].contains("<option value=\"eq\" selected=\"selected\">"),
        "{sel}"
    );
    // An operator in no list is the first option, outside the groups.
    let html = research_edit(&with_refinement("odrl:weird"), vec![]).await;
    let sel = select_of(&html, REFINEMENT_OP);
    let weird = sel
        .find("<option value=\"odrl:weird\" selected=\"selected\">")
        .unwrap_or_else(|| panic!("{sel}"));
    assert!(weird < sel.find("<optgroup").unwrap(), "{sel}");
    assert!(sel.contains("odrl:weird (not in the list)"));
}

#[tokio::test]
async fn conflict_select_has_profile_term() {
    let html = research_edit(&profile_file("research-policy.json"), vec![]).await;
    let sel = select_of(&html, "policy[0]#conflict");
    assert!(
        sel.contains("<option value=\"ex:ignore\">Ignore conflicts</option>"),
        "{sel}"
    );
    let json = profile_file("research-policy.json")
        .replace(r#""uid":"#, r#""conflict": "ex:ignore", "uid":"#);
    let html = research_edit(&json, vec![]).await;
    let sel = select_of(&html, "policy[0]#conflict");
    assert!(
        sel.contains("<option value=\"ex:ignore\" selected=\"selected\">Ignore conflicts</option>"),
        "{sel}"
    );
    assert!(!sel.contains("not in the list"), "{sel}");
}

#[tokio::test]
async fn literal_slot_placeholder_from_datatype() {
    let html = research_edit(&profile_file("research-policy.json"), vec![]).await;
    let tag = |path: &str| -> String {
        spans_with(&html, &format!("data-slot=\"{path}\""))[0]
            .split('>')
            .next()
            .unwrap()
            .to_string()
    };
    let count = tag("policy[0].permission[0].action[0].refinement[0]#rightOperand[0]");
    assert!(
        count.contains("data-placeholder=\"whole number\""),
        "{count}"
    );
    assert!(
        count.contains("aria-placeholder=\"whole number\""),
        "{count}"
    );
    let period = tag("policy[0].permission[0].constraint[1]#rightOperand[0]");
    assert!(
        period.contains("data-placeholder=\"duration, e.g. P2Y\""),
        "{period}"
    );
    // No datatype hint: the configured placeholder.
    let purpose = tag("policy[0].permission[0].constraint[0]#rightOperand[0]");
    assert!(purpose.contains("data-placeholder=\"value\""), "{purpose}");
    // The host can change the words.
    let doc = Rc::new(read_model(&profile_file("research-policy.json")).unwrap());
    let mut cfg = EditConfig::default();
    cfg.labels.datatype_placeholders = vec![(
        "http://www.w3.org/2001/XMLSchema#integer".into(),
        "Ganzzahl".into(),
    )];
    let cfg = cfg.with_wording(research_wording());
    let html = render_view(props(doc, EditMode::Edit, cfg)).await;
    assert!(html.contains("data-placeholder=\"Ganzzahl\""));
}

#[tokio::test]
async fn issue_from_check_profile_inline() {
    let json = profile_file("research-policy.json");
    let doc = read_model(&json).unwrap();
    let issues = prose_core::edit::check_profile(&doc, &research_wording());
    assert!(!issues.is_empty());
    let html = research_edit(&json, issues).await;
    let distill = spans_with(
        &html,
        "data-slot=\"policy[0].permission[1].action[0]#name\"",
    );
    let tag = distill[0].split('>').next().unwrap();
    assert!(tag.contains("prose-slot-warning"), "{tag}");
    assert!(
        tag.contains(
            "aria-describedby=\"prose-issue-slot:policy[0].permission[1].action[0]#name-0\""
        ),
        "{tag}"
    );
    assert!(html.contains("prose-issue prose-issue-warning"), "{html}");
    assert!(html.contains("ex:distill"));
}
