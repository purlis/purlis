// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Near-duplicate pairs without comparing every pair (#5504).
//!
//! Comparing every pair is O(n²): minutes on a 10,000-entry store. Instead each entry is
//! compared only with its **candidates**, the entries it shares a word with among the first
//! words of each set, rarest first. This is the prefix filter of set-similarity joins
//! (Chaudhuri, Ganti and Kaushik, 2006), and it is exact, not a guess:
//!
//! - Two sets with Jaccard ≥ t share `o ≥ t·|r ∪ s| ≥ t·max(|r|, |s|)` words, so `o` is at
//!   least `⌈t·|r|⌉` and at least `⌈t·|s|⌉`.
//! - Order every set's words the same way. Two sets sharing `o` words share one among `r`'s
//!   first `|r| − o + 1` and `s`'s first `|s| − o + 1`: otherwise `r` would need `o` shared
//!   words among its last `o − 1`.
//! - Each set's prefix here is `|s| − ⌈t·|s|⌉ + 1` long, so it holds that one word.
//!
//! So every pair the full comparison finds is a candidate, and each candidate is scored by the
//! same measure. The answer is the full comparison's, pair for pair and score for score.
//! Ordering the words rarest first keeps the candidate lists short.

use std::collections::{BTreeSet, HashMap};

/// Every pair of `sets` with Jaccard ≥ `threshold`, as `(score, i, j)` with `i < j`:
/// strongest first, then in listing order, as [`super::duplicates`] answers.
pub(super) fn pairs(sets: &[BTreeSet<String>], threshold: f64) -> Vec<(f64, usize, usize)> {
    // At or below zero, every pair counts, even one sharing no word: there is nothing to
    // filter on. A NaN threshold matches nothing, so the full comparison answers that too.
    if threshold.is_nan() || threshold <= 0.0 {
        return ordered(every_pair(sets, threshold));
    }
    let ranked = ranked(sets);
    // Which earlier entries have each word in their prefix, by the word's rank.
    let mut holders: Vec<Vec<usize>> = vec![Vec::new(); ranked.iter().flatten().count()];
    // The entry each earlier one was last scored against, so a pair is scored once.
    let mut scored = vec![usize::MAX; sets.len()];
    let mut found = Vec::new();
    for (j, words) in ranked.iter().enumerate() {
        for &word in &words[..prefix(words.len(), threshold)] {
            let earlier = &mut holders[word as usize];
            for &i in earlier.iter() {
                if scored[i] != j {
                    scored[i] = j;
                    if let Some(score) = overlap(&ranked[i], words, threshold) {
                        found.push((score, i, j));
                    }
                }
            }
            earlier.push(j);
        }
    }
    ordered(found)
}

/// Each set's words as their ranks, rarest first: a word's rank is its place among every word
/// of `sets`, ordered by how many sets hold it and then by the word itself.
fn ranked(sets: &[BTreeSet<String>]) -> Vec<Vec<u32>> {
    let mut frequency: HashMap<&str, usize> = HashMap::new();
    for set in sets {
        for word in set {
            *frequency.entry(word.as_str()).or_default() += 1;
        }
    }
    let mut order: Vec<(usize, &str)> = frequency.into_iter().map(|(w, n)| (n, w)).collect();
    order.sort_unstable();
    let rank: HashMap<&str, u32> = order
        .into_iter()
        .enumerate()
        .map(|(at, (_, word))| (word, at as u32))
        .collect();
    sets.iter()
        .map(|set| {
            let mut words: Vec<u32> = set.iter().map(|w| rank[w.as_str()]).collect();
            words.sort_unstable();
            words
        })
        .collect()
}

/// [`scores`] over two sets of ranks, each sorted: the same integers, so the same score. Two
/// sets whose sizes alone keep them under the threshold (Jaccard is at most the smaller size
/// over the larger) are not counted.
fn overlap(a: &[u32], b: &[u32], threshold: f64) -> Option<f64> {
    let (small, large) = (a.len().min(b.len()), a.len().max(b.len()));
    if small == 0 || (small as f64) / (large as f64) < threshold - 1e-12 {
        return None;
    }
    let (mut x, mut y, mut shared) = (0, 0, 0);
    while x < a.len() && y < b.len() {
        match a[x].cmp(&b[y]) {
            std::cmp::Ordering::Less => x += 1,
            std::cmp::Ordering::Greater => y += 1,
            std::cmp::Ordering::Equal => {
                shared += 1;
                x += 1;
                y += 1;
            }
        }
    }
    let jaccard = shared as f64 / (a.len() + b.len() - shared) as f64;
    (jaccard >= threshold).then_some(jaccard)
}

