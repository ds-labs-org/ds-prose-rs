//! Following constraint references is bounded in depth, not only in count:
//! a chain of references in a flat `@graph` must not nest the reading (and
//! the reader's recursion) deeper than JSON nesting itself could. Its own
//! test binary: a stack overflow aborts the process, not just the test.
use prose_core::{Condition, render};

/// A policy referring to `urn:c:0`, where `urn:c:i` is a logical constraint
/// on `urn:c:{i+1}`, down to one plain constraint: `n` links in all.
fn chain(n: usize) -> String {
    let mut declared: Vec<String> = (0..n - 1)
        .map(|i| {
            format!(
                r#"{{"@type": "LogicalConstraint", "@id": "urn:c:{i}", "and": [{{"@id": "urn:c:{}"}}]}}"#,
                i + 1
            )
        })
        .collect();
    declared.push(format!(
        r#"{{"@type": "Constraint", "@id": "urn:c:{}", "leftOperand": "count", "operator": "lteq", "rightOperand": 1}}"#,
        n - 1
    ));
    format!(
        r#"{{"@context": "http://www.w3.org/ns/odrl.jsonld", "@graph": [
            {{"@type": "Set", "uid": "urn:p",
              "permission": [{{"action": "use", "target": "urn:a", "constraint": {{"@id": "urn:c:0"}}}}]}},
            {}
        ]}}"#,
        declared.join(",")
    )
}

fn depth(c: &[Condition]) -> usize {
    c.iter().map(|c| 1 + depth(&c.children)).max().unwrap_or(0)
}

/// On a stack as small as wasm's (1 MiB), in a debug build.
#[test]
fn a_long_chain_of_references_is_read_to_a_bounded_depth() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let doc = render(&chain(1_000)).unwrap();
            let read = &doc.policies[0].rules[0].conditions;
            assert!(depth(read) <= 256, "{} levels", depth(read));
            assert!(
                doc.warnings.iter().any(|w| w.contains("deeper than")),
                "{:?}",
                doc.warnings
            );
        })
        .unwrap()
        .join()
        .expect("the render overflowed its stack or panicked");
}

/// A chain well within the bound reads in full, with no warning.
#[test]
fn a_short_chain_of_references_reads_in_full() {
    let doc = render(&chain(20)).unwrap();
    let read = &doc.policies[0].rules[0].conditions;
    assert_eq!(depth(read), 20);
    assert_eq!(doc.warnings, Vec::<String>::new());
}
