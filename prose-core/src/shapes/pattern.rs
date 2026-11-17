//! The `sh:pattern` subset: a small regular-expression matcher, so a shape's
//! pattern can be checked without a regex dependency in the wasm.
//!
//! Read: literals, `.`, `^`, `$`, groups `(..)` and `(?:..)`, alternation,
//! character classes (ranges, negation, `\d \w \s` and their negations),
//! the escapes `\n \r \t` and escaped punctuation, and the quantifiers `*`,
//! `+`, `?`, `{n}`, `{n,}` and `{n,m}` (a trailing `?` is accepted: whether
//! a match exists does not depend on greed). The flag `i` is read.
//!
//! Not read, and refused with the reason: look-around, back-references,
//! `\b`, `\p{..}`, named groups and the flags `s`, `m` and `x`. Like
//! SHACL's, a match is searched anywhere in the text unless `^` and `$` pin
//! it.
//!
//! The pattern is third-party input. Matching keeps a set of visited
//! (instruction, position) states, so it takes time proportional to the
//! pattern times the text and never backtracks exponentially; a pattern
//! past [`MAX_PROGRAM`] instructions or a text past [`MAX_TEXT`] characters
//! is not run.

/// The most instructions a pattern may compile to.
pub(crate) const MAX_PROGRAM: usize = 1_000;
/// The longest text a pattern is matched against, in characters.
pub(crate) const MAX_TEXT: usize = 1_024;
/// Group nesting and the largest counted repeat.
const MAX_DEPTH: usize = 24;
const MAX_REPEAT: u32 = 200;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pattern {
    source: String,
    flags: String,
    prog: Vec<Inst>,
    insensitive: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Inst {
    Char(char),
    Any,
    Class(Vec<Item>, bool),
    Start,
    End,
    Split(usize, usize),
    Jmp(usize),
    Match,
}

#[derive(Clone, Debug, PartialEq)]
enum Item {
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

#[derive(Debug)]
enum Ast {
    Empty,
    Char(char),
    Any,
    Class(Vec<Item>, bool),
    Start,
    End,
    Cat(Vec<Ast>),
    Alt(Vec<Ast>),
    Rep(Box<Ast>, u32, Option<u32>),
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl Parser<'_> {
    fn alt(&mut self, depth: usize) -> Result<Ast, String> {
        if depth > MAX_DEPTH {
            return Err("groups nest too deeply".to_string());
        }
        let mut branches = vec![self.cat(depth)?];
        while self.chars.peek() == Some(&'|') {
            self.chars.next();
            branches.push(self.cat(depth)?);
        }
        Ok(match branches.len() {
            1 => branches.remove(0),
            _ => Ast::Alt(branches),
        })
    }

    fn cat(&mut self, depth: usize) -> Result<Ast, String> {
        let mut items = Vec::new();
        while let Some(&c) = self.chars.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom(depth)?;
            items.push(self.quantified(atom)?);
        }
        Ok(match items.len() {
            0 => Ast::Empty,
            1 => items.remove(0),
            _ => Ast::Cat(items),
        })
    }

    fn quantified(&mut self, atom: Ast) -> Result<Ast, String> {
        let (min, max) = match self.chars.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => {
                self.chars.next();
                let range = self.counts()?;
                return self.finish_quantifier(atom, range);
            }
            _ => return Ok(atom),
        };
        self.chars.next();
        self.finish_quantifier(atom, (min, max))
    }

    fn finish_quantifier(
        &mut self,
        atom: Ast,
        (min, max): (u32, Option<u32>),
    ) -> Result<Ast, String> {
        if matches!(atom, Ast::Start | Ast::End) {
            return Err("an anchor has nothing to repeat".to_string());
        }
        if self.chars.peek() == Some(&'?') {
            // Lazy: whether a match exists does not depend on greed.
            self.chars.next();
        }
        Ok(Ast::Rep(Box::new(atom), min, max))
    }

    /// `n}`, `n,}` or `n,m}` after the `{`.
    fn counts(&mut self) -> Result<(u32, Option<u32>), String> {
        let number = |p: &mut Parser<'_>| -> Option<u32> {
            let mut n: Option<u32> = None;
            while let Some(d) = p.chars.peek().and_then(|c| c.to_digit(10)) {
                n = Some(n.unwrap_or(0).saturating_mul(10).saturating_add(d));
                p.chars.next();
            }
            n
        };
        let min = number(self).ok_or("a repeat count needs a number")?;
        let max = match self.chars.next() {
            Some('}') => Some(min),
            Some(',') => {
                let max = number(self);
                match self.chars.next() {
                    Some('}') => max,
                    _ => return Err("an unclosed repeat count".to_string()),
                }
            }
            _ => return Err("an unclosed repeat count".to_string()),
        };
        if max.is_some_and(|m| m < min) {
            return Err("a repeat count has its maximum below its minimum".to_string());
        }
        if min > MAX_REPEAT || max.is_some_and(|m| m > MAX_REPEAT) {
            return Err(format!("a repeat count is past {MAX_REPEAT}"));
        }
        Ok((min, max))
    }

    fn atom(&mut self, depth: usize) -> Result<Ast, String> {
        let c = self.chars.next().ok_or("the pattern ends early")?;
        match c {
            '(' => {
                if self.chars.peek() == Some(&'?') {
                    self.chars.next();
                    match self.chars.next() {
                        Some(':') => {}
                        Some('=' | '!') => return Err("look-ahead is not read".to_string()),
                        Some('<') => {
                            return Err(match self.chars.peek() {
                                Some('=' | '!') => "look-behind is not read".to_string(),
                                _ => "named groups are not read".to_string(),
                            });
                        }
                        _ => return Err("this group form is not read".to_string()),
                    }
                }
                let inner = self.alt(depth + 1)?;
                match self.chars.next() {
                    Some(')') => Ok(inner),
                    _ => Err("an unclosed group".to_string()),
                }
            }
            ')' => Err("an unmatched )".to_string()),
            '[' => self.class(),
            '.' => Ok(Ast::Any),
            '^' => Ok(Ast::Start),
            '$' => Ok(Ast::End),
            '*' | '+' | '?' => Err("a quantifier has nothing to repeat".to_string()),
            '\\' => match self.escape()? {
                Escaped::Char(c) => Ok(Ast::Char(c)),
                Escaped::Item(i) => Ok(Ast::Class(vec![i], false)),
            },
            c => Ok(Ast::Char(c)),
        }
    }

    fn escape(&mut self) -> Result<Escaped, String> {
        let c = self
            .chars
            .next()
            .ok_or("the pattern ends with a backslash")?;
        Ok(match c {
            'd' => Escaped::Item(Item::Digit(true)),
            'D' => Escaped::Item(Item::Digit(false)),
            'w' => Escaped::Item(Item::Word(true)),
            'W' => Escaped::Item(Item::Word(false)),
            's' => Escaped::Item(Item::Space(true)),
            'S' => Escaped::Item(Item::Space(false)),
            'n' => Escaped::Char('\n'),
            'r' => Escaped::Char('\r'),
            't' => Escaped::Char('\t'),
            '1'..='9' => return Err("a back-reference is not read".to_string()),
            'b' | 'B' => return Err("\\b is not read".to_string()),
            'p' | 'P' => return Err("\\p is not read".to_string()),
            c if c.is_ascii_alphanumeric() => {
                return Err(format!("the escape \\{c} is not read"));
            }
            c => Escaped::Char(c),
        })
    }

    /// After the `[`.
    fn class(&mut self) -> Result<Ast, String> {
        let negate = self.chars.peek() == Some(&'^');
        if negate {
            self.chars.next();
        }
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.chars.next().ok_or("an unclosed character class")?;
            if c == ']' && !first {
                break;
            }
            first = false;
            let lo = match c {
                '\\' => match self.escape()? {
                    Escaped::Char(c) => c,
                    Escaped::Item(i) => {
                        items.push(i);
                        continue;
                    }
                },
                c => c,
            };
            // A range, unless the dash ends the class.
            let mut ahead = self.chars.clone();
            if ahead.next() == Some('-') && ahead.peek().is_some_and(|&n| n != ']') {
                self.chars.next();
                let hi = match self.chars.next() {
                    Some('\\') => match self.escape()? {
                        Escaped::Char(c) => c,
                        Escaped::Item(_) => {
                            return Err("a class escape cannot end a range".to_string());
                        }
                    },
                    Some(c) => c,
                    None => return Err("an unclosed character class".to_string()),
                };
                if hi < lo {
                    return Err("a range runs backwards".to_string());
                }
                items.push(Item::Range(lo, hi));
            } else {
                items.push(Item::Range(lo, lo));
            }
        }
        Ok(Ast::Class(items, negate))
    }
}

