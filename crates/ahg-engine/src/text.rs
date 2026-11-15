//! JavaScript string semantics the Electron code relied on, kept in one place
//! so every ported module trims and slices the way the original did.

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
