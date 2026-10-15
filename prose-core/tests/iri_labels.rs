//! Parties, assets and right operands named by an IRI (#45). The sentence
//! reads with the label the document declares for that IRI (`rdfs:label`,
//! `dct:title`, `skos:prefLabel`, `schema:name`); without one it keeps the
//! IRI, so nothing is lost for a text reader. Either way, the reading lists
//! the IRIs each policy names, so a host can show them short and keep the
//! IRI on hover or as a link.
use prose_core::profile::Wording;
use prose_core::wording::short_iri;
use prose_core::{Mention, Reading, render, render_with};
use serde_json::{Value, json};

fn read(policy: &Value) -> Reading {
    render_with(&policy.to_string(), &Wording::default()).unwrap()
}

fn sentence(r: &Reading) -> &str {
    &r.document.policies[0].rules[0].sentence
}

fn mention<'a>(r: &'a Reading, iri: &str) -> &'a Mention {
    r.mentions[0]
        .iter()
        .find(|m| m.iri == iri)
        .unwrap_or_else(|| panic!("{iri} is not among {:?}", r.mentions))
}

/// A permission whose asset and assigner have IRIs, and `nodes` beside it.
fn graph(rule: Value, nodes: Vec<Value>) -> Value {
    let mut graph = vec![json!({
        "@type": "Set",
        "uid": "http://example.com/policy:1",
        "permission": [rule]
    })];
    graph.extend(nodes);
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld",
            {"rdfs": "http://www.w3.org/2000/01/rdf-schema#", "dct": "http://purl.org/dc/terms/"}],
        "@graph": graph
    })
}

#[test]
fn an_asset_without_a_label_keeps_its_iri_and_is_listed() {
    let r = read(&graph(
        json!({"action": "play", "target": "http://example.com/asset:9898.movie"}),
        vec![],
    ));
    assert_eq!(
        sentence(&r),
        "Anyone may play http://example.com/asset:9898.movie."
    );
    let m = mention(&r, "http://example.com/asset:9898.movie");
    assert_eq!(m.label, None);
    assert_eq!(m.text(), "http://example.com/asset:9898.movie");
    assert_eq!(m.short_label(), "9898.movie");
}

#[test]
fn a_label_declared_on_a_node_of_the_graph_names_the_asset() {
    let r = read(&graph(
        json!({"action": "play", "target": "http://example.com/asset:9898.movie"}),
        vec![json!({"@id": "http://example.com/asset:9898.movie", "rdfs:label": "The Movie"})],
    ));
    assert_eq!(sentence(&r), "Anyone may play The Movie.");
    let m = mention(&r, "http://example.com/asset:9898.movie");
    assert_eq!(m.label.as_deref(), Some("The Movie"));
    assert_eq!(m.text(), "The Movie");
    assert_eq!(m.short_label(), "The Movie");
    assert!(
        r.document.warnings.is_empty(),
        "a node that only labels an IRI is not a skipped policy: {:?}",
        r.document.warnings
    );
}

#[test]
fn a_label_on_the_party_itself_names_it() {
    let r = read(&graph(
        json!({"action": "play", "target": "http://example.com/a",
            "assigner": {"@id": "http://example.com/party:org:abc", "dct:title": "ABC Pictures"},
            "assignee": {"@id": "urn:person:b", "http://www.w3.org/2004/02/skos/core#prefLabel": "Billie"}}),
        vec![],
    ));
    assert_eq!(
        sentence(&r),
        "Billie may play http://example.com/a, as permitted by ABC Pictures."
    );
}

#[test]
fn every_label_vocabulary_is_read() {
    for key in [
        "rdfs:label",
        "http://www.w3.org/2000/01/rdf-schema#label",
        "dct:title",
        "dcterms:title",
        "http://purl.org/dc/terms/title",
        "skos:prefLabel",
        "schema:name",
        "http://schema.org/name",
        "https://schema.org/name",
    ] {
        let r = read(&graph(
            json!({"action": "play", "target": "urn:asset:1"}),
            vec![json!({"@id": "urn:asset:1", key: "The Movie"})],
        ));
        assert_eq!(sentence(&r), "Anyone may play The Movie.", "{key}");
    }
}

