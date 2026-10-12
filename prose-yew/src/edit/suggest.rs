//! Filtering of vocabulary suggestions. Pure, so it is tested on the host.
use super::config::Suggestion;

pub(super) fn strip(s: &str) -> String {
    let lower = s.to_lowercase();
    lower
        .strip_prefix("odrl:")
        .map(str::to_string)
        .unwrap_or(lower)
}

/// Case-insensitive match on value and label after stripping `odrl:`:
/// prefix matches first, then substring matches, then matches in the hint
/// text, each in list order. Groups keep the order in which they first
/// appear, and the ranking applies within each group. An empty query gives
/// the first `max` options.
pub fn filter_suggestions<'a>(
    options: &'a [Suggestion],
    query: &str,
    max: usize,
) -> Vec<&'a Suggestion> {
    let q = strip(query.trim());
    if q.is_empty() {
        return options.iter().take(max).collect();
    }
    // Per group, in first-seen order: (prefix, substring, hint) matches.
    type Ranks<'a> = [Vec<&'a Suggestion>; 3];
    let mut groups: Vec<(Option<&str>, Ranks<'a>)> = vec![];
    for o in options {
        let (v, l) = (strip(&o.value), strip(&o.label));
        let rank = if v.starts_with(&q) || l.starts_with(&q) {
            0
        } else if v.contains(&q) || l.contains(&q) {
            1
        } else if o
            .hint
            .as_ref()
            .is_some_and(|h| h.to_lowercase().contains(&q))
        {
            2
        } else {
            continue;
        };
        let g = o.group.as_deref();
        let at = match groups.iter().position(|(k, _)| *k == g) {
            Some(i) => i,
            None => {
                groups.push((g, Default::default()));
                groups.len() - 1
            }
        };
        groups[at].1[rank].push(o);
    }
    groups
        .into_iter()
        .flat_map(|(_, ranks)| ranks.into_iter().flatten())
        .take(max)
        .collect()
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

    fn grouped(value: &str, label: &str, hint: Option<&str>, group: Option<&str>) -> Suggestion {
        let mut s = Suggestion::new(value, label, hint);
        s.group = group.map(|g| g.to_string().into());
        s
    }

    #[test]
    fn filter_matches_hint_last() {
        let o = vec![
            grouped(
                "ex:train",
                "Train",
                Some("training data for a machine-learning model"),
                None,
            ),
            grouped("modelize", "modelize", None, None),
            grouped("use", "use", Some("use the asset"), None),
        ];
        let got: Vec<&str> = filter_suggestions(&o, "model", 8)
            .iter()
            .map(|s| s.value.as_str())
            .collect();
        assert_eq!(got, ["modelize", "ex:train"]);
        // A hint match never beats a value or label match.
        let got: Vec<&str> = filter_suggestions(&o, "t", 8)
            .iter()
            .map(|s| s.value.as_str())
            .collect();
        assert_eq!(got, ["ex:train", "use"]);
    }

    #[test]
    fn filter_keeps_group_order() {
        let p = Some("From profile: Research");
        let o = vec![
            grouped("ex:train", "Train", None, p),
            grouped("ex:fineTune", "Fine-tune", None, p),
            grouped("transfer", "transfer", None, None),
            grouped("distribute", "distribute", None, None),
        ];
        let got: Vec<&str> = filter_suggestions(&o, "t", 8)
            .iter()
            .map(|s| s.value.as_str())
            .collect();
        // The profile's group first; prefix matches first inside each group.
        assert_eq!(got, ["ex:train", "ex:fineTune", "transfer", "distribute"]);
        assert_eq!(filter_suggestions(&o, "t", 3).len(), 3);
        // An empty query lists the options in their order.
        let all: Vec<&str> = filter_suggestions(&o, "", 8)
            .iter()
            .map(|s| s.value.as_str())
            .collect();
        assert_eq!(all, ["ex:train", "ex:fineTune", "transfer", "distribute"]);
    }
}
