//! Language-tagged text, and the hygiene applied to every profile string.

/// One text in several languages: (language tag, text) pairs in the order the
/// profile gave them. Tags are kept lower-cased.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LangText(Vec<(Option<String>, String)>);

impl LangText {
    /// The text in the best language for `prefs` (most preferred first).
    pub fn pick(&self, prefs: &[String]) -> Option<&str> {
        let _ = prefs;
        None
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every (language tag, text) pair, in the profile's order.
    pub fn iter(&self) -> impl Iterator<Item = (Option<&str>, &str)> {
        self.0.iter().map(|(l, t)| (l.as_deref(), t.as_str()))
    }

    /// Adds a text; an exact duplicate is dropped.
    pub(crate) fn push(&mut self, lang: Option<&str>, text: String) {
        let lang = lang.map(str::to_ascii_lowercase);
        if !self.0.iter().any(|(l, t)| *l == lang && *t == text) {
            self.0.push((lang, text));
        }
    }
}

/// `s` as plain text of at most `max_chars` characters.
pub(crate) fn plain(s: &str, max_chars: usize) -> String {
    let _ = max_chars;
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(pairs: &[(Option<&str>, &str)]) -> LangText {
        let mut t = LangText::default();
        for (l, s) in pairs {
            t.push(*l, s.to_string());
        }
        t
    }

    fn prefs(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn pick_exact_then_primary_subtag_then_en_then_untagged_then_first() {
        let t = text(&[
            (Some("fr"), "Entraîner"),
            (Some("de"), "Trainieren"),
            (Some("de-CH"), "Trainiere"),
            (Some("en"), "Train"),
            (None, "train"),
        ]);
        assert_eq!(t.pick(&prefs(&["de-CH"])), Some("Trainiere"));
        assert_eq!(t.pick(&prefs(&["DE-ch"])), Some("Trainiere"));
        assert_eq!(t.pick(&prefs(&["de-AT"])), Some("Trainieren"));
        assert_eq!(t.pick(&prefs(&["it", "fr"])), Some("Entraîner"));
        assert_eq!(t.pick(&prefs(&["it"])), Some("Train"));
        assert_eq!(t.pick(&[]), Some("Train"));

        let untagged = text(&[(Some("fr"), "Entraîner"), (None, "train")]);
        assert_eq!(untagged.pick(&prefs(&["it"])), Some("train"));

        let first = text(&[(Some("fr"), "Entraîner"), (Some("de"), "Trainieren")]);
        assert_eq!(first.pick(&prefs(&["it"])), Some("Entraîner"));
        assert_eq!(first.pick(&prefs(&["en-GB"])), Some("Entraîner"));

        let en_gb = text(&[(Some("fr"), "Entraîner"), (Some("en-gb"), "Train")]);
        assert_eq!(en_gb.pick(&prefs(&["it"])), Some("Train"));

        assert_eq!(LangText::default().pick(&prefs(&["en"])), None);
        assert!(LangText::default().is_empty());
    }

    #[test]
    fn push_lowercases_tags_and_drops_exact_duplicates() {
        let t = text(&[(Some("EN"), "Train"), (Some("en"), "Train"), (None, "x")]);
        let all: Vec<_> = t.iter().collect();
        assert_eq!(all, vec![(Some("en"), "Train"), (None, "x")]);
    }

    #[test]
    fn strips_tags_decodes_entities_collapses_ws() {
        assert_eq!(
            plain(
                "Only the <code>gteq</code> and <code>gt</code> operators.<br />Small counts.",
                4_000
            ),
            "Only the gteq and gt operators. Small counts."
        );
        assert_eq!(
            plain(
                "a &amp; b &lt;script&gt; &quot;c&quot; &#39;d&#x27; &nbsp;e",
                4_000
            ),
            "a & b <script> \"c\" 'd' e"
        );
        assert_eq!(plain("  one\n\t two   three ", 4_000), "one two three");
        // A `<` that does not open a tag is text.
        assert_eq!(plain("a < b and 1<2", 4_000), "a < b and 1<2");
        // An unknown entity is kept as written.
        assert_eq!(plain("&unknown; &#xZZ;", 4_000), "&unknown; &#xZZ;");
        // An unterminated tag is dropped, not shown half-way.
        assert_eq!(plain("text <a href=\"x", 4_000), "text");
    }

    #[test]
    fn strips_bidi_zero_width_controls() {
        let s = "\u{202E}evil\u{202C} \u{2066}x\u{2069}\u{200B}y\u{FEFF}\u{0007}z\u{200F}\u{00AD}";
        assert_eq!(plain(s, 4_000), "evil xyz");
        // A bidi control written as an entity is removed too.
        assert_eq!(plain("a&#x202E;b", 4_000), "ab");
        // Line breaks and tabs are whitespace, not dropped.
        assert_eq!(plain("a\r\nb", 4_000), "a b");
    }

    #[test]
    fn truncates_at_limit_with_ellipsis() {
        assert_eq!(plain("abcdef", 6), "abcdef");
        assert_eq!(plain("abcdefg", 6), "abcde\u{2026}");
        assert_eq!(plain("abcd efgh", 6), "abcd\u{2026}");
        let long = "\u{e9}".repeat(300);
        let cut = plain(&long, 200);
        assert_eq!(cut.chars().count(), 200);
        assert!(cut.ends_with('\u{2026}'));
        assert_eq!(plain("abc", 0), "");
    }
}
