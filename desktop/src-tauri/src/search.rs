//! Pure helpers for building search queries.

/// Normalizes a code like the `codigo_norm` SQL expression in the DB triggers.
pub fn normalize_code(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| {
            !matches!(
                c,
                ' ' | '\t' | '\n' | '\r' | '\x0B' | '\x0C' | '-' | '_' | '\u{A0}'
            )
        })
        .collect()
}

/// Wraps text as an FTS5 string literal (phrase), doubling inner quotes.
pub fn fts_phrase(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Builds the FTS5 MATCH expression, or None if the query is too short (< 3 chars).
pub fn build_match(query: &str) -> Option<String> {
    let q = query.trim();
    if q.chars().count() < 3 {
        return None;
    }
    let mut m = fts_phrase(q);
    let norm = normalize_code(q);
    if norm.chars().count() >= 3 {
        m.push_str(" OR codigo_norm : ");
        m.push_str(&fts_phrase(&norm));
    }
    Some(m)
}

/// `%q%` pattern escaping `%`, `_` and `\` (use with `ESCAPE '\'`).
pub fn like_pattern(q: &str) -> String {
    let mut out = String::with_capacity(q.len() + 2);
    out.push('%');
    for c in q.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        assert_eq!(normalize_code("RY-10_00 \u{A0}x\t"), "ry1000x");
        assert_eq!(normalize_code(""), "");
    }

    #[test]
    fn phrase_escapes_quotes() {
        assert_eq!(fts_phrase(r#"a"b"#), r#""a""b""#);
    }

    #[test]
    fn match_short_is_none() {
        assert_eq!(build_match("ab"), None);
        assert_eq!(build_match("  a "), None);
        assert_eq!(build_match(""), None);
    }

    #[test]
    fn match_with_and_without_code_clause() {
        assert_eq!(
            build_match("the").unwrap(),
            r#""the" OR codigo_norm : "the""#
        );
        // normalized shorter than 3 -> no code clause
        assert_eq!(build_match("a-b").unwrap(), r#""a-b""#);
        assert_eq!(
            build_match("RY-1").unwrap(),
            r#""RY-1" OR codigo_norm : "ry1""#
        );
    }

    #[test]
    fn like_escapes() {
        assert_eq!(like_pattern(r"50%_a\b"), r"%50\%\_a\\b%");
    }
}
