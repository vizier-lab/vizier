use std::collections::HashMap;

use crate::schema::{Memory, MemoryPassageResult};

const RRF_K: f64 = 60.0;
const W_SIMILARITY: f64 = 2.0;
const W_LINKS: f64 = 1.0;
const W_RECENCY: f64 = 1.0;
const W_READ_COUNT: f64 = 1.0;

/// Resolves a relation string (as stored in canonical form: `path/to/concept.md` for
/// same-bundle, `bundle/slug` or `bundle` for cross-bundle) to the `(bundle, path)` key it
/// targets, relative to the linking memory's own bundle.
fn resolve_relation_key(source_bundle: &str, relation: &str) -> (String, String) {
    if let Some(path) = relation.strip_suffix(".md") {
        return (source_bundle.to_string(), path.to_string());
    }
    if let Some((bundle, slug)) = relation.split_once('/') {
        return (bundle.to_string(), slug.to_string());
    }
    (relation.to_string(), String::new())
}

/// Incoming + outgoing link count per document, over the whole corpus.
fn compute_link_counts(all_memories: &[Memory]) -> HashMap<(String, String), usize> {
    let mut incoming: HashMap<(String, String), usize> = HashMap::new();
    for mem in all_memories {
        for relation in &mem.relations {
            let key = resolve_relation_key(&mem.bundle, relation);
            *incoming.entry(key).or_insert(0) += 1;
        }
    }

    let mut counts = HashMap::new();
    for mem in all_memories {
        let key = (mem.bundle.clone(), mem.slug.clone());
        let incoming_count = incoming.get(&key).copied().unwrap_or(0);
        counts.insert(key, mem.relations.len() + incoming_count);
    }
    counts
}

/// 1-based ranks over `items`, ordered by `key` descending — highest value gets rank 1.
///
/// **Equal keys get equal ranks**, each group taking the average of the positions it spans. This
/// matters more than it looks: with competition ranking, two passages of identical relevance would
/// be split by their arbitrary fetch order, and because similarity carries double weight that
/// arbitrary split outweighed every real signal behind it — a stale, never-read document beat a
/// fresh, often-read one on an exact score tie. Tied ranks make a tie a tie, so the remaining
/// signals are what actually decide it.
fn ranks_by_desc<T, K: PartialOrd, F: Fn(&T) -> K>(items: &[T], key: F) -> Vec<f64> {
    let mut indexed: Vec<usize> = (0..items.len()).collect();
    indexed.sort_by(|a, b| {
        key(&items[*b])
            .partial_cmp(&key(&items[*a]))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(b))
    });

    let mut ranks = vec![0.0f64; items.len()];
    let mut group_start = 0usize;
    while group_start < indexed.len() {
        let mut group_end = group_start + 1;
        while group_end < indexed.len() {
            let a = key(&items[indexed[group_start]]);
            let b = key(&items[indexed[group_end]]);
            if a.partial_cmp(&b) != Some(std::cmp::Ordering::Equal) {
                break;
            }
            group_end += 1;
        }
        // Average of the 1-based positions this group of equal keys occupies.
        let average = ((group_start + 1) + group_end) as f64 / 2.0;
        for &idx in &indexed[group_start..group_end] {
            ranks[idx] = average;
        }
        group_start = group_end;
    }
    ranks
}

/// What a passage's source document contributes to the blend: link count, recency, read count.
/// Passages themselves carry only the similarity signal.
pub struct SourceSignals {
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub read_count: u64,
}

