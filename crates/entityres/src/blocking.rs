use crate::compare::{fold, normalize};
use crate::config::{Config, Record, Transform};
use std::collections::{BTreeMap, BTreeSet};

fn block_keys(r: &Record, field: &str, t: Transform, len: usize, cfg: &Config) -> BTreeSet<String> {
    let Some(norm) = r
        .fields
        .get(field)
        .and_then(|v| normalize(v, cfg.normalize_of(field)))
    else {
        return BTreeSet::new();
    };
    match t {
        Transform::Exact => BTreeSet::from([norm]),
        Transform::Prefix => BTreeSet::from([norm.chars().take(len).collect()]),
        Transform::Tokens => fold(&norm).split(' ').map(str::to_string).collect(),
        Transform::Digits => {
            let d: String = norm.chars().filter(char::is_ascii_digit).collect();
            if d.is_empty() {
                BTreeSet::new()
            } else {
                BTreeSet::from([d])
            }
        }
    }
}

pub fn candidates(records: &[Record], cfg: &Config) -> BTreeSet<(usize, usize)> {
    let n = records.len();
    let mut out = BTreeSet::new();
    if cfg.blocking.is_empty() {
        for i in 0..n {
            for j in i + 1..n {
                out.insert((i, j));
            }
        }
    }
    let rules = cfg
        .blocking
        .iter()
        .map(|r| (r.field.as_str(), r.transform, r.n))
        .chain(cfg.keys.iter().map(|k| (k.as_str(), Transform::Exact, 0)));
    for (field, transform, len) in rules {
        let mut blocks: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, r) in records.iter().enumerate() {
            for key in block_keys(r, field, transform, len, cfg) {
                blocks.entry(key).or_default().push(i);
            }
        }
        for ids in blocks
            .values()
            .filter(|ids| ids.len() <= cfg.routing.max_block)
        {
            for (a, &x) in ids.iter().enumerate() {
                for &y in &ids[a + 1..] {
                    out.insert((x.min(y), x.max(y)));
                }
            }
        }
    }
    out
}
