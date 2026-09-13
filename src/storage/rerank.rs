use std::collections::HashMap;

use crate::schema::Memory;

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

fn compute_link_counts(
    candidates: &[Memory],
    all_memories: &[Memory],
) -> HashMap<(String, String), usize> {
    let mut incoming: HashMap<(String, String), usize> = HashMap::new();
    for mem in all_memories {
        for relation in &mem.relations {
            let key = resolve_relation_key(&mem.bundle, relation);
            *incoming.entry(key).or_insert(0) += 1;
        }
    }

    let mut counts = HashMap::new();
    for mem in candidates {
        let outgoing = mem.relations.len();
        let key = (mem.bundle.clone(), mem.slug.clone());
        let incoming_count = incoming.get(&key).copied().unwrap_or(0);
        counts.insert(key, outgoing + incoming_count);
    }
    counts
}

fn assign_ranks<T, K: Ord, F: Fn(&T) -> K>(items: &[T], key: F) -> Vec<usize> {
    let mut indexed: Vec<(usize, &T)> = items.iter().enumerate().collect();
    indexed.sort_by_key(|(_, a)| key(a));

    let mut ranks = vec![0; items.len()];
    for (rank, (idx, _)) in indexed.iter().enumerate() {
        ranks[*idx] = rank + 1;
    }
    ranks
}

pub fn rerank_memories(candidates: Vec<Memory>, all_memories: &[Memory]) -> Vec<Memory> {
    let mut unique: Vec<Memory> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for mem in candidates {
        if seen.insert((mem.bundle.clone(), mem.slug.clone())) {
            unique.push(mem);
        }
    }

    let link_counts = compute_link_counts(&unique, all_memories);

    let sim_ranks = assign_ranks(&unique, |m| m.slug.clone());
    let mut link_indexed: Vec<(usize, usize)> = unique
        .iter()
        .enumerate()
        .map(|(i, m)| {
            (
                i,
                link_counts
                    .get(&(m.bundle.clone(), m.slug.clone()))
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect();
    link_indexed.sort_by_key(|b| std::cmp::Reverse(b.1));
    let mut link_ranks = vec![0; unique.len()];
    for (rank, (idx, _)) in link_indexed.iter().enumerate() {
        link_ranks[*idx] = rank + 1;
    }

    let mut rec_indexed: Vec<(usize, chrono::DateTime<chrono::Utc>)> = unique
        .iter()
        .enumerate()
        .map(|(i, m)| (i, m.updated_at))
        .collect();
    rec_indexed.sort_by_key(|b| std::cmp::Reverse(b.1));
    let mut rec_ranks = vec![0; unique.len()];
    for (rank, (idx, _)) in rec_indexed.iter().enumerate() {
        rec_ranks[*idx] = rank + 1;
    }

    let mut read_indexed: Vec<(usize, u64)> = unique
        .iter()
        .enumerate()
        .map(|(i, m)| (i, m.read_count))
        .collect();
    read_indexed.sort_by_key(|b| std::cmp::Reverse(b.1));
    let mut read_ranks = vec![0; unique.len()];
    for (rank, (idx, _)) in read_indexed.iter().enumerate() {
        read_ranks[*idx] = rank + 1;
    }

    let mut scored: Vec<(usize, f64)> = unique
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let sim_score = W_SIMILARITY / (RRF_K + sim_ranks[i] as f64);
            let link_score = W_LINKS / (RRF_K + link_ranks[i] as f64);
            let rec_score = W_RECENCY / (RRF_K + rec_ranks[i] as f64);
            let read_score = W_READ_COUNT / (RRF_K + read_ranks[i] as f64);
            (i, sim_score + link_score + rec_score + read_score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored.into_iter().map(|(i, _)| unique[i].clone()).collect()
}
