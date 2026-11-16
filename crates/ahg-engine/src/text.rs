//! JavaScript string semantics the Electron code relied on, kept in one place
//! so every ported module trims and slices the way the original did.
use fancy_regex::Regex;

/// Characters `String.prototype.trim` removes.
pub fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{B}' | '\u{C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}'
            | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

/// `s.trim()`.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

/// `s.slice(0, units)`, counted in UTF-16 code units as JavaScript does.
/// ponytail: a character straddling the limit is dropped whole, where
/// JavaScript would keep half of it; the result is otherwise identical.
pub fn utf16_prefix(s: &str, units: usize) -> &str {
    let mut used = 0;
    for (i, c) in s.char_indices() {
        used += c.len_utf16();
        if used > units {
            return &s[..i];
        }
    }
    s
}

/// `s.slice(-units)`, counted in UTF-16 code units. A character straddling
/// the cut is dropped whole, as in `utf16_prefix`.
pub fn utf16_suffix(s: &str, units: usize) -> &str {
    let mut used = 0;
    for (i, c) in s.char_indices().rev() {
        used += c.len_utf16();
        if used > units {
            return &s[i + c.len_utf8()..];
        }
    }
    s
}

/// `s.length` in JavaScript: UTF-16 code units.
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// JavaScript's `\s`, for use inside a character class.
pub const JS_SPACE: &str = r"\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}";

/// Compiles a pattern with JavaScript's `i` flag as it behaves without `u`:
/// only ASCII letters fold. Every letter outside a character class or an
/// escape becomes a two-case class; classes are written with both cases.
pub fn ascii_ci_regex(pattern: &str) -> Regex {
    let mut out = String::with_capacity(pattern.len() * 2);
    let mut chars = pattern.chars();
    let mut in_class = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                out.push(c);
                out.extend(chars.next());
            }
            '[' => {
                in_class = true;
                out.push(c);
            }
            ']' => {
                in_class = false;
                out.push(c);
            }
            c if c.is_ascii_alphabetic() && !in_class => {
                out.extend(['[', c.to_ascii_lowercase(), c.to_ascii_uppercase(), ']']);
            }
            c => out.push(c),
        }
    }
    Regex::new(&out).expect("pattern is a compile-time constant")
}

/// JavaScript truthiness of a JSON value.
pub fn truthy(v: &serde_json::Value) -> bool {
    use serde_json::Value;
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// A path separator in a Windows path: `\` or `/`.
pub(crate) fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}
