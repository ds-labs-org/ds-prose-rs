//! Language-tagged text, and the hygiene applied to every profile string.

/// One text in several languages: (language tag, text) pairs in the order the
/// profile gave them. Tags are kept lower-cased.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LangText(Vec<(Option<String>, String)>);

impl LangText {
    /// The text in the best language for `prefs` (most preferred first).
    /// Each preference is tried in turn before the next: its exact tag, then
    /// the tag shortened subtag by subtag (`zh-Hant-TW`, `zh-Hant`, `zh`, as
    /// in RFC 4647 lookup), then any tag with the same primary subtag
    /// (`de-AT` for `de-CH`). Then English, then untagged text, then the
    /// first text given.
    pub fn pick(&self, prefs: &[String]) -> Option<&str> {
        self.preferred(prefs)
            .or_else(|| self.tagged(&|l| primary(l) == "en"))
            .or_else(|| {
                self.0
                    .iter()
                    .find(|(l, _)| l.is_none())
                    .map(|(_, t)| t.as_str())
            })
            .or_else(|| self.0.first().map(|(_, t)| t.as_str()))
    }

    /// The text in the best language for `prefs`, as [`LangText::pick`]
    /// tries them, and nothing when none of them is given.
    pub(crate) fn preferred(&self, prefs: &[String]) -> Option<&str> {
        prefs.iter().find_map(|p| {
            let mut range = p.as_str();
            loop {
                if let Some(t) = self.tagged(&|l| l.eq_ignore_ascii_case(range)) {
                    return Some(t);
                }
                match shorter(range) {
                    Some(r) => range = r,
                    None => break,
                }
            }
            self.tagged(&|l| primary(l).eq_ignore_ascii_case(primary(p)))
        })
    }

    /// The first text whose tag `want` accepts.
    fn tagged(&self, want: &dyn Fn(&str) -> bool) -> Option<&str> {
        self.0
            .iter()
            .find(|(l, _)| l.as_deref().is_some_and(want))
            .map(|(_, t)| t.as_str())
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

/// The primary subtag: `de` for `de-CH`.
fn primary(tag: &str) -> &str {
    tag.split('-').next().unwrap_or(tag)
}

/// `range` without its last subtag, and without a single-letter subtag
/// that would then end it (`de-CH-x-a` gives `de-CH`); `None` at the
/// primary subtag.
fn shorter(range: &str) -> Option<&str> {
    let mut r = &range[..range.rfind('-')?];
    while let Some(i) = r.rfind('-')
        && r.len() - i == 2
    {
        r = &r[..i];
    }
    Some(r)
}

/// `s` as plain text of at most `max_chars` characters: tags stripped,
/// entities decoded, bidi, zero-width and control characters removed,
/// whitespace collapsed, and an ellipsis where it was cut. The result is
/// meant for a text node, never for markup.
pub(crate) fn plain(s: &str, max_chars: usize) -> String {
    let decoded = decode_entities(&strip_tags(s));
    let mut out = String::with_capacity(decoded.len());
    let mut space = false;
    for c in decoded.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
        } else if !invisible(c) {
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
    }
    truncate(out, max_chars)
}

/// Every control character, and every Unicode default-ignorable code
/// point (bidi controls, zero-width characters, the soft hyphen, variation
/// selectors, tag characters, fillers), plus the interlinear annotation
/// characters: text that renders as nothing, or as a blank that is not
/// whitespace. Whitespace is handled before this is asked.
pub(crate) fn invisible(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{00AD}' | '\u{034F}' | '\u{061C}' | '\u{115F}' | '\u{1160}'
            | '\u{17B4}' | '\u{17B5}' | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}'
            | '\u{3164}' | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}' | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFFB}' | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}' | '\u{E0000}'..='\u{E0FFF}')
}

