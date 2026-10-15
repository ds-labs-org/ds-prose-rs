//! Wording: turning ODRL terms into English fragments.

const ODRL_NS: &str = "http://www.w3.org/ns/odrl/2/";

/// The local name of an ODRL term: `odrl:use`, `http://www.w3.org/ns/odrl/2/use`
/// and a bare `use` all give `use`. Terms from other vocabularies keep their
/// last path/fragment segment.
pub fn local_name(term: &str) -> &str {
    if let Some(rest) = term.strip_prefix(ODRL_NS) {
        return rest;
    }
    if let Some(rest) = term.strip_prefix("odrl:") {
        return rest;
    }
    if term.contains("://") {
        return term.rsplit(['#', '/']).next().unwrap_or(term);
    }
    term.rsplit_once(':').map_or(term, |(_, local)| local)
}

/// A short name for an IRI: its last segment after a `/`, `#` or `:`, with
/// trailing ones ignored, and the segment before it too when the last has
/// no letter; the whole IRI if that leaves nothing.
/// `http://example.com/asset:9898.movie` gives `9898.movie`,
/// `http://example.com/person:88` gives `person:88`.
pub fn short_iri(iri: &str) -> &str {
    let trimmed = iri.trim_end_matches(['/', '#', ':']);
    let cut = |s: &str| s.rfind(['/', '#', ':']).map_or(0, |i| i + 1);
    let mut start = cut(trimmed);
    if !trimmed[start..].chars().any(char::is_alphabetic) && start > 0 {
        let before = cut(&trimmed[..start - 1]);
        if before < start - 1 {
            start = before;
        }
    }
    match &trimmed[start..] {
        "" => iri,
        short => short,
    }
}

/// `dateTime` -> `date time`, `payment_amount` -> `payment amount`.
pub fn humanise(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if c == '-' || c == '_' {
            out.push(' ');
            prev_lower = false;
        } else if c.is_uppercase() && prev_lower {
            out.push(' ');
            out.extend(c.to_lowercase());
            prev_lower = false;
        } else {
            out.extend(c.to_lowercase());
            prev_lower = c.is_lowercase() || c.is_ascii_digit();
        }
    }
    out
}

/// The left operands this module has a noun phrase for: (term, phrase).
pub(crate) const OPERANDS: &[(&str, &str)] = &[
    ("dateTime", "the date and time"),
    ("elapsedTime", "the elapsed time"),
    ("meteredTime", "the metered time"),
    ("delayPeriod", "the delay period"),
    ("spatial", "the location"),
    ("spatialCoordinates", "the coordinates"),
    ("payAmount", "the payment amount"),
    ("count", "the number of times used"),
    ("deliveryChannel", "the delivery channel"),
    ("fileFormat", "the file format"),
    ("industry", "the industry"),
    ("language", "the language"),
    ("media", "the media"),
    ("product", "the product"),
    ("purpose", "the purpose"),
    ("recipient", "the recipient"),
    ("systemDevice", "the system or device"),
    ("unitOfCount", "the unit of count"),
    ("version", "the version"),
    ("virtualLocation", "the virtual location"),
    ("event", "the event"),
    ("percentage", "the percentage"),
    ("resolution", "the resolution"),
    ("timeInterval", "the time interval"),
];

/// A left operand as a noun phrase.
pub fn operand(term: &str) -> String {
    let local = local_name(term);
    match OPERANDS.iter().find(|(t, _)| *t == local) {
        Some((_, phrase)) => (*phrase).to_string(),
        None => format!("the {}", humanise(local)),
    }
}

/// An operator as a verb phrase that follows its left operand.
pub fn operator(term: &str) -> String {
    let local = local_name(term);
    match local {
        "eq" => "is equal to",
        "neq" => "is not equal to",
        "gt" => "is greater than",
        "gteq" => "is at least",
        "lt" => "is less than",
        "lteq" => "is at most",
        "isA" => "is a kind of",
        "hasPart" => "has as a part",
        "isPartOf" => "is part of",
        "isAllOf" => "is all of",
        "isAnyOf" => "is any of",
        "isNoneOf" => "is none of",
        _ => return format!("is related by {} to", humanise(local)),
    }
    .to_string()
}

