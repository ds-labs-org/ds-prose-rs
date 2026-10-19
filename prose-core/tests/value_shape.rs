//! One shape for a constraint's right operand, shared by the reader, the
//! editor sentence and the dropdown.
use prose_core::wording::ValueShape;

const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

#[test]
fn value_shape_of_each_spelling() {
    use ValueShape::*;
    let full = |t: &str| format!("{XSD}{t}");
    for (value, datatype, want) in [
        ("2026-12-31", None, DateOnly),
        ("2026-12-31", Some(full("date")), DateOnly),
        ("2026-12-31", Some("xsd:date".to_string()), DateOnly),
        ("2026-12-31+02:00", Some(full("date")), DateOnly),
        ("2026-12-31T10:00:00Z", None, Instant),
        ("2026-12-31T10:00:00Z", Some(full("dateTime")), Instant),
        (
            "2026-12-31T10:00:00Z",
            Some("xsd:dateTime".to_string()),
            Instant,
        ),
        ("2026-12-31T10:00:00Z", Some(full("date")), Other),
        ("P30D", None, Duration),
        ("PT60M", Some(full("duration")), Duration),
        ("PT60M", Some("xsd:duration".to_string()), Duration),
        ("2026-12-31", Some(full("string")), Other),
        ("5", None, Other),
        ("", None, Other),
        ("2026-1-31", None, Other),
    ] {
        assert_eq!(
            ValueShape::of(value, datatype.as_deref()),
            want,
            "{value} {datatype:?}"
        );
    }
}

#[test]
fn value_shape_defaults_to_other() {
    assert_eq!(ValueShape::default(), ValueShape::Other);
}
