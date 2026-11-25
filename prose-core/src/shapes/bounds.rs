//! `sh:minInclusive` and `sh:maxInclusive` on the lexical form of a value.
use std::cmp::Ordering;

/// A number, or an ISO 8601 date or date-time (they order as text).
enum Kind {
    Number(f64),
    Moment(String),
}

fn kind(s: &str) -> Option<Kind> {
    let t = s.trim();
    if let Ok(n) = t.parse::<f64>()
        && n.is_finite()
    {
        return Some(Kind::Number(n));
    }
    let b = t.as_bytes();
    let date = b.len() >= 10
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit);
    date.then(|| Kind::Moment(t.to_string()))
}

/// Which bound a value breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bound {
    Below,
    Above,
}

fn compare(v: &Kind, b: &Kind) -> Option<Ordering> {
    match (v, b) {
        (Kind::Number(v), Kind::Number(b)) => v.partial_cmp(b),
        // Dates compare as text up to the shorter form's length: a bare
        // date against a date-time compares the days.
        (Kind::Moment(v), Kind::Moment(b)) => {
            let n = v.len().min(b.len());
            Some(v.get(..n)?.cmp(b.get(..n)?))
        }
        _ => None,
    }
}

/// The bound `value` breaks, if any. A value or a bound that is not a number
/// or a date, or two that are not of one kind, cannot be compared and break
/// nothing: a bound that cannot be checked is not an error.
pub(crate) fn outside_bounds(value: &str, min: Option<&str>, max: Option<&str>) -> Option<Bound> {
    let v = kind(value)?;
    if let Some(b) = min.and_then(kind)
        && compare(&v, &b) == Some(Ordering::Less)
    {
        return Some(Bound::Below);
    }
    if let Some(b) = max.and_then(kind)
        && compare(&v, &b) == Some(Ordering::Greater)
    {
        return Some(Bound::Above);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_compare_as_numbers() {
        assert_eq!(
            outside_bounds("0", Some("1"), Some("100")),
            Some(Bound::Below)
        );
        assert_eq!(
            outside_bounds("101", Some("1"), Some("100")),
            Some(Bound::Above)
        );
        assert_eq!(outside_bounds("1", Some("1"), Some("100")), None);
        assert_eq!(outside_bounds("100", Some("1"), Some("100")), None);
        assert_eq!(
            outside_bounds("9", Some("10"), None),
            Some(Bound::Below),
            "not as text"
        );
        assert_eq!(
            outside_bounds("2.5", None, Some("2.49")),
            Some(Bound::Above)
        );
    }

    #[test]
    fn dates_compare_as_dates() {
        assert_eq!(
            outside_bounds("2026-01-01", Some("2026-06-01"), None),
            Some(Bound::Below)
        );
        assert_eq!(
            outside_bounds("2026-07-01T10:00:00Z", None, Some("2026-06-30")),
            Some(Bound::Above)
        );
        assert_eq!(
            outside_bounds("2026-06-30", Some("2026-06-01"), Some("2026-06-30")),
            None
        );
    }

    #[test]
    fn what_cannot_be_compared_breaks_nothing() {
        assert_eq!(outside_bounds("many", Some("1"), Some("2")), None);
        assert_eq!(outside_bounds("5", Some("2026-01-01"), None), None);
        assert_eq!(outside_bounds("5", Some("x"), Some("y")), None);
        assert_eq!(outside_bounds("5", None, None), None);
    }
}