enum Escaped {
    Char(char),
    Item(Item),
}

fn emit(ast: &Ast, prog: &mut Vec<Inst>) -> Result<(), String> {
    if prog.len() > MAX_PROGRAM {
        return Err("the pattern is too large".to_string());
    }
    match ast {
        Ast::Empty => {}
        Ast::Char(c) => prog.push(Inst::Char(*c)),
        Ast::Any => prog.push(Inst::Any),
        Ast::Class(items, negate) => prog.push(Inst::Class(items.clone(), *negate)),
        Ast::Start => prog.push(Inst::Start),
        Ast::End => prog.push(Inst::End),
        Ast::Cat(items) => {
            for i in items {
                emit(i, prog)?;
            }
        }
        Ast::Alt(branches) => {
            let mut jumps = Vec::new();
            for (n, b) in branches.iter().enumerate() {
                if n + 1 < branches.len() {
                    let split = prog.len();
                    prog.push(Inst::Split(split + 1, 0));
                    emit(b, prog)?;
                    jumps.push(prog.len());
                    prog.push(Inst::Jmp(0));
                    let next = prog.len();
                    prog[split] = Inst::Split(split + 1, next);
                } else {
                    emit(b, prog)?;
                }
            }
            let end = prog.len();
            for j in jumps {
                prog[j] = Inst::Jmp(end);
            }
        }
        Ast::Rep(x, min, max) => {
            for _ in 0..*min {
                emit(x, prog)?;
            }
            match max {
                None => {
                    let split = prog.len();
                    prog.push(Inst::Split(split + 1, 0));
                    emit(x, prog)?;
                    prog.push(Inst::Jmp(split));
                    let end = prog.len();
                    prog[split] = Inst::Split(split + 1, end);
                }
                Some(max) => {
                    let mut splits = Vec::new();
                    for _ in *min..*max {
                        splits.push(prog.len());
                        prog.push(Inst::Split(0, 0));
                        emit(x, prog)?;
                    }
                    let end = prog.len();
                    for s in splits {
                        prog[s] = Inst::Split(s + 1, end);
                    }
                }
            }
        }
    }
    if prog.len() > MAX_PROGRAM {
        return Err("the pattern is too large".to_string());
    }
    Ok(())
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn in_item(item: &Item, c: char, insensitive: bool) -> bool {
    match item {
        Item::Range(lo, hi) => {
            (*lo..=*hi).contains(&c)
                || (insensitive
                    && (c.to_lowercase().chain(c.to_uppercase())).any(|v| (*lo..=*hi).contains(&v)))
        }
        Item::Digit(yes) => c.is_ascii_digit() == *yes,
        Item::Word(yes) => (c.is_alphanumeric() || c == '_') == *yes,
        Item::Space(yes) => c.is_whitespace() == *yes,
    }
}

impl Pattern {
    /// Compiles `source` with SHACL `flags` (`""` or `"i"`), or says why it
    /// is not in the subset.
    pub(crate) fn new(source: &str, flags: &str) -> Result<Pattern, String> {
        if let Some(f) = flags.chars().find(|&f| f != 'i') {
            return Err(format!("the flag {f:?} is not read (only i is)"));
        }
        let mut parser = Parser {
            chars: source.chars().peekable(),
        };
        let ast = parser.alt(0)?;
        if parser.chars.next().is_some() {
            return Err("an unmatched )".to_string());
        }
        let mut prog = Vec::new();
        emit(&ast, &mut prog)?;
        prog.push(Inst::Match);
        Ok(Pattern {
            source: source.to_string(),
            flags: flags.to_string(),
            prog,
            insensitive: flags.contains('i'),
        })
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    /// Whether `text` matches; `None` when it is too long to be run.
    pub(crate) fn is_match(&self, text: &str) -> Option<bool> {
        let chars: Vec<char> = text.chars().take(MAX_TEXT + 1).collect();
        if chars.len() > MAX_TEXT {
            return None;
        }
        // A state that failed once fails again, from any start: one visited
        // set serves every start position.
        let mut seen = vec![false; self.prog.len() * (chars.len() + 1)];
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for start in 0..=chars.len() {
            stack.push((0, start));
            while let Some((pc, pos)) = stack.pop() {
                let key = pc * (chars.len() + 1) + pos;
                if std::mem::replace(&mut seen[key], true) {
                    continue;
                }
                match &self.prog[pc] {
                    Inst::Match => return Some(true),
                    Inst::Jmp(to) => stack.push((*to, pos)),
                    Inst::Split(a, b) => {
                        stack.push((*b, pos));
                        stack.push((*a, pos));
                    }
                    Inst::Start => {
                        if pos == 0 {
                            stack.push((pc + 1, pos));
                        }
                    }
                    Inst::End => {
                        if pos == chars.len() {
                            stack.push((pc + 1, pos));
                        }
                    }
                    Inst::Any => {
                        if pos < chars.len() {
                            stack.push((pc + 1, pos + 1));
                        }
                    }
                    Inst::Char(c) => {
                        if let Some(&got) = chars.get(pos)
                            && (got == *c || (self.insensitive && fold(got) == fold(*c)))
                        {
                            stack.push((pc + 1, pos + 1));
                        }
                    }
                    Inst::Class(items, negate) => {
                        if let Some(&got) = chars.get(pos)
                            && items.iter().any(|i| in_item(i, got, self.insensitive)) != *negate
                        {
                            stack.push((pc + 1, pos + 1));
                        }
                    }
                }
            }
        }
        Some(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &str, t: &str) -> bool {
        Pattern::new(p, "").unwrap().is_match(t).unwrap()
    }

    #[test]
    fn literals_and_anchors() {
        assert!(m("abc", "xxabcxx"), "searched anywhere");
        assert!(!m("^abc", "xabc"));
        assert!(m("^abc$", "abc"));
        assert!(!m("^abc$", "abcd"));
        assert!(m("", "anything"));
    }

    #[test]
    fn dot_classes_and_escapes() {
        assert!(m("^a.c$", "abc"));
        assert!(!m("^a.c$", "ac"));
        assert!(m("^[a-c]+$", "abcabc"));
        assert!(!m("^[a-c]+$", "abd"));
        assert!(m("^[^0-9]+$", "abc"));
        assert!(!m("^[^0-9]+$", "ab1"));
        assert!(m(r"^\d+$", "2026"));
        assert!(!m(r"^\d+$", "20x6"));
        assert!(m(r"^[\w-]+$", "a-b_c"));
        assert!(m(r"^\s*$", " \t"));
        assert!(m(r"^a\.b$", "a.b"));
        assert!(!m(r"^a\.b$", "axb"));
        assert!(m(r"^\S+$", "abc"));
    }

    #[test]
    fn quantifiers() {
        assert!(m("^ab*c$", "ac"));
        assert!(m("^ab*c$", "abbbc"));
        assert!(!m("^ab+c$", "ac"));
        assert!(m("^ab?c$", "abc"));
        assert!(m("^a{3}$", "aaa"));
        assert!(!m("^a{3}$", "aa"));
        assert!(m("^a{2,}$", "aaaaa"));
        assert!(m("^a{2,3}$", "aaa"));
        assert!(!m("^a{2,3}$", "aaaa"));
        assert!(m("^[A-Z]{3}-[0-9]+$", "ABC-12"));
        assert!(!m("^[A-Z]{3}-[0-9]+$", "AB-12"));
        assert!(
            m("^a+?$", "aaa"),
            "a lazy mark does not change whether it matches"
        );
    }

    #[test]
    fn groups_and_alternation() {
        assert!(m("^(ab|cd)+$", "abcdab"));
        assert!(!m("^(ab|cd)+$", "abc"));
        assert!(m("^(?:eu|us)-west$", "us-west"));
        assert!(m("^a|b$", "xb"));
        assert!(m("^(a|)b$", "b"));
    }

    #[test]
    fn the_i_flag_ignores_case() {
        let p = Pattern::new("^abc[x-z]$", "i").unwrap();
        assert_eq!(p.is_match("ABCY"), Some(true));
        assert_eq!(
            Pattern::new("^abc$", "").unwrap().is_match("ABC"),
            Some(false)
        );
        assert_eq!(p.source(), "^abc[x-z]$");
    }

    #[test]
    fn unicode_is_read_by_character() {
        assert!(m("^é+$", "éé"));
        assert!(m("^.$", "é"));
    }

    #[test]
    fn what_is_not_in_the_subset_is_refused_with_a_reason() {
        for (p, why) in [
            ("(?=a)b", "look"),
            ("(?!a)b", "look"),
            (r"(a)\1", "back-reference"),
            (r"\bword", "\\b"),
            (r"\p{L}", "\\p"),
            ("(?<n>a)", "named"),
            ("(a", "unclosed"),
            ("a)", "unmatched"),
            ("[a", "unclosed"),
            ("*a", "nothing to repeat"),
            ("a{2,1}", "repeat"),
            ("a{500}", "repeat"),
            ("[z-a]", "range"),
        ] {
            let e = Pattern::new(p, "").unwrap_err();
            assert!(e.contains(why), "{p}: {e}");
        }
        for flags in ["s", "m", "x", "iq"] {
            let e = Pattern::new("a", flags).unwrap_err();
            assert!(e.contains("flag"), "{flags}: {e}");
        }
    }

    #[test]
    fn a_hostile_pattern_costs_a_bounded_amount() {
        // Nested quantifiers: exponential for a naive backtracker.
        let p = Pattern::new("^(a*)*b$", "").unwrap();
        let text = "a".repeat(1_000);
        let started = std::time::Instant::now();
        assert_eq!(p.is_match(&text), Some(false));
        assert!(started.elapsed().as_secs() < 5);
        // A program past the cap is refused.
        let big = "(a{200}){200}";
        assert!(Pattern::new(big, "").unwrap_err().contains("too large"));
        // A text past the cap is not run.
        assert_eq!(p.is_match(&"a".repeat(MAX_TEXT + 1)), None);
    }
}