/// Rank passages by a reciprocal-rank fusion of four signals: the passage's own relevance,
/// plus its source document's link count, recency, and read count.
///
/// The similarity input used to be `assign_ranks(&unique, |m| m.slug.clone())` — **alphabetical
/// order by slug**, at `W_SIMILARITY = 2.0`, double every other signal. So the strongest input to
/// memory recall was the alphabet (research Decision 4). Passage-level ranking could not be built
/// on that, so this is a repair as much as a change, and it alters recall ordering for every
/// existing query independently of chunking (SC-002).
///
/// `candidates` pairs each passage with its source document's signals. Deduplication by
/// `(bundle, path, ordinal)` happens here, because over-fetching can return the same passage
/// twice when a document appears under more than one key.
pub fn rerank_passages(
    candidates: Vec<(MemoryPassageResult, SourceSignals)>,
    all_memories: &[Memory],
) -> Vec<MemoryPassageResult> {
    let mut unique: Vec<(MemoryPassageResult, SourceSignals)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (passage, signals) in candidates {
        if seen.insert((
            passage.bundle.clone(),
            passage.path.clone(),
            passage.ordinal,
        )) {
            unique.push((passage, signals));
        }
    }
    if unique.len() <= 1 {
        return unique.into_iter().map(|(p, _)| p).collect();
    }

    let link_counts = compute_link_counts(all_memories);

    let sim_ranks = ranks_by_desc(&unique, |(p, _)| p.score);
    let link_ranks = ranks_by_desc(&unique, |(p, _)| {
        link_counts
            .get(&(p.bundle.clone(), p.path.clone()))
            .copied()
            .unwrap_or(0)
    });
    let rec_ranks = ranks_by_desc(&unique, |(_, s)| s.updated_at);
    let read_ranks = ranks_by_desc(&unique, |(_, s)| s.read_count);

    let mut scored: Vec<(usize, f64)> = (0..unique.len())
        .map(|i| {
            let blended = W_SIMILARITY / (RRF_K + sim_ranks[i])
                + W_LINKS / (RRF_K + link_ranks[i])
                + W_RECENCY / (RRF_K + rec_ranks[i])
                + W_READ_COUNT / (RRF_K + read_ranks[i]);
            (i, blended)
        })
        .collect();

    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    let mut passages: Vec<Option<MemoryPassageResult>> =
        unique.into_iter().map(|(p, _)| Some(p)).collect();
    scored
        .into_iter()
        .filter_map(|(i, _)| passages[i].take())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(path: &str, ordinal: usize, score: f64) -> MemoryPassageResult {
        MemoryPassageResult {
            bundle: "default".into(),
            path: path.into(),
            title: path.into(),
            ordinal,
            ordinal_end: ordinal,
            line_start: 1,
            line_end: 2,
            text: format!("text of {path}#{ordinal}"),
            score,
            truncated: false,
        }
    }

    /// A fixed epoch, so `days_ago` is the only thing that varies. Deriving from `Utc::now()`
    /// per call made every candidate differ by nanoseconds, which is a recency signal the test did
    /// not mean to set.
    fn signals(days_ago: i64, read_count: u64) -> SourceSignals {
        let epoch = chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .expect("fixed epoch is valid");
        SourceSignals {
            updated_at: epoch - chrono::Duration::days(days_ago),
            read_count,
        }
    }

    /// The repair itself: the highest-scoring passage wins even though its path sorts last
    /// alphabetically, which is what the old similarity rank would have ordered on.
    #[test]
    fn the_best_scoring_passage_ranks_first_regardless_of_alphabetical_order() {
        let candidates = vec![
            (passage("aaa", 0, 0.10), signals(1, 0)),
            (passage("zzz", 0, 0.95), signals(1, 0)),
            (passage("mmm", 0, 0.50), signals(1, 0)),
        ];
        let out = rerank_passages(candidates, &[]);
        assert_eq!(
            out.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(),
            vec!["zzz", "mmm", "aaa"],
            "score decides the whole order, not the alphabet"
        );
    }

    #[test]
    fn duplicate_passages_are_collapsed_by_bundle_path_and_ordinal() {
        let candidates = vec![
            (passage("note", 2, 0.8), signals(1, 0)),
            (passage("note", 2, 0.8), signals(1, 0)),
            (passage("note", 3, 0.7), signals(1, 0)),
        ];
        let out = rerank_passages(candidates, &[]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].ordinal, 2);
        assert_eq!(out[1].ordinal, 3);
    }

    /// On an exact score tie the similarity signal must be *tied*, not resolved by fetch order.
    /// Before tied ranks, the arbitrary order won at double weight and this came out backwards.
    #[test]
    fn recency_and_read_count_still_break_a_tie_on_score() {
        let candidates = vec![
            (passage("stale", 0, 0.5), signals(400, 0)),
            (passage("fresh", 0, 0.5), signals(1, 9)),
        ];
        let out = rerank_passages(candidates, &[]);
        assert_eq!(
            out[0].path, "fresh",
            "the blend still carries recency and read count"
        );
    }

    #[test]
    fn a_single_candidate_passes_through_untouched() {
        let out = rerank_passages(vec![(passage("only", 0, 0.3), signals(1, 0))], &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "only");
    }

    #[test]
    fn ranking_is_deterministic_for_equal_signals() {
        let build = || {
            vec![
                (passage("a", 0, 0.5), signals(1, 0)),
                (passage("b", 0, 0.5), signals(1, 0)),
                (passage("c", 0, 0.5), signals(1, 0)),
            ]
        };
        let first: Vec<String> = rerank_passages(build(), &[])
            .into_iter()
            .map(|p| p.path)
            .collect();
        let second: Vec<String> = rerank_passages(build(), &[])
            .into_iter()
            .map(|p| p.path)
            .collect();
        assert_eq!(first, second);
    }
}
