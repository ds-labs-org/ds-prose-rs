//! Hostile shapes cost a bounded amount (audit): a shared long `sh:in`
//! list, a chain of `sh:not`, and a huge `sh:in` are read in well under the
//! seconds they once took. Thresholds are generous so the tests do not flake.
#![cfg(feature = "turtle")]
use prose_core::shapes::ShapesGraph;
use std::fmt::Write;
use std::time::{Duration, Instant};

const SHAPES: usize = 200;
const HEAD: &str = "@prefix sh: <http://www.w3.org/ns/shacl#> .\n@prefix odrl: <http://www.w3.org/ns/odrl/2/> .\n@prefix ex: <https://example.org/> .\n@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n";

fn list(out: &mut String, name: &str, items: usize) {
    for i in 0..items {
        let rest = if i + 1 == items {
            "rdf:nil".to_string()
        } else {
            format!("_:{name}{}", i + 1)
        };
        writeln!(out, "_:{name}{i} rdf:first ex:v{i} ; rdf:rest {rest} .").unwrap();
    }
}

fn timed(text: &str, limit: Duration) -> ShapesGraph {
    let started = Instant::now();
    let g = ShapesGraph::parse(text).unwrap();
    let took = started.elapsed();
    assert!(took < limit, "took {took:?}");
    g
}

#[test]
fn a_long_list_shared_by_many_shapes_is_read_once() {
    let mut t = HEAD.to_string();
    list(&mut t, "l", 5_000);
    for n in 0..SHAPES {
        writeln!(
            t,
            "ex:S{n} a sh:NodeShape ; sh:property [ sh:path odrl:leftOperand ; sh:hasValue ex:left{n} ] , [ sh:path odrl:rightOperand ; sh:in _:l0 ] ."
        )
        .unwrap();
    }
    let g = timed(&t, Duration::from_secs(5));
    assert_eq!(
        g.constraint("https://example.org/left0")
            .unwrap()
            .values
            .len(),
        5_000
    );
}

#[test]
fn a_chain_of_sh_not_is_skipped_in_linear_time() {
    let mut t = HEAD.to_string();
    for n in 0..3_000 {
        writeln!(t, "ex:S{n} sh:not ex:S{} .", n + 1).unwrap();
    }
    writeln!(
        t,
        "ex:S3000 a sh:NodeShape ; sh:property [ sh:path odrl:leftOperand ; sh:hasValue ex:left ] , [ sh:path odrl:rightOperand ; sh:in ( ex:a ) ] ."
    )
    .unwrap();
    let g = timed(&t, Duration::from_millis(1_500));
    assert!(g.constraint("https://example.org/left").is_none());
}

#[test]
fn values_past_the_cap_on_one_left_operand_are_cut_with_a_warning() {
    // Many shapes each add distinct values to one left operand.
    let mut t = HEAD.to_string();
    for n in 0..120 {
        write!(
            t,
            "ex:S{n} a sh:NodeShape ; sh:property [ sh:path odrl:leftOperand ; sh:hasValue ex:left ] , [ sh:path odrl:rightOperand ; sh:in ("
        )
        .unwrap();
        for k in 0..200 {
            write!(t, " ex:v{n}x{k}").unwrap();
        }
        writeln!(t, " ) ] .").unwrap();
    }
    let g = timed(&t, Duration::from_secs(5));
    let c = g.constraint("https://example.org/left").unwrap();
    assert_eq!(c.values.len(), 20_000);
    assert!(
        g.warnings().iter().any(|w| w.contains("more values than")),
        "{:?}",
        g.warnings()
    );
}
