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

/// A left operand as a noun phrase.
pub fn operand(term: &str) -> String {
    let local = local_name(term);
    let known = match local {
        "dateTime" => "the date and time",
        "elapsedTime" => "the elapsed time",
        "meteredTime" => "the metered time",
        "delayPeriod" => "the delay period",
        "spatial" => "the location",
        "spatialCoordinates" => "the coordinates",
        "payAmount" => "the payment amount",
        "count" => "the number of times used",
        "deliveryChannel" => "the delivery channel",
        "fileFormat" => "the file format",
        "industry" => "the industry",
        "language" => "the language",
        "media" => "the media",
        "product" => "the product",
        "purpose" => "the purpose",
        "recipient" => "the recipient",
        "systemDevice" => "the system or device",
        "unitOfCount" => "the unit of count",
        "version" => "the version",
        "virtualLocation" => "the virtual location",
        "event" => "the event",
        "percentage" => "the percentage",
        "resolution" => "the resolution",
        "timeInterval" => "the time interval",
        _ => return format!("the {}", humanise(local)),
    };
    known.to_string()
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
