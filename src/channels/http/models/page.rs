//! Opt-in paging, search, filtering and counts for list endpoints (spec 024, research R7).
//!
//! A list handler loads its items as it always has, then calls [`paginate`] when the request
//! carries a `limit`; without one it keeps answering with the plain array, so existing clients
//! see no change. Each entity describes itself once through [`Listing`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const MAX_LIMIT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Default, Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
pub struct ListQuery {
    /// Free-text search, matched by the entity's own rule.
    pub q: Option<String>,
    /// One filter key (e.g. `active`); `all` or absent means no filter.
    pub filter: Option<String>,
    /// The entity's sort key; an unknown one uses the default.
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub offset: Option<usize>,
    /// Present switches the response to a `Page`; clamped to 1..=100.
    pub limit: Option<usize>,
}

impl ListQuery {
    pub fn is_paged(&self) -> bool {
        self.limit.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Page<T: Serialize> {
    pub items: Vec<T>,
    /// Matches after `q` and `filter`, before `offset`/`limit`.
    pub total: usize,
    /// Matches per filter key after `q`, ignoring `filter`. Always has `all`.
    pub counts: BTreeMap<String, usize>,
}

/// Sort values compare within one sort key only, so mixing variants never matters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SortKey {
    Num(i64),
    Text(String),
}

impl SortKey {
    pub fn text(s: &str) -> Self {
        SortKey::Text(s.to_lowercase())
    }
}

pub struct ListingError {
    pub code: &'static str,
    pub message: String,
}

pub trait Listing<T> {
    /// Whether `item` matches the search text. `q` arrives lower-cased and non-empty.
    fn matches(&self, item: &T, q: &str) -> bool;

    /// The filter keys that apply to `items` (`all` is implicit), e.g. one per role name.
    fn filter_keys(&self, items: &[T]) -> Vec<String>;

    /// Whether `item` belongs to `filter`. Only called with keys from [`Self::filter_keys`].
    fn in_filter(&self, filter: &str, item: &T) -> bool;

    /// The sort value of `item` under `sort`, which is always a key this entity supports.
    fn sort_key(&self, item: &T, sort: &str) -> SortKey;

    /// The supported sort keys, default first.
    fn sorts(&self) -> &'static [&'static str];

    /// The direction a sort takes when the request gives none.
    fn default_order(&self, _sort: &str) -> SortOrder {
        SortOrder::Asc
    }
}

