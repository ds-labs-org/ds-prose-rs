//! Filtering of vocabulary suggestions. Pure, so it is tested on the host.
use super::config::Suggestion;

fn strip(s: &str) -> String {
    let lower = s.to_lowercase();
    lower
        .strip_prefix("odrl:")
        .map(str::to_string)
        .unwrap_or(lower)
}

/// Case-insensitive match on value and label after stripping `odrl:`:
/// prefix matches first, then substring matches, each in list order. An
/// empty query gives the first `max` options.
pub fn filter_suggestions<'a>(
    options: &'a [Suggestion],
    query: &str,
    max: usize,
) -> Vec<&'a Suggestion> {
    let q = strip(query.trim());
    if q.is_empty() {
        return options.iter().take(max).collect();
    }
    let mut prefix = vec![];
    let mut inner = vec![];
    for o in options {
        let (v, l) = (strip(&o.value), strip(&o.label));
        if v.starts_with(&q) || l.starts_with(&q) {
            prefix.push(o);
        } else if v.contains(&q) || l.contains(&q) {
            inner.push(o);
        }
    }
    prefix.into_iter().chain(inner).take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Vec<Suggestion> {
        ["use", "reproduce", "distribute", "odrl:display"]
            .iter()
            .map(|v| Suggestion::new(v, v, None))
            .collect()
    }

    #[test]
    fn empty_query_gives_first_max() {
        let o = opts();
        assert_eq!(filter_suggestions(&o, "", 2).len(), 2);
        assert_eq!(filter_suggestions(&o, "  ", 10).len(), 4);
    }

    #[test]
    fn prefix_before_substring() {
        let o = opts();
        let got: Vec<&str> = filter_suggestions(&o, "d", 8)
            .iter()
            .map(|s| s.value.as_str())
            .collect();
        assert_eq!(got, vec!["distribute", "odrl:display", "reproduce"]);
    }

    #[test]
    fn strips_odrl_prefix_and_ignores_case() {
        let o = opts();
        let got = filter_suggestions(&o, "ODRL:Use", 8);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].value, "use");
        assert_eq!(filter_suggestions(&o, "DISP", 8)[0].value, "odrl:display");
    }

    #[test]
    fn respects_max() {
        let o = opts();
        assert_eq!(filter_suggestions(&o, "u", 1).len(), 1);
    }
}
