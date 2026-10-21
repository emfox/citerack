//! Case and accent folding, so a query typed without accents still matches.

/// Maps a character to the lowercase ASCII it should match against; an accented
/// Latin letter folds to its base letter without a Unicode normalisation crate.
pub fn fold_char(c: char) -> char {
    let lowered = c.to_lowercase().next().unwrap_or(c);
    match lowered {
        '\u{00e0}'..='\u{00e5}' | '\u{0101}' | '\u{0103}' | '\u{0105}' | '\u{01ce}' => 'a',
        '\u{00e7}' | '\u{0107}' | '\u{0109}' | '\u{010d}' => 'c',
        '\u{00e8}'..='\u{00eb}' | '\u{0113}' | '\u{0115}' | '\u{0117}' | '\u{011b}' => 'e',
        '\u{0123}' | '\u{011f}' => 'g',
        '\u{00ec}'..='\u{00ef}' | '\u{012b}' | '\u{012d}' | '\u{012f}' | '\u{01d0}' => 'i',
        '\u{0142}' => 'l',
        '\u{00f1}' | '\u{0144}' | '\u{0148}' => 'n',
        '\u{00f2}'..='\u{00f6}'
        | '\u{00f8}'
        | '\u{014d}'
        | '\u{014f}'
        | '\u{0151}'
        | '\u{01d2}' => 'o',
        '\u{0155}' | '\u{0159}' => 'r',
        '\u{015b}' | '\u{015d}' | '\u{015f}' | '\u{0161}' => 's',
        '\u{0163}' | '\u{0165}' | '\u{0167}' => 't',
        '\u{00f9}'..='\u{00fc}'
        | '\u{016b}'
        | '\u{016d}'
        | '\u{016f}'
        | '\u{0171}'
        | '\u{01d4}' => 'u',
        '\u{00fd}' | '\u{00ff}' | '\u{0177}' => 'y',
        '\u{017a}' | '\u{017c}' | '\u{017e}' => 'z',
        other => other,
    }
}

/// Folds a string the same way [`fold_char`] folds one character. Folding never
/// lengthens a character in bytes, so the result fits in the input's own size and
/// is reserved once rather than grown per field.
pub fn fold(text: &str) -> String {
    // ASCII, which is most of a bibliography, folds to itself and is not walked
    // by character.
    if text.is_ascii() {
        return text.to_ascii_lowercase();
    }
    let mut out = String::with_capacity(text.len());
    out.extend(text.chars().map(fold_char));
    out
}