pub fn paginate<T, L>(
    items: Vec<T>,
    listing: &L,
    query: &ListQuery,
) -> Result<Page<T>, ListingError>
where
    T: Serialize,
    L: Listing<T>,
{
    let q = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    let mut items: Vec<T> = match q {
        Some(q) => {
            let q = q.to_lowercase();
            items.into_iter().filter(|i| listing.matches(i, &q)).collect()
        }
        None => items,
    };

    let keys = listing.filter_keys(&items);
    let filter = query.filter.as_deref().filter(|f| *f != "all");
    if let Some(f) = filter
        && !keys.iter().any(|k| k == f)
    {
        return Err(ListingError {
            code: "unknown_filter",
            message: format!("unknown filter `{f}`"),
        });
    }

    let mut counts = BTreeMap::new();
    counts.insert("all".to_string(), items.len());
    for key in &keys {
        let n = items.iter().filter(|i| listing.in_filter(key, i)).count();
        counts.insert(key.clone(), n);
    }

    if let Some(f) = filter {
        items.retain(|i| listing.in_filter(f, i));
    }

    let sort = query
        .sort
        .as_deref()
        .filter(|s| listing.sorts().contains(s))
        .or_else(|| listing.sorts().first().copied());
    if let Some(sort) = sort {
        let order = query.order.unwrap_or_else(|| listing.default_order(sort));
        items.sort_by(|a, b| {
            let ord = listing.sort_key(a, sort).cmp(&listing.sort_key(b, sort));
            match order {
                SortOrder::Asc => ord,
                SortOrder::Desc => ord.reverse(),
            }
        });
    }

    let total = items.len();
    let limit = query.limit.unwrap_or(MAX_LIMIT).clamp(1, MAX_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let items = items.into_iter().skip(offset).take(limit).collect();

    Ok(Page { items, total, counts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize)]
    struct Row {
        name: &'static str,
        active: bool,
        n: i64,
    }

    struct Rows;

    impl Listing<Row> for Rows {
        fn matches(&self, item: &Row, q: &str) -> bool {
            item.name.to_lowercase().contains(q)
        }
        fn filter_keys(&self, _: &[Row]) -> Vec<String> {
            vec!["active".into()]
        }
        fn in_filter(&self, filter: &str, item: &Row) -> bool {
            filter == "active" && item.active
        }
        fn sort_key(&self, item: &Row, sort: &str) -> SortKey {
            match sort {
                "n" => SortKey::Num(item.n),
                _ => SortKey::text(item.name),
            }
        }
        fn sorts(&self) -> &'static [&'static str] {
            &["name", "n"]
        }
    }

    fn rows() -> Vec<Row> {
        vec![
            Row { name: "Cedar", active: true, n: 3 },
            Row { name: "alder", active: false, n: 9 },
            Row { name: "Birch", active: true, n: 1 },
        ]
    }

    fn names(page: &Page<Row>) -> Vec<&'static str> {
        page.items.iter().map(|r| r.name).collect()
    }

    #[test]
    fn counts_ignore_the_filter_but_follow_the_search() {
        let q = ListQuery {
            filter: Some("active".into()),
            limit: Some(10),
            ..Default::default()
        };
        let page = paginate(rows(), &Rows, &q).ok().unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.counts["all"], 3);
        assert_eq!(page.counts["active"], 2);

        let q = ListQuery {
            q: Some("  AL ".into()),
            limit: Some(10),
            ..Default::default()
        };
        let page = paginate(rows(), &Rows, &q).ok().unwrap();
        assert_eq!(names(&page), ["alder"]);
        assert_eq!(page.counts["all"], 1);
        assert_eq!(page.counts["active"], 0);
    }

    #[test]
    fn unknown_filter_is_refused() {
        let q = ListQuery { filter: Some("nope".into()), ..Default::default() };
        let err = paginate(rows(), &Rows, &q).err().unwrap();
        assert_eq!(err.code, "unknown_filter");
    }

    #[test]
    fn unknown_sort_uses_the_default_and_order_flips() {
        let q = ListQuery { sort: Some("zzz".into()), ..Default::default() };
        let page = paginate(rows(), &Rows, &q).ok().unwrap();
        assert_eq!(names(&page), ["alder", "Birch", "Cedar"]);

        let q = ListQuery {
            sort: Some("n".into()),
            order: Some(SortOrder::Desc),
            ..Default::default()
        };
        let page = paginate(rows(), &Rows, &q).ok().unwrap();
        assert_eq!(names(&page), ["alder", "Cedar", "Birch"]);
    }

    #[test]
    fn limit_is_clamped_and_offset_pages() {
        let many: Vec<Row> = (0..150).map(|n| Row { name: "x", active: true, n }).collect();
        let q = ListQuery { limit: Some(1000), ..Default::default() };
        assert_eq!(paginate(many.clone(), &Rows, &q).ok().unwrap().items.len(), 100);

        let q = ListQuery { limit: Some(0), ..Default::default() };
        assert_eq!(paginate(many.clone(), &Rows, &q).ok().unwrap().items.len(), 1);

        let q = ListQuery { limit: Some(100), offset: Some(120), ..Default::default() };
        let page = paginate(many, &Rows, &q).ok().unwrap();
        assert_eq!(page.items.len(), 30);
        assert_eq!(page.total, 150);
    }
}
