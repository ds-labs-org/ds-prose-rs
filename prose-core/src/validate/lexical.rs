//! The lexical spaces of the xsd datatypes the validator checks literals
//! against: hand-written matchers, no regex.

/// Whether a text fits a datatype's lexical space.
pub(crate) type Fits = fn(&str) -> bool;

/// The lexical check for an xsd datatype, and how a message names it;
/// `None` for a datatype that is not checked.
pub(crate) fn lexical(local: &str) -> Option<(Fits, &'static str)> {
    Some(match local {
        "integer" => (is_integer, "an integer"),
        "decimal" => (is_decimal, "a decimal number"),
        "boolean" => (is_boolean, "a boolean (true or false)"),
        "date" => (is_date, "a date"),
        "dateTime" => (is_date_time, "a date and time"),
        "duration" => (is_duration, "a duration"),
        _ => return None,
    })
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn unsigned(s: &str) -> &str {
    s.strip_prefix(['+', '-']).unwrap_or(s)
}

fn is_integer(s: &str) -> bool {
    digits(unsigned(s))
}

fn is_decimal(s: &str) -> bool {
    match unsigned(s).split_once('.') {
        None => digits(unsigned(s)),
        Some((i, f)) => {
            (i.is_empty() || digits(i))
                && (f.is_empty() || digits(f))
                && !(i.is_empty() && f.is_empty())
        }
    }
}

fn is_boolean(s: &str) -> bool {
    matches!(s, "true" | "false" | "1" | "0")
}

/// Two digits in `lo..=hi`.
fn two(s: &str, lo: u8, hi: u8) -> bool {
    s.len() == 2 && digits(s) && s.parse::<u8>().is_ok_and(|n| (lo..=hi).contains(&n))
}

/// `Z`, `+hh:mm` or `-hh:mm`, or nothing.
fn timezone(s: &str) -> bool {
    match s.strip_prefix(['+', '-']) {
        _ if s.is_empty() || s == "Z" => true,
        Some(hm) => hm
            .split_once(':')
            .is_some_and(|(h, m)| two(h, 0, 14) && two(m, 0, 59)),
        None => false,
    }
}

/// `YYYY-MM-DD`, a year of four or more digits (possibly negative); the
/// rest of the text after it.
fn date_part(s: &str) -> Option<&str> {
    let s = s.strip_prefix('-').unwrap_or(s);
    let (year, rest) = s.split_once('-')?;
    if year.len() < 4 || !digits(year) {
        return None;
    }
    let (month, rest) = (rest.get(..2)?, rest.get(2..)?);
    let rest = rest.strip_prefix('-')?;
    let (day, rest) = (rest.get(..2)?, rest.get(2..)?);
    (two(month, 1, 12) && two(day, 1, 31)).then_some(rest)
}

fn is_date(s: &str) -> bool {
    date_part(s).is_some_and(timezone)
}

fn is_date_time(s: &str) -> bool {
    let Some(rest) = date_part(s).and_then(|r| r.strip_prefix('T')) else {
        return false;
    };
    let (Some(h), Some(m), Some(sec)) = (rest.get(..2), rest.get(3..5), rest.get(6..8)) else {
        return false;
    };
    if rest.as_bytes()[2] != b':' || rest.as_bytes()[5] != b':' {
        return false;
    }
    let mut tail = &rest[8..];
    if let Some(frac) = tail.strip_prefix('.') {
        let n = frac.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 {
            return false;
        }
        tail = &frac[n..];
    }
    let midnight = h == "24" && m == "00" && sec == "00";
    (midnight || two(h, 0, 23)) && two(m, 0, 59) && two(sec, 0, 59) && timezone(tail)
}

/// `-?P(nY)?(nM)?(nD)?(T(nH)?(nM)?(n(.n)?S)?)?` with at least one part, and
/// at least one after a `T`.
fn is_duration(s: &str) -> bool {
    let Some(body) = s.strip_prefix('-').unwrap_or(s).strip_prefix('P') else {
        return false;
    };
    let (date, time) = match body.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (body, None),
    };
    let Some(date_parts) = parts(date, &['Y', 'M', 'D'], false) else {
        return false;
    };
    let time_parts = match time {
        None => 0,
        Some(t) => match parts(t, &['H', 'M', 'S'], true) {
            Some(0) | None => return false,
            Some(n) => n,
        },
    };
    date_parts + time_parts > 0
}

/// How many `<number><designator>` parts `s` holds, designators in the
/// order given, each at most once; only the last may be a decimal when
/// `last_decimal`. `None` when `s` is not such a list.
fn parts(s: &str, designators: &[char], last_decimal: bool) -> Option<usize> {
    let mut rest = s;
    let mut n = 0;
    let mut next = 0;
    while !rest.is_empty() {
        let end = rest.find(|c: char| !c.is_ascii_digit() && c != '.')?;
        let (num, after) = rest.split_at(end);
        let d = after.chars().next()?;
        let at = designators[next..].iter().position(|x| *x == d)? + next;
        let decimal_ok = last_decimal && at == designators.len() - 1;
        let ok = if decimal_ok {
            is_decimal(num) && !num.starts_with(['+', '-'])
        } else {
            digits(num)
        };
        if !ok {
            return None;
        }
        n += 1;
        next = at + 1;
        rest = &after[d.len_utf8()..];
    }
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_and_decimals() {
        for ok in ["0", "-3", "+7", "0042"] {
            assert!(is_integer(ok), "{ok}");
        }
        for bad in ["", "-", "1.0", "1e3", " 1"] {
            assert!(!is_integer(bad), "{bad}");
        }
        for ok in ["1.5", ".5", "5.", "-2", "+0.0"] {
            assert!(is_decimal(ok), "{ok}");
        }
        for bad in [".", "", "1,5", "1e3", "--1"] {
            assert!(!is_decimal(bad), "{bad}");
        }
    }

    #[test]
    fn dates_and_times() {
        for ok in [
            "2026-01-31",
            "-0044-03-15",
            "12026-01-01",
            "2026-01-31+14:00",
        ] {
            assert!(is_date(ok), "{ok}");
        }
        for bad in [
            "2026-1-31",
            "26-01-31",
            "2026-00-10",
            "2026-01-31T",
            "2026-01-31+15:00",
        ] {
            assert!(!is_date(bad), "{bad}");
        }
        for ok in ["2026-01-31T24:00:00", "2026-01-31T23:59:59.999Z"] {
            assert!(is_date_time(ok), "{ok}");
        }
        for bad in [
            "2026-01-31T24:00:01",
            "2026-01-31T10:00",
            "2026-01-31T10:00:00.",
            "2026-01-31T1a:00:00",
        ] {
            assert!(!is_date_time(bad), "{bad}");
        }
    }

    #[test]
    fn durations() {
        for ok in ["P1Y", "P1M", "PT1M", "P1DT1S", "PT1.25S", "-P0D"] {
            assert!(is_duration(ok), "{ok}");
        }
        for bad in [
            "P", "PT", "P1D T1H", "P1M1Y", "P1Y1Y", "PT1.5M", "P-1D", "P1DT",
        ] {
            assert!(!is_duration(bad), "{bad}");
        }
    }
}
