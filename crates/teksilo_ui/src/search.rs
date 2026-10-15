//! Accent-insensitive search with offsets into the original Unicode text.
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub block: Option<u64>,
    /// Character offsets, as used by the text editor (not UTF-8 bytes).
    pub range: Option<(usize, usize)>,
    pub before: String,
    pub matched: String,
    pub after: String,
    pub count: usize,
}

pub fn matches(text: &str, query: &str) -> Vec<(usize, usize)> {
    let mut normalized = String::new();
    let mut positions = Vec::new();
    for (index, ch) in text.chars().enumerate() {
        for c in ch
            .to_string()
            .nfd()
            .filter(|c| !is_combining_mark(*c))
            .flat_map(char::to_lowercase)
        {
            normalized.push(c);
            positions.extend(std::iter::repeat_n(index, c.len_utf8()));
        }
    }
    let query: String = query
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect();
    if query.is_empty() {
        return vec![];
    }
    let chars: Vec<_> = text.chars().collect();
    normalized
        .match_indices(&query)
        .map(|(start, hit)| {
            let mut end = positions[start + hit.len() - 1] + 1;
            while end < chars.len() && is_combining_mark(chars[end]) {
                end += 1;
            }
            (positions[start], end)
        })
        .collect()
}

pub fn hit(text: &str, query: &str, block: Option<u64>, text_range: bool) -> Option<SearchHit> {
    let ranges = matches(text, query);
    let &(start, end) = ranges.first()?;
    let chars: Vec<_> = text.chars().collect();
    let clean = |slice: &[char]| {
        slice
            .iter()
            .map(|c| if c.is_whitespace() { ' ' } else { *c })
            .collect::<String>()
    };
    let left = start.saturating_sub(18);
    let right = (end + 36).min(chars.len());
    Some(SearchHit {
        block,
        range: text_range.then_some((start, end)),
        before: format!(
            "{}{}",
            if left > 0 { "…" } else { "" },
            clean(&chars[left..start])
        ),
        matched: clean(&chars[start..end]),
        after: format!(
            "{}{}",
            clean(&chars[end..right]),
            if right < chars.len() { "…" } else { "" }
        ),
        count: ranges.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offsets_survive_accents_emoji_and_combining_marks() {
        assert_eq!(
            matches("☕ Café cafe\u{301}", "CAFÉ"),
            vec![(2, 6), (7, 12)]
        );
        let h = hit("first line\nCafé tomorrow", "cafe", Some(7), true).unwrap();
        assert_eq!(h.range, Some((11, 15)));
        assert_eq!(h.matched, "Café");
        assert_eq!(h.before, "first line ");
        assert!(matches("text", "").is_empty());
    }
}
