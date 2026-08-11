//! Case-insensitive matching for the `/` filter and the per-character
//! highlight in the task row renderer.
//!
//! Two matchers live here:
//! - `substring_match_ci` — contiguous substring (used by the `/` task-text
//!   search, the saved-filter sidebar counts, and the row highlighter).
//! - `subseq_match_ci` — subsequence, chars in order with gaps allowed (used
//!   by the command palette's fuzzy menu finder).

/// Returns byte offsets in `haystack` of the **first** case-insensitive
/// contiguous match of `needle`. Returns `None` when `needle` is empty or
/// not found as a contiguous substring.
///
/// Offsets are into the original `haystack` (not a lowercased copy), so they
/// land on `char_indices` boundaries and are safe to slice — same contract
/// as `subseq_match_ci`.
///
/// Multi-word needles match the literal contiguous phrase (including the
/// exact whitespace between words). There is no AND/OR splitting.
pub fn substring_match_ci(haystack: &str, needle: &str) -> Option<Vec<usize>> {
    if needle.is_empty() {
        return None;
    }
    let needle_chars: Vec<String> = needle
        .chars()
        .map(|c| c.to_lowercase().collect::<String>())
        .collect();
    let n = needle_chars.len();

    // Walk every char-aligned start position in the haystack.
    let haystack_chars: Vec<(usize, String)> = haystack
        .char_indices()
        .map(|(b, c)| (b, c.to_lowercase().collect::<String>()))
        .collect();

    if n > haystack_chars.len() {
        return None;
    }

    for start in 0..=haystack_chars.len() - n {
        let mut ok = true;
        for (i, nc) in needle_chars.iter().enumerate() {
            if haystack_chars[start + i].1 != *nc {
                ok = false;
                break;
            }
        }
        if ok {
            return Some(
                haystack_chars[start..start + n]
                    .iter()
                    .map(|(b, _)| *b)
                    .collect(),
            );
        }
    }
    None
}

/// Returns byte offsets in `haystack` where each char of `needle` is matched
/// in order, case-insensitively, with arbitrary gaps allowed. Returns `None`
/// when not every needle char can be matched, or when `needle` is empty.
///
/// Offsets are into the original `haystack` (not a lowercased copy), so they
/// land on `char_indices` boundaries and are safe to slice.
pub fn subseq_match_ci(haystack: &str, needle: &str) -> Option<Vec<usize>> {
    if needle.is_empty() {
        return None;
    }
    let needle_lower: Vec<String> = needle
        .chars()
        .map(|c| c.to_lowercase().collect::<String>())
        .collect();
    let mut positions = Vec::with_capacity(needle_lower.len());
    let mut idx = 0;
    for (byte, ch) in haystack.char_indices() {
        if idx == needle_lower.len() {
            break;
        }
        let ch_lower: String = ch.to_lowercase().collect();
        if ch_lower == needle_lower[idx] {
            positions.push(byte);
            idx += 1;
        }
    }
    (idx == needle_lower.len()).then_some(positions)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn matches_contiguous_substring() {
        assert_eq!(subseq_match_ci("Hello", "ell"), Some(vec![1, 2, 3]));
    }

    #[test]
    fn matches_subsequence_with_gaps() {
        // The motivating bug: "cade" finds C, a, d, e in "Call dentist".
        let positions = subseq_match_ci("Call dentist", "cade").unwrap();
        assert_eq!(positions, vec![0, 1, 5, 6]);
    }

    #[test]
    fn case_insensitive_both_directions() {
        assert_eq!(subseq_match_ci("HELLO", "ell"), Some(vec![1, 2, 3]));
        assert_eq!(subseq_match_ci("hello", "ELL"), Some(vec![1, 2, 3]));
    }

    #[test]
    fn empty_needle_is_none() {
        assert_eq!(subseq_match_ci("anything", ""), None);
    }

    #[test]
    fn missing_chars_return_none() {
        assert_eq!(subseq_match_ci("hello", "xyz"), None);
        // "cae" is a subsequence of "Call dentist" but "caz" is not.
        assert_eq!(subseq_match_ci("Call dentist", "caz"), None);
    }

    #[test]
    fn order_matters() {
        // Subsequence is in-order: "dc" can't match "Call dentist" because 'd'
        // appears after 'c'.
        assert_eq!(subseq_match_ci("Call dentist", "dc"), None);
    }

    #[test]
    fn offsets_land_on_char_boundaries_for_unicode() {
        // "Café" byte layout: C(0) a(1) f(2) é(3..5). Matching "cé" should
        // return byte offsets that the caller can slice without panicking.
        let positions = subseq_match_ci("Café", "cé").unwrap();
        assert_eq!(positions, vec![0, 3]);
        let haystack = "Café";
        for p in positions {
            // Will panic on a non-boundary slice; assert we're safe.
            let _ = &haystack[p..];
        }
    }

    // --- substring_match_ci ---------------------------------------------

    #[test]
    fn substring_matches_contiguous_run() {
        assert_eq!(
            substring_match_ci("Hello", "ell"),
            Some(vec![1, 2, 3])
        );
    }

    #[test]
    fn substring_case_insensitive_both_directions() {
        assert_eq!(
            substring_match_ci("HELLO", "ell"),
            Some(vec![1, 2, 3])
        );
        assert_eq!(
            substring_match_ci("hello", "ELL"),
            Some(vec![1, 2, 3])
        );
    }

    #[test]
    fn substring_empty_needle_is_none() {
        assert_eq!(substring_match_ci("anything", ""), None);
    }

    #[test]
    fn substring_missing_returns_none() {
        assert_eq!(substring_match_ci("hello", "xyz"), None);
        // "cae" is not a contiguous substring of "Call dentist".
        assert_eq!(substring_match_ci("Call dentist", "cae"), None);
    }

    #[test]
    fn substring_returns_first_occurrence_only() {
        // "ab" appears twice; we return the first match's offsets.
        assert_eq!(
            substring_match_ci("ab ab", "ab"),
            Some(vec![0, 1])
        );
    }

    #[test]
    fn substring_offsets_land_on_char_boundaries_for_unicode() {
        // "Café" byte layout: C(0) a(1) f(2) é(3..5). Matching "fé" should
        // return byte offsets that the caller can slice without panicking.
        let positions = substring_match_ci("Café", "fé").unwrap();
        assert_eq!(positions, vec![2, 3]);
        let haystack = "Café";
        for p in positions {
            let _ = &haystack[p..];
        }
    }

    #[test]
    fn substring_matches_multiword_phrase_contiguously() {
        // The whole phrase must appear as a contiguous run.
        assert_eq!(
            substring_match_ci("Call dentist today", "call dentist"),
            Some(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])
        );
        // "call  dentist" (two spaces) is NOT a contiguous match for the
        // single-space needle.
        assert_eq!(
            substring_match_ci("call  dentist", "call dentist"),
            None
        );
    }

    #[test]
    fn substring_does_not_match_with_gaps() {
        // The motivating change: "cade" must NOT match "Call dentist" under
        // substring matching, even though it is a subsequence.
        assert_eq!(substring_match_ci("Call dentist", "cade"), None);
        // "thane" must not match "the name" by scattering.
        assert_eq!(substring_match_ci("the name", "thane"), None);
    }

    #[test]
    fn substring_haystack_shorter_than_needle_returns_none() {
        assert_eq!(substring_match_ci("", "a"), None);
        assert_eq!(substring_match_ci("ab", "abc"), None);
        // Same length, no match — must not panic either.
        assert_eq!(substring_match_ci("abc", "xyz"), None);
    }
}