#[test]
fn a_label_key_is_read_through_the_documents_prefixes() {
    let r = read(&json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"lbl": "http://www.w3.org/2000/01/rdf-schema#"}],
        "@type": "Set",
        "uid": "urn:p",
        "permission": [{"action": "play",
            "target": {"@id": "urn:asset:1", "lbl:label": "The Movie"}}]
    }));
    assert_eq!(sentence(&r), "Anyone may play The Movie.");
}

#[test]
fn an_english_or_untagged_label_wins_over_another_language() {
    let r = read(&graph(
        json!({"action": "play", "target": "urn:asset:1"}),
        vec![json!({"@id": "urn:asset:1", "rdfs:label": [
            {"@value": "Der Film", "@language": "de"},
            {"@value": "The Movie", "@language": "en"}
        ]})],
    ));
    assert_eq!(sentence(&r), "Anyone may play The Movie.");
    let r = read(&graph(
        json!({"action": "play", "target": "urn:asset:1"}),
        vec![
            json!({"@id": "urn:asset:1", "rdfs:label": {"@value": "Der Film", "@language": "de"}}),
        ],
    ));
    assert_eq!(
        sentence(&r),
        "Anyone may play Der Film.",
        "the only label is used"
    );
}

#[test]
fn a_right_operand_iri_reads_with_its_label() {
    let r = read(&graph(
        json!({"action": "play", "target": "urn:asset:1",
            "constraint": [{"leftOperand": "spatial", "operator": "eq",
                "rightOperand": {"@id": "https://www.wikidata.org/wiki/Q183"}}]}),
        vec![json!({"@id": "https://www.wikidata.org/wiki/Q183", "schema:name": "Germany"})],
    ));
    assert_eq!(
        r.document.policies[0].rules[0].conditions[0].text,
        "the location is equal to Germany"
    );
    let m = mention(&r, "https://www.wikidata.org/wiki/Q183");
    assert_eq!(m.label.as_deref(), Some("Germany"));
}

#[test]
fn a_compact_iri_is_listed_expanded_and_shown_as_written() {
    let r = read(&json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": "http://example.com/"}],
        "@type": "Set",
        "uid": "urn:p",
        "permission": [{"action": "play", "target": "ex:asset:1"}]
    }));
    assert_eq!(sentence(&r), "Anyone may play ex:asset:1.");
    let m = mention(&r, "http://example.com/asset:1");
    assert_eq!(m.written, "ex:asset:1");
    assert_eq!(m.text(), "ex:asset:1");
}

#[test]
fn a_label_two_iris_share_names_neither() {
    let r = read(&graph(
        json!({"action": "play", "target": ["urn:asset:1", "urn:asset:2"]}),
        vec![
            json!({"@id": "urn:asset:1", "rdfs:label": "The Movie"}),
            json!({"@id": "urn:asset:2", "rdfs:label": "The Movie"}),
        ],
    ));
    assert_eq!(sentence(&r), "Anyone may play urn:asset:1 and urn:asset:2.");
}

#[test]
fn a_collection_and_a_party_of_the_policy_are_listed() {
    let r = read(&json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Offer",
        "uid": "urn:p",
        "assigner": {"@id": "urn:org:a", "rdfs:label": "Org A"},
        "permission": [{"action": "play",
            "target": {"@type": "AssetCollection", "source": "urn:collection:films"}}]
    }));
    assert_eq!(r.document.policies[0].intro, "An offer made by Org A.");
    assert_eq!(
        sentence(&r),
        "Anyone may play any asset in the collection urn:collection:films, as permitted by Org A."
    );
    assert!(
        r.mentions[0]
            .iter()
            .any(|m| m.iri == "urn:collection:films")
    );
    assert!(r.mentions[0].iter().any(|m| m.iri == "urn:org:a"));
}

#[test]
fn a_plain_name_is_not_an_iri() {
    let r = read(&json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set",
        "uid": "urn:p",
        "permission": [{"action": "play", "target": "movie", "assignee": "Alice"}]
    }));
    assert_eq!(sentence(&r), "Alice may play movie.");
    assert!(r.mentions[0].is_empty(), "{:?}", r.mentions);
}

#[test]
fn render_reads_labels_too() {
    let doc = render(
        &graph(
            json!({"action": "play", "target": "urn:asset:1"}),
            vec![json!({"@id": "urn:asset:1", "rdfs:label": "The Movie"})],
        )
        .to_string(),
    )
    .unwrap();
    assert_eq!(
        doc.policies[0].rules[0].sentence,
        "Anyone may play The Movie."
    );
}

