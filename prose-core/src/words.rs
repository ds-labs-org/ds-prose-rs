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
    generic_phrase(term).render(false)
}

/// A verb that agrees in number with its subject. A phrase writes it as a
/// slot, `{be}`, `{have}`, `{do}`, `{contain}` or `{match}`, and
/// [`Phrase::render`] fills the slot for the number: one place conjugates
/// every built-in phrase and every profile label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verb {
    Be,
    Have,
    Do,
    Contain,
    Match,
}

impl Verb {
    /// (slot name, singular, plural).
    const ALL: [(Verb, &'static str, &'static str, &'static str); 5] = [
        (Verb::Be, "be", "is", "are"),
        (Verb::Have, "have", "has", "have"),
        (Verb::Do, "do", "does", "do"),
        (Verb::Contain, "contain", "contains", "contain"),
        (Verb::Match, "match", "matches", "match"),
    ];

    fn of_slot(name: &str) -> Option<Verb> {
        Verb::ALL.iter().find(|v| v.1 == name).map(|v| v.0)
    }

    /// The verb a label opens with, written in the singular. "are" is not
    /// one: a label that opens with it keeps its words.
    fn of_word(word: &str) -> Option<Verb> {
        Verb::ALL.iter().find(|v| v.2 == word).map(|v| v.0)
    }

    fn form(self, plural: bool) -> &'static str {
        let v = Verb::ALL.iter().find(|v| v.0 == self).expect("every verb");
        if plural { v.3 } else { v.2 }
    }
}

/// A verb phrase: an optional leading verb that agrees in number, then the
/// words after it, which never change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Phrase {
    verb: Option<Verb>,
    rest: String,
}

impl Phrase {
    /// A built-in template: `{be} before the start of`. Only a slot at the
    /// very start is read; what follows is text.
    pub(crate) fn parse(template: &str) -> Phrase {
        let slot = template
            .strip_prefix('{')
            .and_then(|t| t.split_once('}'))
            .and_then(|(name, rest)| Verb::of_slot(name).map(|v| (v, rest)));
        match slot {
            Some((verb, rest)) => Phrase {
                verb: Some(verb),
                rest: rest.trim_start().to_string(),
            },
            None => Phrase {
                verb: None,
                rest: template.to_string(),
            },
        }
    }

    /// A profile operator label, already lower-cased. A label that opens
    /// with a verb in the singular ("is", "has", "does", "contains",
    /// "matches") has that verb conjugated. "are" and any other opening
    /// keep their words: "are near" stays, and "earlier than" follows "is"
    /// or "are". The label is text, never a template.
    pub(crate) fn from_label(label: &str) -> Phrase {
        let (first, rest) = label.split_once(' ').unwrap_or((label, ""));
        if let Some(verb) = Verb::of_word(first) {
            return Phrase {
                verb: Some(verb),
                rest: rest.to_string(),
            };
        }
        if first == "are" {
            return Phrase {
                verb: None,
                rest: label.to_string(),
            };
        }
        Phrase {
            verb: Some(Verb::Be),
            rest: label.to_string(),
        }
    }

    /// The phrase for a subject of the given number.
    pub(crate) fn render(&self, plural: bool) -> String {
        match (self.verb, self.rest.is_empty()) {
            (Some(v), true) => v.form(plural).to_string(),
            (Some(v), false) => format!("{} {}", v.form(plural), self.rest),
            (None, _) => self.rest.clone(),
        }
    }
}

/// The built-in operator phrases, as templates. Private, and one value so a
/// host override can replace it later.
struct PhraseTable {
    /// Operator -> template, for any left operand.
    generic: &'static [(&'static str, &'static str)],
    /// The template for an operator this table does not list; `{op}` is
    /// the operator's name, humanised.
    unknown: &'static str,
    /// A point in time compared with a point in time.
    instant: &'static [(&'static str, &'static str)],
    /// A point in time compared with a whole day.
    day: &'static [(&'static str, &'static str)],
}

const PHRASES: PhraseTable = PhraseTable {
    generic: &[
        ("eq", "{be} equal to"),
        ("neq", "{be} not equal to"),
        ("gt", "{be} greater than"),
        ("gteq", "{be} at least"),
        ("lt", "{be} less than"),
        ("lteq", "{be} at most"),
        ("isA", "{be} a kind of"),
        ("hasPart", "{have} as a part"),
        ("isPartOf", "{be} part of"),
        ("isAllOf", "{be} all of"),
        ("isAnyOf", "{be} any of"),
        ("isNoneOf", "{be} none of"),
    ],
    unknown: "{be} related by {op} to",
    instant: &[
        ("lt", "{be} before"),
        ("lteq", "{be} on or before"),
        ("gt", "{be} after"),
        ("gteq", "{be} on or after"),
    ],
    day: &[
        ("lt", "{be} before the start of"),
        ("lteq", "{be} on or before the end of"),
        ("gt", "{be} after the end of"),
        ("gteq", "{be} on or after the start of"),
    ],
};

