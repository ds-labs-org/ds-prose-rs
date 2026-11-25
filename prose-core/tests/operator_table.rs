//! The wording of every operator, for every kind of left operand, right
//! operand shape and number, pinned in one file. A change to the phrase
//! table must leave this file byte-identical unless it means to change a
//! sentence. Regenerate with `BLESS=1 cargo test -p prose-core --test
//! operator_table` and read the diff.
use prose_core::profile::{Prefixes, ProfileLimits, ProfileVocab, Wording};
use prose_core::wording::{OperatorQuery, ValueShape, operator_phrase};
use serde_json::json;

const EX: &str = "https://example.org/odrl/profile/research/";

fn profile() -> Wording {
    let op = |id: &str, label: &str| json!({"@id": format!("ex:{id}"), "@type": "odrl:Operator", "rdfs:label": label});
    let v = ProfileVocab::from_value(
        &json!({
            "@context": {"odrl": "http://www.w3.org/ns/odrl/2/", "ex": EX,
                         "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                         "owl": "http://www.w3.org/2002/07/owl#"},
            "@graph": [
                {"@id": "https://example.org/p", "@type": "owl:Ontology"},
                op("earlierThan", "Earlier than"),
                op("hasOverlap", "Has overlap with"),
                op("doesNotOverlap", "Does not overlap"),
                op("containsDay", "Contains"),
                op("matchesWindow", "Matches window"),
                op("areNear", "Are near"),
                op("withinRegion", "Is within region"),
                op("have", "Have two"),
                op("bare", "x"),
                {"@id": "ex:moment", "@type": "odrl:LeftOperand", "rdfs:label": "Moment"}
            ]
        }),
        &ProfileLimits::default(),
    )
    .unwrap();
    Wording::new().with_profile(v)
}

const LEFTS: [&str; 8] = [
    "dateTime",
    "odrl:dateTime",
    "timeInterval",
    "delayPeriod",
    "elapsedTime",
    "count",
    "ex:moment",
    "ex:other",
];

const OPS: [&str; 22] = [
    "eq",
    "neq",
    "gt",
    "gteq",
    "lt",
    "lteq",
    "isA",
    "hasPart",
    "isPartOf",
    "isAllOf",
    "isAnyOf",
    "isNoneOf",
    "odrl:lt",
    "ex:near",
    "ex:earlierThan",
    "ex:hasOverlap",
    "ex:doesNotOverlap",
    "ex:containsDay",
    "ex:matchesWindow",
    "ex:areNear",
    "ex:withinRegion",
    "ex:bare",
];

fn table() -> String {
    let w = profile();
    let p = Prefixes::of_document(&json!({"@context": {"ex": EX}}));
    let shapes = [
        ("instant", ValueShape::Instant),
        ("day", ValueShape::DateOnly),
        ("duration", ValueShape::Duration),
        ("other", ValueShape::Other),
    ];
    let mut out = String::new();
    for profiled in [false, true] {
        for left in LEFTS {
            for op in OPS {
                for (name, shape) in shapes {
                    let mut q = OperatorQuery::new(left, op).value(shape);
                    if profiled {
                        q = q.wording(&w, &p);
                    }
                    out.push_str(&format!(
                        "{} | {left} | {op} | {name} | {}\n",
                        if profiled { "profile" } else { "builtin" },
                        operator_phrase(&q)
                    ));
                }
            }
        }
    }
    out
}

#[test]
fn every_operator_for_every_shape_and_number_reads_as_pinned() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/operator_table.txt"
    );
    if std::env::var_os("BLESS").is_some() {
        std::fs::write(path, table()).unwrap();
    }
    let want = std::fs::read_to_string(path).expect("run once with BLESS=1");
    assert_eq!(table(), want);
}