#[test]
fn the_short_label_is_the_last_meaningful_segment() {
    assert_eq!(
        short_iri("http://example.com/asset:9898.movie"),
        "9898.movie"
    );
    assert_eq!(short_iri("http://example.com/party/alice"), "alice");
    assert_eq!(short_iri("http://example.com/vocab#term"), "term");
    assert_eq!(short_iri("http://example.com/party/alice/"), "alice");
    // A last segment without a letter says little alone: the one before it
    // goes with it.
    assert_eq!(short_iri("urn:uuid:1234"), "uuid:1234");
    assert_eq!(
        short_iri("http://example.com/photoAlbum:55"),
        "photoAlbum:55"
    );
    assert_eq!(short_iri("http://example.com/c/9"), "c/9");
    assert_eq!(short_iri("http://example.com/2024/07/"), "2024/07");
    assert_eq!(short_iri("http://example.com/"), "example.com");
    assert_eq!(short_iri("urn:"), "urn");
    assert_eq!(short_iri("plain"), "plain");
}

/// A node that does more than label an IRI is not read as a label only: a
/// labelled rule, or a dataset holding a policy, left out of the reading
/// is still reported (#45 review).
#[test]
fn a_labelled_node_with_more_than_a_label_is_still_reported_as_skipped() {
    let rule = read(&graph(
        json!({"action": "play", "target": "http://ex.com/a1"}),
        vec![json!({"@id": "http://ex.com/r2", "@type": "Prohibition",
            "rdfs:label": "No sharing", "action": "distribute", "target": "http://ex.com/a1"})],
    ));
    assert_eq!(
        rule.document.warnings,
        ["skipped a node that is not an ODRL policy"]
    );
    let dataset = read(&graph(
        json!({"action": "play", "target": "http://ex.com/a1"}),
        vec![
            json!({"@id": "http://ex.com/ds", "@type": "http://www.w3.org/ns/dcat#Dataset",
            "dct:title": "A dataset",
            "http://www.w3.org/ns/odrl/2/hasPolicy": {"@type": "Offer", "uid": "urn:q",
                "permission": [{"action": "play", "target": "http://ex.com/a2"}]}}),
        ],
    ));
    assert_eq!(
        dataset.document.warnings,
        ["skipped a node that is not an ODRL policy"]
    );
}

/// The text of each span of `names` in `text`, with its mention's IRI.
fn spans<'a>(
    r: &'a Reading,
    text: &'a str,
    names: &[prose_core::NameSpan],
) -> Vec<(&'a str, &'a str)> {
    names
        .iter()
        .map(|n| (&text[n.start..n.end], r.mentions[0][n.mention].iri.as_str()))
        .collect()
}

#[test]
fn the_reading_says_where_each_text_names_an_iri_and_nowhere_else() {
    let r = read(&graph(
        json!({"action": "play",
            "assignee": {"@id": "http://ex.com/alice", "rdfs:label": "Alice"},
            "target": "http://ex.com/d1",
            "constraint": [
                {"leftOperand": "recipient", "operator": "neq", "rightOperand": "Alice"},
                {"leftOperand": "spatial", "operator": "eq",
                    "rightOperand": {"@id": "https://www.wikidata.org/wiki/Q183"}}]}),
        vec![json!({"@id": "https://www.wikidata.org/wiki/Q183", "schema:name": "Germany"})],
    ));
    let rule = &r.document.policies[0].rules[0];
    let names = &r.names[0].rules[0];
    assert_eq!(
        spans(&r, &rule.sentence, &names.sentence),
        [
            ("Alice", "http://ex.com/alice"),
            ("http://ex.com/d1", "http://ex.com/d1")
        ]
    );
    assert_eq!(
        rule.conditions[0].text,
        "the recipient is not equal to \"Alice\""
    );
    assert_eq!(names.conditions[0].text, [], "a literal is not the party");
    assert_eq!(
        spans(&r, &rule.conditions[1].text, &names.conditions[1].text),
        [("Germany", "https://www.wikidata.org/wiki/Q183")]
    );
    assert!(r.names[0].intro.is_empty());
}