fn find(table: &'static [(&'static str, &'static str)], op: &str) -> Option<&'static str> {
    table.iter().find(|(o, _)| *o == op).map(|(_, t)| *t)
}

/// The phrase for `term` that does not depend on its left operand.
fn generic_phrase(term: &str) -> Phrase {
    let local = local_name(term);
    match find(PHRASES.generic, local) {
        Some(t) => Phrase::parse(t),
        None => Phrase::parse(&PHRASES.unknown.replacen("{op}", &humanise(local), 1)),
    }
}

/// Left operands whose noun phrase is plural: "the date and time" is two
/// nouns, so its operator reads "are before", not "is before".
pub(crate) const PLURAL_OPERANDS: &[&str] = &["dateTime"];

/// Whether `term`'s built-in noun phrase is plural.
pub(crate) fn plural_operand(term: &str) -> bool {
    PLURAL_OPERANDS.contains(&local_name(term))
}

/// The verb phrase after `left`, for the number of its noun phrase: the
/// generic phrase, or a point in time's own ("is before" rather than "is
/// less than", the start or end of a whole day). Only a date and time is a
/// point in time: a time interval, a delay period or an elapsed time is a
/// quantity and keeps the quantity wording.
pub(crate) fn built_in_phrase(left: &str, op: &str, plural: bool, shape: ValueShape) -> String {
    let table = match (local_name(left) == "dateTime", shape) {
        (true, ValueShape::DateOnly) => Some(PHRASES.day),
        (true, _) => Some(PHRASES.instant),
        (false, _) => None,
    };
    table
        .and_then(|t| find(t, local_name(op)))
        .map_or_else(|| generic_phrase(op), Phrase::parse)
        .render(plural)
}

const XSD_DATE: &str = "http://www.w3.org/2001/XMLSchema#date";

/// Whether a literal's datatype, expanded as far as the policy's prefixes
/// go, is xsd:date: the full IRI, or `xsd:`/`xs:` left unbound.
fn is_date_type(datatype: &str) -> bool {
    matches!(datatype, XSD_DATE | "xsd:date" | "xs:date")
}

/// Exactly `YYYY-MM-DD`: ASCII digits, no zone, no range check.
fn is_plain_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            _ => c.is_ascii_digit(),
        })
}

/// `YYYY-MM-DD` followed by nothing, `Z`, or `+hh:mm`/`-hh:mm`. Shape only.
fn is_date_with_zone(s: &str) -> bool {
    let Some(date) = s.get(..10) else {
        return false;
    };
    if !is_plain_date(date) {
        return false;
    }
    match &s[10..] {
        "" | "Z" => true,
        zone => {
            let z = zone.as_bytes();
            z.len() == 6
                && (z[0] == b'+' || z[0] == b'-')
                && z[1].is_ascii_digit()
                && z[2].is_ascii_digit()
                && z[3] == b':'
                && z[4].is_ascii_digit()
                && z[5].is_ascii_digit()
        }
    }
}

/// Whether a right operand is a date without a time of day. `datatype` is
/// the expanded `@type`, `None` for a plain string or an untyped `@value`.
/// Without a datatype the value must be exactly `YYYY-MM-DD`; with xsd:date
/// the value must also look like a date, since editing a typed literal can
/// leave a time of day in it. Nothing is computed: the zone, if any, is not
/// read and the date is never rewritten.
fn date_only(value: &str, datatype: Option<&str>) -> bool {
    match datatype {
        None => is_plain_date(value),
        Some(t) => is_date_type(t) && is_date_with_zone(value),
    }
}

/// What a constraint's one right operand is, as far as wording goes. Compute
/// it once with [`ValueShape::of`]; the reader, the editor sentence and the
/// editor's operator dropdown all read the same shape, so they agree.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ValueShape {
    /// A date with a time of day: xsd:dateTime or xsd:dateTimeStamp, or an
    /// untyped string shaped `YYYY-MM-DDThh:mm(:ss(.s)?)?(zone)?`.
    Instant,
    /// A whole day: `YYYY-MM-DD` untyped, or xsd:date with a date-shaped
    /// value (an optional zone is allowed).
    DateOnly,
    /// An xsd:duration, or an untyped string that reads as an ISO 8601
    /// duration.
    Duration,
    /// Anything else: a number, a reference, several values, no value.
    #[default]
    Other,
}

