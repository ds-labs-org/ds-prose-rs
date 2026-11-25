//! Shared fixtures: shapes that close an operand to a class, and a profile
//! whose class is a chain too deep for the default limits.
#![allow(dead_code)]
use serde_json::json;

/// A profile whose members of `dx:Big` are a chain of `n`, each narrower than the one before, and shapes that close `dx:purpose` to the members of that class. A chain deeper than the default depth limit is cut.
pub fn chain_profile(n: usize) -> String {
    let mut graph = vec![
        json!({"@id": "https://example.org/chain/profile", "@type": "owl:Ontology"}),
        json!({"@id": "dx:purpose", "@type": ["odrl:LeftOperand", "skos:Concept"]}),
        json!({"@id": "dx:Big", "@type": "rdfs:Class"}),
    ];
    for i in 0..n {
        let mut node = json!({"@id": format!("dx:m{i}"), "@type": ["odrl:RightOperand", "dx:Big", "skos:Concept"]});
        if i > 0 {
            node["skos:broader"] = json!({"@id": format!("dx:m{}", i - 1)});
        }
        graph.push(node);
    }
    json!({
        "@context": {"odrl": "http://www.w3.org/ns/odrl/2/", "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                     "skos": "http://www.w3.org/2004/02/skos/core#", "owl": "http://www.w3.org/2002/07/owl#",
                     "dx": "https://example.org/dataspace/"},
        "@graph": graph
    })
    .to_string()
}

pub const CLASS_SHAPES: &str = r#"{"@context":{"sh":"http://www.w3.org/ns/shacl#","odrl":"http://www.w3.org/ns/odrl/2/","dx":"https://example.org/dataspace/"},"@graph":[{"@id":"dx:S","@type":"sh:NodeShape","sh:property":[{"sh:path":{"@id":"odrl:leftOperand"},"sh:hasValue":{"@id":"dx:purpose"}},{"sh:path":{"@id":"odrl:rightOperand"},"sh:class":{"@id":"dx:Big"}}]}]}"#;
