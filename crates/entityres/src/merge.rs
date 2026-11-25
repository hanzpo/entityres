use crate::compare::{normalize, number, value_text};
use crate::config::{Config, MergeRule, Record};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub struct Group {
    pub value: Value,
    pub members: Vec<usize>,
}

pub enum Pick {
    Value(Value, Vec<usize>),
    Ask(Vec<Group>),
}

fn group(vals: &[(usize, &Value)], cfg: &Config, field: &str) -> Vec<Group> {
    let mut out: Vec<(String, Group)> = Vec::new();
    for &(m, v) in vals {
        let Some(norm) = normalize(v, cfg.normalize_of(field)) else {
            continue;
        };
        match out.iter_mut().find(|(n, _)| *n == norm) {
            Some((_, g)) => g.members.push(m),
            None => out.push((
                norm,
                Group {
                    value: v.clone(),
                    members: vec![m],
                },
            )),
        }
    }
    out.into_iter().map(|(_, g)| g).collect()
}

fn newer(a: &Value, b: &Value) -> Ordering {
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        _ => value_text(a).cmp(&value_text(b)),
    }
}

pub fn pick(
    members: &[usize],
    records: &[Record],
    cfg: &Config,
    field: &str,
    answer: impl FnOnce(&[Group]) -> Option<usize>,
) -> Option<Pick> {
    let vals: Vec<(usize, &Value)> = members
        .iter()
        .filter_map(|&m| records[m].fields.get(field).map(|v| (m, v)))
        .collect();
    let rule = cfg.merge_of(field);
    if rule == MergeRule::Union {
        let flat: Vec<(usize, &Value)> = vals
            .iter()
            .flat_map(|&(m, v)| match v {
                Value::Array(a) => a.iter().map(|x| (m, x)).collect::<Vec<_>>(),
                _ => vec![(m, v)],
            })
            .collect();
        let groups = group(&flat, cfg, field);
        if groups.is_empty() {
            return None;
        }
        let mut from: Vec<usize> = groups.iter().flat_map(|g| g.members.clone()).collect();
        from.sort_unstable();
        from.dedup();
        return Some(Pick::Value(
            Value::Array(groups.into_iter().map(|g| g.value).collect()),
            from,
        ));
    }
    let mut groups = group(&vals, cfg, field);
    if groups.len() <= 1 {
        return groups.pop().map(|g| Pick::Value(g.value, g.members));
    }
    let chosen = match rule {
        MergeRule::SourcePriority | MergeRule::Union => 0,
        MergeRule::MostCommon => {
            let best = groups.iter().map(|g| g.members.len()).max().unwrap_or(0);
            groups
                .iter()
                .position(|g| g.members.len() == best)
                .unwrap_or(0)
        }
        MergeRule::Longest => {
            let len = |g: &Group| value_text(&g.value).map_or(0, |t| t.chars().count());
            let best = groups.iter().map(len).max().unwrap_or(0);
            groups.iter().position(|g| len(g) == best).unwrap_or(0)
        }
        MergeRule::Newest => {
            let updated = cfg.merge.updated_field.as_deref().unwrap_or("updated_at");
            let mut latest: Option<(usize, &Value)> = None;
            for g in &groups {
                for &m in &g.members {
                    if let Some(u) = records[m].fields.get(updated).filter(|u| !u.is_null()) {
                        if latest.is_none_or(|(_, l)| newer(u, l) == Ordering::Greater) {
                            latest = Some((m, u));
                        }
                    }
                }
            }
            latest
                .and_then(|(m, _)| groups.iter().position(|g| g.members.contains(&m)))
                .unwrap_or(0)
        }
        MergeRule::Judge => match answer(&groups) {
            Some(i) if i < groups.len() => i,
            _ => return Some(Pick::Ask(groups)),
        },
    };
    let g = groups.swap_remove(chosen);
    Some(Pick::Value(g.value, g.members))
}

pub fn fields_of(members: &[usize], records: &[Record]) -> Vec<String> {
    let mut all: BTreeMap<&str, ()> = BTreeMap::new();
    for &m in members {
        for k in records[m].fields.keys() {
            all.insert(k, ());
        }
    }
    all.into_keys().map(str::to_string).collect()
}