impl ValueShape {
    /// The shape of one literal. `datatype` is the `@type` expanded as far
    /// as the policy's prefixes go, `None` for a plain string or an untyped
    /// `@value`. Shape only: nothing is computed and no range is checked.
    pub fn of(value: &str, datatype: Option<&str>) -> ValueShape {
        if date_only(value, datatype) {
            return ValueShape::DateOnly;
        }
        match datatype {
            Some(t) if is_duration_type(t) => {
                if duration(value).is_some() {
                    ValueShape::Duration
                } else {
                    ValueShape::Other
                }
            }
            Some(t) if is_date_time_type(t) => ValueShape::Instant,
            Some(_) => ValueShape::Other,
            None if duration(value).is_some() => ValueShape::Duration,
            None if is_instant(value) => ValueShape::Instant,
            None => ValueShape::Other,
        }
    }
}

/// xsd:dateTime or xsd:dateTimeStamp, the full IRI or `xsd:`/`xs:` unbound.
fn is_date_time_type(datatype: &str) -> bool {
    let local = datatype
        .strip_prefix("http://www.w3.org/2001/XMLSchema#")
        .or_else(|| datatype.strip_prefix("xsd:"))
        .or_else(|| datatype.strip_prefix("xs:"));
    matches!(local, Some("dateTime" | "dateTimeStamp"))
}

