# Substring search for task text

**Date:** 2026-08-11
**Status:** Approved (pending implementation)
**Project:** tuxedo (Rust todo.txt TUI)

## Problem

The `/` search in tuxedo uses `subseq_match_ci` (`src/search.rs:10`): a
case-insensitive **subsequence** matcher. Characters of the needle must appear
in order, but gaps are allowed. So typing `Thane` matches any task whose body
contains `T`, `h`, `a`, `n`, `e` somewhere in sequence — scattering the letters
across unrelated words. The user wants a **full text search**: typing `Thane`
should match only tasks containing `Thane` as a contiguous run.

The subsequence matcher is used in four places:

| Location | Purpose |
|---|---|
| `src/core/filter.rs:103` | Core task filter predicate (TUI + CLI) |
| `src/ui/filters.rs:83` | Saved-filter match counts in the sidebar |
| `src/ui/task_row.rs:76` | Per-character highlight in rendered rows |
| `src/app/palette.rs:370` | Command palette menu fuzzy finder |

The command palette (#4) is a different UX — typing `arch` to find `archive` is
the right behavior for menu navigation. It stays fuzzy. The three task-text
call sites (#1–3) switch to substring matching.

## Approach

Add a new matcher alongside the existing one, rather than parameterizing or
replacing the single function. The two matchers serve genuinely different UX
needs (menu navigation vs. task text search); keeping them as separate
functions makes that distinction explicit. The substring function is also
structurally simpler than the subsequence one.

Rejected alternatives:

- **Parameterize `subseq_match_ci` with a mode flag** — forces every call site
  to change, muddies two structurally different algorithms behind one function,
  and a flag parameter is a code smell.
- **Replace `subseq_match_ci` with substring everywhere** — kills the fuzzy
  command palette UX the user wants to keep.

## Design

### 1. New matcher — `src/search.rs`

Add:

```rust
pub fn substring_match_ci(haystack: &str, needle: &str) -> Option<Vec<usize>>
```

Returns the byte offsets of the **first** case-insensitive contiguous match in
`haystack`, or `None` when the needle is absent or empty. Returned offsets land
on `char_indices` boundaries in the original `haystack` (not a lowercased
copy), so callers can slice safely — same contract as `subseq_match_ci`.

Implementation walks char-by-char with `to_lowercase().collect::<String>()`
per char, mirroring the existing Unicode-safe approach. This avoids the
lowercased-string-offset pitfall the existing
`build_line_does_not_panic_on_unicode_with_match_term` test documents (e.g.
`"İ".to_lowercase()` = `"i"` + combining dot, 3 bytes vs 2 in the original).
Each char-aligned start position in the haystack is tried; the first one whose
chars match the lowercased needle chars in sequence wins.

`subseq_match_ci` stays in place, unchanged — the command palette keeps it.

### 2. Swap three task-text call sites

| File | Change |
|---|---|
| `src/core/filter.rs` | Swap `subseq_match_ci` → `substring_match_ci` at line 103; update the `use` import at line 13; rewrite the doc comment on `passes_user_filter` (lines 87–89) from "case-insensitive subsequence … gaps allowed" to "case-insensitive substring (contiguous)". |
| `src/ui/filters.rs` | Swap at line 83; update the `use` import at line 8. |
| `src/ui/task_row.rs` | Swap at line 76; update the `use` import at line 4. The per-token highlight filtering in `push_token_spans` (lines 188–217) works unchanged — contiguous positions just produce a contiguous highlighted run within a token; the `p >= token_offset_in_body && p < token_end` filter and the `p - token_offset_in_body` rebasing are position-agnostic. |

`src/app/palette.rs` is untouched.

### 3. Rename "fuzzy search" labels

The `/` search is no longer fuzzy, so labels describing it as "fuzzy search"
become inaccurate:

- `src/app/palette.rs:116`: `label: "fuzzy search"` → `"search"`
- `src/ui/help.rs:44`: `("/", "fuzzy search")` → `("/", "search")`
- `src/app/palette.rs:352`, `:457`: doc-comment prose referencing "fuzzy
  search" — reword to just "search".
- `src/app/palette.rs:474-478`: the test that asserts "fuzzy search" appears in
  results — update the literal and the assertion message.

The command palette's own fuzzy *algorithm* is unchanged; only the label of
the `/` action entry is renamed.

### 4. Tests

**New unit tests in `src/search.rs` for `substring_match_ci`:**

- contiguous substring match (`"ell"` in `"Hello"` → `Some(vec![1,2,3])`)
- case-insensitive both directions (`"ELL"` matches `"hello"` and vice versa)
- empty needle → `None`
- missing substring → `None` (`"xyz"` in `"hello"`)
- first-occurrence only (`"ab"` in `"ab ab"` → positions of the first `ab`)
- Unicode char-boundary safety (`"cé"` in `"Café"` → offsets `[0, 3]`,
  sliceable without panic)
- multi-word contiguous phrase (`"call dentist"` matches the literal
  contiguous substring, case-insensitive)
- **does NOT match with gaps** — `"cade"` must NOT match `"Call dentist"`.
  This is the inversion of the old `matches_subsequence_with_gaps` test and
  is the core behavioral assertion of the change.

**Update `src/ui/task_row.rs` test `build_line_highlights_subsequence_chars`
(line 385):** it currently asserts needle `"cade"` highlights `"Cade"` in
`"Call dentist"`. Under substring matching this needle no longer matches.
Rewrite it as `build_line_highlights_substring` using needle `"dent"` against
`"Call dentist"`, asserting the contiguous highlighted run `"dent"`. The
Unicode panic test (`build_line_does_not_panic_on_unicode_with_match_term`,
line 362) stays valid — `"a"` is a contiguous substring of `"İa"`.

**Snapshot updates:** `list_with_search` (needle `"work"`, a real substring)
needs no change. The help-overlay and command-palette snapshots contain the
string "fuzzy search" and will be regenerated with `cargo insta accept` after
the label rename (the snapshot tests themselves are not source-of-truth
changes — they track rendered output).

### 5. Docs

- `src/search.rs` module doc comment (line 1): describe both matchers —
  subsequence (used by the command palette) and substring (used by the `/`
  filter and row highlighter).
- `README.md:27`: the command-palette bullet says "Same matcher as `/` search,
  ranked so start-of-label hits beat word-boundary hits beat mid-word hits."
  Reword to clarify the palette uses fuzzy matching while `/` uses substring
  search; the ranking scheme still applies to the palette.

### Out of scope

- Highlighting all occurrences — first match only, matching current behavior.
- Multi-word AND/OR semantics — `"call dentist"` matches the literal
  contiguous phrase, not "tasks containing both `call` and `dentist`".
- Any change to the command palette algorithm or its ranking.
- Fuzzy-find libraries (nucleo, fzf-like scoring) — the hand-rolled
  subsequence matcher stays; we're not replacing it.

## Risks and mitigations

- **Backward incompatibility for saved filters.** Existing `filter.<name> =
  <query>` lines in user configs were written under subsequence semantics. A
  query like `cade` that previously matched tasks (by subsequence) will now
  match fewer or none. This is the intended behavior change — the user
  explicitly wants stricter matching — but users with saved filters built on
  fuzzy intent may see empty results. No migration is offered; the saved
  filter query is just text and users can edit it by hand.

- **Snapshot churn.** Renaming "fuzzy search" → "search" in two labels
  touches every snapshot that renders the help overlay or command palette.
  `cargo insta accept` after the rename regenerates them; the diff is
  cosmetic.

- **Performance.** Substring matching is at worst O(n*m) per task (n haystack
  chars, m needle chars) — same complexity class as the subsequence matcher.
  No measurable change expected on typical todo.txt files (hundreds to low
  thousands of tasks).

## Verification

- `cargo test` — unit tests in `search.rs`, `task_row.rs`, `palette.rs`, and
  the full snapshot suite.
- `cargo clippy` — no new warnings expected (the new function mirrors the
  existing one's style).
- Manual smoke test: type `/Thane` in the TUI and confirm only tasks
  containing `Thane` as a contiguous substring appear; confirm the command
  palette (`:` / Ctrl-P) still fuzzy-matches `arch` → `archive`.