#[test]
fn an_iri_written_two_ways_is_one_mention_named_in_both_places() {
    let r = read(&json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": "http://example.com/"}],
        "@type": "Offer",
        "uid": "urn:p",
        "assigner": "ex:org",
        "permission": [{"action": "use", "target": "ex:asset:1"},
            {"action": "play", "target": "http://example.com/asset:1"}]
    }));
    assert_eq!(r.mentions[0].len(), 2, "{:?}", r.mentions);
    let p = &r.document.policies[0];
    assert_eq!(p.intro, "An offer made by ex:org.");
    assert_eq!(
        spans(&r, &p.intro, &r.names[0].intro),
        [("ex:org", "http://example.com/org")]
    );
    assert_eq!(
        spans(&r, &p.rules[1].sentence, &r.names[0].rules[1].sentence),
        [
            ("http://example.com/asset:1", "http://example.com/asset:1"),
            ("ex:org", "http://example.com/org")
        ]
    );
    assert_eq!(
        spans(&r, &p.rules[0].sentence, &r.names[0].rules[0].sentence)[0],
        ("ex:asset:1", "http://example.com/asset:1")
    );
}

#[test]
fn the_reading_of_a_duty_and_a_refined_party_names_their_iris() {
    let r = read(&graph(
        json!({"action": "play", "target": "urn:asset:1",
            "duty": [{"action": "compensate", "assignee": "urn:party:payer"}]}),
        vec![],
    ));
    let rule = &r.document.policies[0].rules[0];
    let duty = &rule.follow_ups[0].rules[0];
    let names = &r.names[0].rules[0].follow_ups[0][0];
    assert!(
        spans(&r, &duty.sentence, &names.sentence)
            .contains(&("urn:party:payer", "urn:party:payer")),
        "{}",
        duty.sentence
    );
}

#[test]
fn a_document_that_writes_a_mark_character_is_read_as_written() {
    let forged = "\u{E000}0\u{E001}ex:org\u{E002}";
    let r = read(&json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": "http://example.com/"}],
        "@type": "Offer",
        "uid": "urn:p",
        "assigner": "ex:org",
        "permission": [{"action": "use", "target": "ex:asset:1",
            "constraint": [{"leftOperand": "purpose", "operator": "eq", "rightOperand": forged}]}]
    }));
    let rule = &r.document.policies[0].rules[0];
    assert_eq!(
        rule.conditions[0].text,
        format!("the purpose is equal to {forged}")
    );
    assert_eq!(
        rule.sentence,
        "Anyone may use ex:asset:1, as permitted by ex:org."
    );
    assert_eq!(r.names[0].rules[0].conditions[0].text, []);
}

/// The label in the first of the wording's languages wins; without one,
/// an untagged label wins over an English one, and English over the rest
/// (#45 review).
#[test]
fn the_wordings_language_wins_then_untagged_then_english() {
    let doc = graph(
        json!({"action": "play", "target": "urn:asset:1"}),
        vec![json!({"@id": "urn:asset:1", "rdfs:label": [
            {"@value": "The Movie", "@language": "en"},
            {"@value": "Der Film", "@language": "de"},
            "Film 1",
            {"@value": "Le Film", "@language": "fr"}
        ]})],
    );
    let in_lang = |langs: &[&str]| {
        let w = Wording::new().with_languages(langs.iter().map(|l| l.to_string()).collect());
        let r = render_with(&doc.to_string(), &w).unwrap();
        r.document.policies[0].rules[0].sentence.clone()
    };
    assert_eq!(in_lang(&["de"]), "Anyone may play Der Film.");
    assert_eq!(in_lang(&["de-CH"]), "Anyone may play Der Film.");
    assert_eq!(in_lang(&["it", "fr"]), "Anyone may play Le Film.");
    assert_eq!(in_lang(&["it"]), "Anyone may play Film 1.", "untagged next");
    assert_eq!(in_lang(&[]), "Anyone may play Film 1.");
    let no_untagged = graph(
        json!({"action": "play", "target": "urn:asset:1"}),
        vec![json!({"@id": "urn:asset:1", "rdfs:label": [
            {"@value": "Der Film", "@language": "de"},
            {"@value": "The Movie", "@language": "en-GB"}
        ]})],
    );
    assert_eq!(
        sentence(&read(&no_untagged)),
        "Anyone may play The Movie.",
        "English next"
    );
}