/// Operators read better with a different phrase for time: "the date and
/// time is before X" rather than "is less than X".
pub fn operator_for(left: &str, op: &str) -> String {
    let temporal = matches!(
        local_name(left),
        "dateTime" | "timeInterval" | "delayPeriod"
    );
    if temporal {
        match local_name(op) {
            "lt" => return "is before".into(),
            "lteq" => return "is on or before".into(),
            "gt" => return "is after".into(),
            "gteq" => return "is on or after".into(),
            _ => {}
        }
    }
    operator(op)
}

/// An action as a base-form verb phrase ("use", "attribute", "grant use").
pub fn action(term: &str) -> String {
    let local = local_name(term);
    match local {
        "grantUse" => "grant use of".to_string(),
        "nextPolicy" => "pass on the next policy for".to_string(),
        "textToSpeech" => "convert to speech".to_string(),
        "attribute" => "attribute".to_string(),
        "attribution" => "attribute".to_string(),
        _ => humanise(local),
    }
}

/// "a", "a and b", "a, b and c".
pub fn join(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {}", init.join(", "), last),
    }
}

const XSD_DURATION: &str = "http://www.w3.org/2001/XMLSchema#duration";

/// Whether a literal's datatype, expanded as far as the policy's prefixes
/// go, is xsd:duration: the full IRI, or `xsd:`/`xs:` left unbound.
pub(crate) fn is_duration_type(datatype: &str) -> bool {
    matches!(datatype, XSD_DURATION | "xsd:duration" | "xs:duration")
}

/// An ISO 8601 / xsd:duration in words: `PT24H` gives "24 hours",
/// `P1Y6M` "1 year and 6 months", `-P3D` "minus 3 days". Zero parts are
/// left out unless all are zero (`P0D` gives "0 days"); only seconds may
/// be fractional, and are kept as written. `None` unless `s` is strictly
/// `-?P(nY)?(nM)?(nW)?(nD)?(T(nH)?(nM)?(n(.n)?S)?)?` with at least one
/// part, and one after a `T`.
pub(crate) fn duration(s: &str) -> Option<String> {
    let (minus, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let body = body.strip_prefix('P')?;
    let (date, time) = match body.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (body, None),
    };
    let mut parts = duration_parts(
        date,
        &[('Y', "year"), ('M', "month"), ('W', "week"), ('D', "day")],
    )?;
    if let Some(t) = time {
        let time_parts = duration_parts(t, &[('H', "hour"), ('M', "minute"), ('S', "second")])?;
        if time_parts.is_empty() {
            return None;
        }
        parts.extend(time_parts);
    }
    let last = parts.last()?.clone();
    let nonzero: Vec<String> = parts
        .into_iter()
        .filter(|(n, _)| n.bytes().any(|b| b.is_ascii_digit() && b != b'0'))
        .map(|(n, unit)| match n.as_str() {
            "1" => format!("1 {unit}"),
            _ => format!("{n} {unit}s"),
        })
        .collect();
    if nonzero.is_empty() {
        return Some(format!("{} {}s", last.0, last.1));
    }
    let words = join(&nonzero);
    Some(if minus {
        format!("minus {words}")
    } else {
        words
    })
}

/// The `<number><designator>` parts of `s`, designators in the order given,
/// each at most once, as (number without leading zeros, unit); the number
/// before `S` may have a fraction. `None` when `s` is not such a list.
fn duration_parts(s: &str, units: &[(char, &'static str)]) -> Option<Vec<(String, &'static str)>> {
    let mut out = Vec::new();
    let mut rest = s;
    let mut next = 0;
    while !rest.is_empty() {
        let end = rest.find(|c: char| !c.is_ascii_digit() && c != '.')?;
        let (num, after) = rest.split_at(end);
        let d = after.chars().next()?;
        let at = units[next..].iter().position(|(x, _)| *x == d)? + next;
        let (int, frac) = match num.split_once('.') {
            Some((i, f)) if d == 'S' => (i, Some(f)),
            Some(_) => return None,
            None => (num, None),
        };
        let digits = |x: &str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
        if !digits(int) || frac.is_some_and(|f| !digits(f)) {
            return None;
        }
        let int = match int.trim_start_matches('0') {
            "" => "0",
            n => n,
        };
        let n = match frac {
            Some(f) => format!("{int}.{f}"),
            None => int.to_string(),
        };
        out.push((n, units[at].1));
        next = at + 1;
        rest = &after[1..];
    }
    Some(out)
}