/// `YYYY-MM-DDThh:mm(:ss(.s)?)?` then nothing, `Z` or `+hh:mm`/`-hh:mm`.
fn is_instant(s: &str) -> bool {
    let Some(date) = s.get(..10) else {
        return false;
    };
    let Some(rest) = s[10..].strip_prefix('T') else {
        return false;
    };
    if !is_plain_date(date) {
        return false;
    }
    let digits = |t: &str| t.len() == 2 && t.bytes().all(|c| c.is_ascii_digit());
    let (Some(hh), Some(rest)) = (rest.get(..2), rest.get(2..)) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(':') else {
        return false;
    };
    let (Some(mm), mut rest) = (rest.get(..2), rest.get(2..).unwrap_or("")) else {
        return false;
    };
    if !digits(hh) || !digits(mm) {
        return false;
    }
    if let Some(sec) = rest.strip_prefix(':') {
        let Some(ss) = sec.get(..2) else {
            return false;
        };
        if !digits(ss) {
            return false;
        }
        rest = &sec[2..];
        if let Some(frac) = rest.strip_prefix('.') {
            let n = frac.bytes().take_while(u8::is_ascii_digit).count();
            if n == 0 {
                return false;
            }
            rest = &frac[n..];
        }
    }
    match rest {
        "" | "Z" => true,
        zone => {
            let z = zone.as_bytes();
            z.len() == 6
                && (z[0] == b'+' || z[0] == b'-')
                && digits(&zone[1..3])
                && z[3] == b':'
                && digits(&zone[4..6])
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::words_ctx::{OperatorQuery, operator_phrase};

    #[test]
    fn operator_for_agrees_with_the_date_and_time() {
        let cases = [
            ("lt", "are before"),
            ("lteq", "are on or before"),
            ("gt", "are after"),
            ("gteq", "are on or after"),
            ("eq", "are equal to"),
            ("neq", "are not equal to"),
            ("isAllOf", "are all of"),
            ("isAnyOf", "are any of"),
            ("isNoneOf", "are none of"),
            ("isPartOf", "are part of"),
            ("hasPart", "have as a part"),
            ("ex:near", "are related by near to"),
            ("odrl:lt", "are before"),
            ("http://www.w3.org/ns/odrl/2/lt", "are before"),
        ];
        for (op, want) in cases {
            assert_eq!(
                operator_phrase(&OperatorQuery::new("dateTime", op)),
                want,
                "{op}"
            );
        }
        assert_eq!(
            operator_phrase(&OperatorQuery::new("odrl:dateTime", "lt")),
            "are before"
        );
    }

    #[test]
    fn other_operands_stay_singular() {
        assert_eq!(
            operator_phrase(&OperatorQuery::new("timeInterval", "lt")),
            "is less than"
        );
        assert_eq!(
            operator_phrase(&OperatorQuery::new("delayPeriod", "eq")),
            "is equal to"
        );
        assert_eq!(
            operator_phrase(&OperatorQuery::new("count", "lteq")),
            "is at most"
        );
        assert_eq!(
            operator_phrase(&OperatorQuery::new("elapsedTime", "gteq")),
            "is at least"
        );
    }

    #[test]
    fn duration_operands_read_as_quantities() {
        let cases = [
            ("lt", "is less than"),
            ("lteq", "is at most"),
            ("gt", "is greater than"),
            ("gteq", "is at least"),
            ("eq", "is equal to"),
        ];
        for left in [
            "timeInterval",
            "delayPeriod",
            "elapsedTime",
            "odrl:timeInterval",
        ] {
            for (op, want) in cases {
                for got in [
                    operator_phrase(&OperatorQuery::new(left, op)),
                    operator_phrase(&OperatorQuery::new(left, op).value(ValueShape::DateOnly)),
                ] {
                    assert_eq!(got, want, "{left} {op}");
                    for bad in ["before", "after", "start of", "end of", "are "] {
                        assert!(!got.contains(bad), "{left} {op}: {got}");
                    }
                }
            }
        }
    }

    #[test]
    fn operator_is_unchanged() {
        assert_eq!(operator("lt"), "is less than");
    }

    #[test]
    fn date_only_detects_dates_without_time() {
        let xsd = "http://www.w3.org/2001/XMLSchema#";
        assert!(date_only("2026-12-31", None));
        for v in [
            "2026-12-31T10:00:00Z",
            "2026-12-31Z",
            "2026-1-31",
            "20261231",
            "P30D",
            "",
        ] {
            assert!(!date_only(v, None), "{v}");
        }
        for dt in [format!("{xsd}date"), "xsd:date".into(), "xs:date".into()] {
            for v in ["2026-12-31", "2026-12-31Z", "2026-12-31+02:00"] {
                assert!(date_only(v, Some(&dt)), "{dt} {v}");
            }
            assert!(!date_only("2026-12-31T10:00:00Z", Some(&dt)), "{dt}");
        }
        assert!(!date_only("2026-12-31", Some(&format!("{xsd}dateTime"))));
        assert!(!date_only("2026-12-31", Some(&format!("{xsd}string"))));
    }

    #[test]
    fn operator_for_date_names_the_start_or_end_of_the_day() {
        let cases = [
            ("lt", "are before the start of"),
            ("lteq", "are on or before the end of"),
            ("gt", "are after the end of"),
            ("gteq", "are on or after the start of"),
            ("eq", "are equal to"),
            ("neq", "are not equal to"),
        ];
        for (op, want) in cases {
            assert_eq!(
                operator_phrase(&OperatorQuery::new("dateTime", op).value(ValueShape::DateOnly)),
                want,
                "{op}"
            );
        }
        assert_eq!(
            operator_phrase(&OperatorQuery::new("timeInterval", "lt").value(ValueShape::DateOnly)),
            "is less than"
        );
        assert_eq!(
            operator_phrase(&OperatorQuery::new("count", "lt").value(ValueShape::DateOnly)),
            "is less than"
        );
    }

    #[test]
    fn a_template_is_conjugated_by_number_in_one_place() {
        let cases = [
            (
                "{be} before the start of",
                "is before the start of",
                "are before the start of",
            ),
            ("{have} as a part", "has as a part", "have as a part"),
            ("{do} not overlap", "does not overlap", "do not overlap"),
            ("{contain}", "contains", "contain"),
            ("{match} window", "matches window", "match window"),
            (
                "{be} related by near to",
                "is related by near to",
                "are related by near to",
            ),
            ("on or before", "on or before", "on or before"),
        ];
        for (template, one, many) in cases {
            let phrase = Phrase::parse(template);
            assert_eq!(phrase.render(false), one, "{template}");
            assert_eq!(phrase.render(true), many, "{template}");
        }
    }

    #[test]
    fn a_profile_label_keeps_its_own_words_and_is_never_read_as_a_template() {
        let cases = [
            ("is within region", "is within region", "are within region"),
            ("has overlap with", "has overlap with", "have overlap with"),
            ("does not overlap", "does not overlap", "do not overlap"),
            ("contains", "contains", "contain"),
            ("matches window", "matches window", "match window"),
            ("are near", "are near", "are near"),
            ("earlier than", "is earlier than", "are earlier than"),
            ("{be} odd", "is {be} odd", "are {be} odd"),
            ("is {have}", "is {have}", "are {have}"),
        ];
        for (label, one, many) in cases {
            let phrase = Phrase::from_label(label);
            assert_eq!(phrase.render(false), one, "{label}");
            assert_eq!(phrase.render(true), many, "{label}");
        }
    }
}
