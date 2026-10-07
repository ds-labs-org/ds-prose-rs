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