/// `s` with every tag replaced by a space. A `<` opens a tag only when a
/// letter, `/`, `!` or `?` follows it; an unterminated tag runs to the end.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let opens = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'));
        if !opens {
            out.push('<');
            rest = after;
            continue;
        }
        out.push(' ');
        match after.find('>') {
            Some(j) => rest = &after[j + 1..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// `s` with the HTML entities profiles use decoded: the five XML ones,
/// `&nbsp;` and numeric references. Anything else is kept as written.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        // Only the next 11 bytes are searched, so a text of many `&` stays
        // linear.
        let entity = after
            .bytes()
            .take(11)
            .position(|b| b == b';')
            .and_then(|j| entity(&after[..j]).map(|c| (c, j)));
        match entity {
            Some((c, j)) => {
                out.push(c);
                rest = &after[j + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let n = name.strip_prefix('#')?;
            let code = match n.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => n.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

/// At most `max` characters; a cut text ends in an ellipsis.
fn truncate(s: String, max: usize) -> String {
    if s.chars().count() <= max {
        return s;
    }
    if max == 0 {
        return String::new();
    }
    let mut cut: String = s.chars().take(max - 1).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('\u{2026}');
    cut
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
    fn pick_follows_the_order_of_preference() {
        // A primary-subtag match on the first choice beats an exact match on
        // a later one (RFC 4647 lookup: de-CH falls back to de before fr).
        let t = text(&[(Some("fr"), "Entraîner"), (Some("de"), "Trainieren")]);
        assert_eq!(t.pick(&prefs(&["de-CH", "fr"])), Some("Trainieren"));
        assert_eq!(t.pick(&prefs(&["fr", "de-CH"])), Some("Entraîner"));
        // Within one preference, the truncated tag beats a sibling region.
        let regions = text(&[
            (Some("de-at"), "Trainieren (AT)"),
            (Some("de"), "Trainieren"),
        ]);
        assert_eq!(regions.pick(&prefs(&["de-CH"])), Some("Trainieren"));
        let script = text(&[
            (Some("zh"), "zh"),
            (Some("zh-hant"), "zh-Hant"),
            (Some("zh-hans"), "zh-Hans"),
        ]);
        assert_eq!(script.pick(&prefs(&["zh-Hant-TW"])), Some("zh-Hant"));
        // A sibling still beats a later preference's exact match.
        assert_eq!(
            regions.pick(&prefs(&["de-AT-x-y"])),
            Some("Trainieren (AT)")
        );
        let sibling = text(&[(Some("fr"), "Entraîner"), (Some("de-at"), "Trainieren")]);
        assert_eq!(sibling.pick(&prefs(&["de-CH", "fr"])), Some("Trainieren"));
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
    fn strips_every_default_ignorable_and_format_character() {
        // Tag characters can smuggle invisible ASCII; the others render as
        // nothing, or as a blank that is not whitespace.
        for c in [
            '\u{034F}',
            '\u{115F}',
            '\u{1160}',
            '\u{17B4}',
            '\u{17B5}',
            '\u{180B}',
            '\u{180E}',
            '\u{180F}',
            '\u{2065}',
            '\u{206A}',
            '\u{206F}',
            '\u{3164}',
            '\u{FE00}',
            '\u{FE0F}',
            '\u{FFA0}',
            '\u{FFF0}',
            '\u{FFF9}',
            '\u{FFFB}',
            '\u{1BCA0}',
            '\u{1D173}',
            '\u{1D17A}',
            '\u{E0000}',
            '\u{E0041}',
            '\u{E007F}',
            '\u{E0100}',
            '\u{E0FFF}',
        ] {
            assert_eq!(plain(&format!("a{c}b"), 4_000), "ab", "U+{:04X}", c as u32);
        }
        // Visible text around those ranges is kept.
        assert_eq!(
            plain("\u{17B3}\u{3165}\u{FFFC}", 4_000),
            "\u{17B3}\u{3165}\u{FFFC}"
        );
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