/// How many of a set's `len` words, rarest first, it needs to look under: `len − ⌈t·len⌉ + 1`.
/// `⌈t·len⌉` is taken a hair low, so a rounding error lengthens the prefix and never shortens it.
fn prefix(len: usize, threshold: f64) -> usize {
    let needed = (threshold * len as f64 - 1e-9).ceil().max(0.0) as usize;
    if needed > len { 0 } else { len - needed + 1 }.min(len)
}

/// `a` and `b`'s Jaccard when it reaches `threshold`, computed exactly as the full comparison
/// computes it, so the two agree on a pair at the threshold itself.
fn scores(a: &BTreeSet<String>, b: &BTreeSet<String>, threshold: f64) -> Option<f64> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    let shared = a.intersection(b).count();
    let jaccard = shared as f64 / (a.len() + b.len() - shared) as f64;
    (jaccard >= threshold).then_some(jaccard)
}

/// The full comparison: every pair, in listing order.
fn every_pair(sets: &[BTreeSet<String>], threshold: f64) -> Vec<(f64, usize, usize)> {
    let mut found = Vec::new();
    for (i, a) in sets.iter().enumerate() {
        for (j, b) in sets.iter().enumerate().skip(i + 1) {
            if let Some(score) = scores(a, b, threshold) {
                found.push((score, i, j));
            }
        }
    }
    found
}

/// Strongest first; pairs in listing order within a score, as Python's stable sort leaves them.
fn ordered(mut found: Vec<(f64, usize, usize)>) -> Vec<(f64, usize, usize)> {
    found.sort_by_key(|&(_, i, j)| (i, j));
    found.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn set(words: &[&str]) -> BTreeSet<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    /// Word sets from a small vocabulary, so that most pairs share words and many sit near
    /// any threshold.
    fn stores() -> impl Strategy<Value = Vec<BTreeSet<String>>> {
        proptest::collection::vec(proptest::collection::btree_set("[a-h]{1,2}", 0..9), 0..40)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        #[test]
        fn the_candidates_find_every_pair_the_full_comparison_finds(
            sets in stores(),
            threshold in prop_oneof![
                Just(0.5), Just(1.0 / 3.0), Just(2.0 / 3.0), Just(1.0), Just(0.0),
                0.01f64..1.0,
            ],
        ) {
            prop_assert_eq!(pairs(&sets, threshold), ordered(every_pair(&sets, threshold)));
        }
    }

    #[test]
    fn a_pair_at_exactly_the_threshold_is_found() {
        // Two words of four shared, then one of three: each pair sits on its threshold.
        let sets = vec![
            set(&["alpha", "beta", "gamma"]),
            set(&["alpha", "beta", "delta"]),
        ];
        assert_eq!(pairs(&sets, 0.5), vec![(0.5, 0, 1)]);
        let sets = vec![set(&["alpha", "beta"]), set(&["alpha", "gamma"])];
        assert_eq!(pairs(&sets, 1.0 / 3.0), vec![(1.0 / 3.0, 0, 1)]);
    }

    #[test]
    fn at_a_threshold_of_zero_a_pair_sharing_no_word_counts_and_an_empty_set_never_does() {
        let sets = vec![set(&["alpha"]), set(&["beta"]), set(&[])];
        assert_eq!(pairs(&sets, 0.0), vec![(0.0, 0, 1)]);
        assert_eq!(pairs(&sets, f64::NAN), vec![]);
    }

    #[test]
    fn above_one_not_even_two_equal_sets_are_a_pair() {
        let sets = vec![set(&["alpha", "beta"]), set(&["alpha", "beta"])];
        assert_eq!(pairs(&sets, 1.0), vec![(1.0, 0, 1)]);
        assert_eq!(pairs(&sets, 1.5), vec![]);
    }
}
