//! The namespaces a profile reader looks for.

pub(crate) const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
pub(crate) const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub(crate) const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
pub(crate) const OWL: &str = "http://www.w3.org/2002/07/owl#";
pub(crate) const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";
pub(crate) const DCT: &str = "http://purl.org/dc/terms/";
pub(crate) const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
pub(crate) const PROF: &str = "http://www.w3.org/ns/dx/prof/";
/// The ds-prose suggestion extension. A ds-prose convention, not ODRL.
pub(crate) const PROSE: &str = "https://ds-labs-org.github.io/ds-prose-rs/profile#";

pub(crate) const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub(crate) const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// Prefixes read even when a document does not bind them, each with a
/// warning. An author's own binding always wins.
pub(crate) const WELL_KNOWN: &[(&str, &str)] = &[
    ("odrl", ODRL),
    ("rdf", RDF),
    ("rdfs", RDFS),
    ("owl", OWL),
    ("skos", SKOS),
    ("dct", DCT),
    ("dcterms", DCT),
    ("xsd", XSD),
    ("prof", PROF),
    ("prose", PROSE),
];

/// The prefixes the W3C ODRL context (`http://www.w3.org/ns/odrl.jsonld`)
/// binds. A document that names that context gets them without a warning.
pub(crate) const ODRL_CONTEXT: &[(&str, &str)] = &[
    ("odrl", ODRL),
    ("rdf", RDF),
    ("rdfs", RDFS),
    ("owl", OWL),
    ("skos", SKOS),
    ("dct", DCT),
    ("xsd", XSD),
    ("vcard", "http://www.w3.org/2006/vcard/ns#"),
    ("foaf", "http://xmlns.com/foaf/0.1/"),
    ("schema", "http://schema.org/"),
    ("cc", "http://creativecommons.org/ns#"),
];

/// Whether `s` names the W3C ODRL JSON-LD context.
pub(crate) fn is_odrl_context(s: &str) -> bool {
    matches!(
        s.trim_end_matches('/'),
        "http://www.w3.org/ns/odrl.jsonld" | "https://www.w3.org/ns/odrl.jsonld"
    )
}

/// `odrl:x` written out in full.
pub(crate) fn odrl(local: &str) -> String {
    format!("{ODRL}{local}")
}
