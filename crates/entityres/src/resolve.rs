use crate::blocking::candidates;
use crate::cluster::{cluster, Edge};
use crate::compare::{digit_groups, feature, level_count, normalize, value_text, Feature};
use crate::config::{Answer, Compare, Config, FieldConfig, Input, Record, Routing, VetoRule};
use crate::fs::{estimate_u, gamma, Gamma, Model};
use crate::merge::{fields_of, pick, Pick};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Match,
    NoMatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum By {
    Key,
    Veto,
    Judge,
    Fs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Flag {
    LowEvidence,
    Conflict,
    SemanticMismatch,
    ClusterConflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Pair,
    Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct Pair {
    pub id: String,
    pub a: String,
    pub b: String,
    pub p: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judge: Option<f64>,
    pub decision: Decision,
    pub by: By,
    pub unsure: bool,
    pub flags: Vec<Flag>,
    pub weights: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entity {
    pub id: String,
    pub records: Vec<String>,
    pub fields: BTreeMap<String, Value>,
    pub from: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Question {
    pub id: String,
    pub kind: Kind,
    pub priority: f64,
    pub state: Value,
    pub questions: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub pair: String,
    pub blocked_by: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub records: usize,
    pub candidates: usize,
    pub judged: usize,
    pub unsure: usize,
    pub trusted: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldModel {
    pub m: Vec<f64>,
    pub u: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelOut {
    pub lambda: f64,
    pub fields: BTreeMap<String, FieldModel>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Output {
    pub entities: Vec<Entity>,
    pub pairs: Vec<Pair>,
    pub pending: Vec<Question>,
    pub conflicts: Vec<Conflict>,
    pub stats: Stats,
    pub model: ModelOut,
}

const BUCKETS: usize = 10;
const CLUSTER_PRIORITY: f64 = 100.0;

fn pair_id(a: &str, b: &str) -> String {
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    format!("pair:{x}|{y}")
}

fn fnv(parts: &[String]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain([0x1f]) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn bucket(p: f64) -> usize {
    ((p * BUCKETS as f64) as usize).min(BUCKETS - 1)
}

fn decide(p: f64) -> Decision {
    if p >= 0.5 {
        Decision::Match
    } else {
        Decision::NoMatch
    }
}

type Parsed = (BTreeMap<String, f64>, BTreeMap<String, usize>);

fn parse_answers(answers: &[Answer]) -> Result<Parsed, Error> {
    let mut pairs = BTreeMap::new();
    let mut values = BTreeMap::new();
    for a in answers {
        if a.id.starts_with("pair:") {
            let p = a.answers["same"]["noul"]
                .as_f64()
                .ok_or_else(|| Error(format!("answer {} has no same.noul", a.id)))?;
            pairs.insert(a.id.clone(), p.clamp(0.0, 1.0));
        } else if a.id.starts_with("value:") {
            let choice = a.answers["pick"]["choice"]
                .as_str()
                .and_then(|c| c.parse().ok())
                .ok_or_else(|| Error(format!("answer {} has no pick.choice", a.id)))?;
            values.insert(a.id.clone(), choice);
        } else {
            return Err(Error(format!("unknown answer id {}", a.id)));
        }
    }
    Ok((pairs, values))
}

fn vetoed(a: &Record, b: &Record, cfg: &Config) -> bool {
    cfg.vetoes.iter().any(|v| {
        let field = v.field.as_deref().unwrap_or_default();
        let (x, y) = (a.fields.get(field), b.fields.get(field));
        match v.rule {
            VetoRule::SameSource => a.source == b.source,
            VetoRule::Different => {
                let how = cfg.normalize_of(field);
                match (
                    x.and_then(|x| normalize(x, how)),
                    y.and_then(|y| normalize(y, how)),
                ) {
                    (Some(x), Some(y)) => x != y,
                    _ => false,
                }
            }
            VetoRule::NumbersDiffer => match (x.map(digit_groups), y.map(digit_groups)) {
                (Some(x), Some(y)) => !x.is_empty() && !y.is_empty() && x != y,
                _ => false,
            },
        }
    })
}

fn key_match(a: &Record, b: &Record, cfg: &Config) -> bool {
    cfg.keys.iter().any(|k| {
        let how = cfg.normalize_of(k);
        match (
            a.fields.get(k).and_then(|x| normalize(x, how)),
            b.fields.get(k).and_then(|y| normalize(y, how)),
        ) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    })
}

fn flags_for(
    g: &Gamma,
    w: &[Option<f64>],
    fcs: &[&FieldConfig],
    r: &Routing,
    prior: f64,
) -> Vec<Flag> {
    let mut out = Vec::new();
    if g.iter().flatten().count() < r.min_fields {
        out.push(Flag::LowEvidence);
    }
    let pos: f64 = w.iter().flatten().filter(|x| **x > 0.0).sum();
    let neg: f64 = w.iter().flatten().filter(|x| **x < 0.0).sum();
    if pos >= r.conflict && -neg >= r.conflict {
        out.push(Flag::Conflict);
    }
    let missed: Vec<bool> = fcs
        .iter()
        .zip(g)
        .map(|(fc, l)| fc.semantic && *l == Some(level_count(fc) as u8 - 1))
        .collect();
    let rest: f64 = w
        .iter()
        .zip(&missed)
        .filter_map(|(w, m)| (!m).then_some(*w).flatten())
        .sum();
    if missed.contains(&true) && prior + rest >= 0.0 {
        out.push(Flag::SemanticMismatch);
    }
    out
}

fn suspicious(p: f64, flags: &[Flag], r: &Routing) -> bool {
    flags.iter().any(|f| match f {
        Flag::Conflict | Flag::ClusterConflict => true,
        Flag::LowEvidence => p >= r.accept,
        Flag::SemanticMismatch => p <= r.reject,
    })
}

fn record_state(r: &Record, key: &str) -> Value {
    json!({ "id": key, "source": r.source, "fields": r.fields })
}

fn source_rank(cfg: &Config, source: &str) -> usize {
    cfg.sources
        .iter()
        .position(|s| s == source)
        .unwrap_or(cfg.sources.len())
}

pub fn run(input: &Input) -> Result<Output, Error> {
    let cfg = &input.config;
    let r = &cfg.routing;
    let records = &input.records;
    let entity = cfg.entity();
    let keys: Vec<String> = records
        .iter()
        .map(|x| format!("{}:{}", x.source, x.id))
        .collect();
    let mut seen = BTreeSet::new();
    for k in &keys {
        if !seen.insert(k) {
            return Err(Error(format!("duplicate record {k}")));
        }
    }
    for v in &cfg.vetoes {
        if v.rule != VetoRule::SameSource && v.field.is_none() {
            return Err(Error(format!("veto {:?} needs a field", v.rule)));
        }
    }
    let (pair_answers, value_answers) = parse_answers(&input.answers)?;

    let compared: Vec<(&str, &FieldConfig)> = cfg
        .fields
        .iter()
        .filter(|(_, f)| f.compare != Compare::None)
        .map(|(k, f)| (k.as_str(), f))
        .collect();
    let fcs: Vec<&FieldConfig> = compared.iter().map(|c| c.1).collect();
    let levels: Vec<usize> = fcs.iter().map(|f| level_count(f)).collect();
    let feats: Vec<Vec<Option<Feature>>> = records
        .iter()
        .map(|x| {
            compared
                .iter()
                .map(|(name, fc)| feature(x.fields.get(*name), fc))
                .collect()
        })
        .collect();

    let cands: Vec<(usize, usize)> = candidates(records, cfg).into_iter().collect();
    let veto: Vec<bool> = cands
        .iter()
        .map(|&(i, j)| vetoed(&records[i], &records[j], cfg))
        .collect();
    let keyed: Vec<bool> = cands
        .iter()
        .map(|&(i, j)| key_match(&records[i], &records[j], cfg))
        .collect();
    let ids: Vec<String> = cands
        .iter()
        .map(|&(i, j)| pair_id(&keys[i], &keys[j]))
        .collect();
    let judged: Vec<Option<f64>> = ids.iter().map(|id| pair_answers.get(id).copied()).collect();
    let gammas: Vec<Gamma> = cands
        .iter()
        .map(|&(i, j)| gamma(&feats[i], &feats[j], &fcs))
        .collect();

    let u = estimate_u(&feats, &fcs, &levels, r.u_sample);
    let (fit_g, fit_l): (Vec<Gamma>, Vec<Option<bool>>) = (0..cands.len())
        .filter(|&k| !veto[k])
        .map(|k| {
            let label = if keyed[k] {
                Some(true)
            } else {
                judged[k].map(|p| p >= 0.5)
            };
            (gammas[k].clone(), label)
        })
        .unzip();
    let model = Model::fit(&fit_g, &fit_l, u, r.em_iterations);

    let scored: Vec<(f64, Vec<Option<f64>>)> = gammas.iter().map(|g| model.score(g)).collect();
    let mut agree = [(0usize, 0usize); BUCKETS];
    for k in 0..cands.len() {
        if let (Some(j), false, false) = (judged[k], veto[k], keyed[k]) {
            let p = scored[k].0;
            let slot = &mut agree[bucket(p)];
            slot.0 += 1;
            slot.1 += usize::from(decide(p) == decide(j));
        }
    }
    let trusted: Vec<bool> = agree
        .iter()
        .map(|&(n, a)| n >= r.trust_labels && n > 0 && a as f64 / n as f64 >= r.trust_agreement)
        .collect();

    let prior = (model.lambda / (1.0 - model.lambda)).ln();
    let mut pairs: Vec<Pair> = Vec::with_capacity(cands.len());
    for (k, &(i, j)) in cands.iter().enumerate() {
        let (p, w) = &scored[k];
        let p = *p;
        let flags = flags_for(&gammas[k], w, &fcs, r, prior);
        let (decision, by, unsure) = if veto[k] {
            (Decision::NoMatch, By::Veto, false)
        } else if keyed[k] {
            (Decision::Match, By::Key, false)
        } else if let Some(jp) = judged[k] {
            (decide(jp), By::Judge, false)
        } else if !suspicious(p, &flags, r) && p >= r.accept {
            (Decision::Match, By::Fs, false)
        } else if !suspicious(p, &flags, r) && p <= r.reject {
            (Decision::NoMatch, By::Fs, false)
        } else if flags.is_empty() && trusted[bucket(p)] {
            (decide(p), By::Fs, false)
        } else {
            (decide(p), By::Fs, true)
        };
        let (a, b) = if keys[i] <= keys[j] { (i, j) } else { (j, i) };
        pairs.push(Pair {
            id: ids[k].clone(),
            a: keys[a].clone(),
            b: keys[b].clone(),
            p,
            judge: judged[k],
            decision,
            by,
            unsure,
            flags,
            weights: compared
                .iter()
                .zip(w)
                .filter_map(|((name, _), w)| w.map(|w| (name.to_string(), w)))
                .collect(),
        });
    }

    let edges: Vec<Edge> = pairs
        .iter()
        .enumerate()
        .filter(|(_, x)| x.decision == Decision::Match)
        .map(|(k, x)| Edge {
            pair: k,
            i: cands[k].0,
            j: cands[k].1,
            hard: matches!(x.by, By::Key | By::Judge),
            p: x.judge.unwrap_or(x.p),
        })
        .collect();
    let cannot: Vec<(usize, usize, usize)> = pairs
        .iter()
        .enumerate()
        .filter(|(_, x)| {
            x.decision == Decision::NoMatch && (matches!(x.by, By::Veto | By::Judge) || !x.unsure)
        })
        .map(|(k, _)| (cands[k].0, cands[k].1, k))
        .collect();
    let clusters = cluster(records.len(), edges, &cannot);

    let mut ask: BTreeMap<usize, f64> = BTreeMap::new();
    let mut conflicts = Vec::new();
    for &(edge, by) in &clusters.blocked {
        let target = [edge, by].into_iter().find(|&k| pairs[k].by == By::Fs);
        match target {
            Some(k) => {
                let x = &mut pairs[k];
                if !x.flags.contains(&Flag::ClusterConflict) {
                    x.flags.push(Flag::ClusterConflict);
                }
                x.unsure = true;
                if k == edge {
                    x.decision = Decision::NoMatch;
                }
                ask.insert(k, CLUSTER_PRIORITY + x.p);
            }
            None => conflicts.push(Conflict {
                pair: pairs[edge].id.clone(),
                blocked_by: pairs[by].id.clone(),
            }),
        }
    }

    let mut size = vec![1usize; records.len()];
    for g in &clusters.groups {
        for &m in g {
            size[m] = g.len();
        }
    }
    let group_of: BTreeMap<usize, usize> = clusters
        .groups
        .iter()
        .enumerate()
        .flat_map(|(gi, g)| g.iter().map(move |&m| (m, gi)))
        .collect();
    for (k, x) in pairs.iter().enumerate() {
        if !x.unsure || ask.contains_key(&k) {
            continue;
        }
        let (i, j) = cands[k];
        let impact = if group_of[&i] == group_of[&j] {
            size[i]
        } else {
            size[i] + size[j]
        };
        let flagged = !x.flags.is_empty();
        let unc = (1.0 - (2.0 * x.p - 1.0).abs()).max(if flagged { 0.25 } else { 0.0 });
        let boost = 1.0 + 0.5 * x.flags.len() as f64;
        ask.insert(k, unc * boost * (1.0 + (impact as f64).ln()));
    }
    let mut ranked: Vec<(usize, f64)> = ask.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(pairs[a.0].id.cmp(&pairs[b.0].id)));
    let mut pending: Vec<Question> = ranked
        .into_iter()
        .take(r.budget)
        .map(|(k, priority)| {
            let (i, j) = cands[k];
            let (a, b) = if keys[i] <= keys[j] { (i, j) } else { (j, i) };
            Question {
                id: pairs[k].id.clone(),
                kind: Kind::Pair,
                priority,
                state: json!({
                    "entity": entity,
                    "a": record_state(&records[a], &keys[a]),
                    "b": record_state(&records[b], &keys[b]),
                }),
                questions: json!({
                    "same": {
                        "type": "noul",
                        "instructions": format!(
                            "Do A and B describe the same {entity}? Each source formats, abbreviates and leaves out fields in its own way, so a missing or differently written field is not evidence by itself."
                        ),
                        "criteria": {
                            "true": format!("A and B are the same {entity}."),
                            "false": "A and B are different.",
                        },
                    }
                }),
            }
        })
        .collect();

    let mut value_questions = Vec::new();
    let mut entities = Vec::with_capacity(clusters.groups.len());
    for g in &clusters.groups {
        let mut members = g.clone();
        members.sort_by(|&a, &b| {
            source_rank(cfg, &records[a].source)
                .cmp(&source_rank(cfg, &records[b].source))
                .then(keys[a].cmp(&keys[b]))
        });
        let mut record_keys: Vec<String> = g.iter().map(|&m| keys[m].clone()).collect();
        record_keys.sort();
        let id = record_keys[0].clone();
        let mut fields = BTreeMap::new();
        let mut from = BTreeMap::new();
        for field in fields_of(&members, records) {
            let mut qid = String::new();
            let picked = pick(&members, records, cfg, &field, |groups| {
                let options: Vec<String> = groups
                    .iter()
                    .map(|g| value_text(&g.value).unwrap_or_default())
                    .collect();
                qid = format!("value:{id}:{field}:{:016x}", fnv(&options));
                value_answers.get(&qid).copied()
            });
            let (value, who) = match picked {
                None => continue,
                Some(Pick::Value(v, who)) => (v, who),
                Some(Pick::Ask(groups)) => {
                    let criteria: serde_json::Map<String, Value> = groups
                        .iter()
                        .enumerate()
                        .map(|(i, g)| {
                            (
                                i.to_string(),
                                Value::String(value_text(&g.value).unwrap_or_default()),
                            )
                        })
                        .collect();
                    value_questions.push(Question {
                        id: qid,
                        kind: Kind::Value,
                        priority: members.len() as f64,
                        state: json!({
                            "entity": entity,
                            "id": id,
                            "field": field,
                            "records": members.iter().map(|&m| record_state(&records[m], &keys[m])).collect::<Vec<_>>(),
                        }),
                        questions: json!({
                            "pick": {
                                "type": "choice",
                                "instructions": format!(
                                    "These records all describe one {entity}. They disagree on \"{field}\". Pick the value that is most correct and complete."
                                ),
                                "criteria": criteria,
                            }
                        }),
                    });
                    let first = groups.into_iter().next().expect("ask has options");
                    (first.value, first.members)
                }
            };
            let mut who: Vec<String> = who.iter().map(|&m| keys[m].clone()).collect();
            who.sort();
            fields.insert(field.clone(), value);
            from.insert(field, who);
        }
        entities.push(Entity {
            id,
            records: record_keys,
            fields,
            from,
        });
    }
    entities.sort_by(|a, b| a.id.cmp(&b.id));
    if pending.is_empty() {
        value_questions.sort_by(|a, b| b.priority.total_cmp(&a.priority).then(a.id.cmp(&b.id)));
        pending = value_questions.into_iter().take(r.budget).collect();
    }

    let stats = Stats {
        records: records.len(),
        candidates: cands.len(),
        judged: pairs.iter().filter(|x| x.by == By::Judge).count(),
        unsure: pairs.iter().filter(|x| x.unsure).count(),
        trusted: trusted
            .iter()
            .enumerate()
            .filter(|(_, t)| **t)
            .map(|(b, _)| {
                format!(
                    "{:.1}-{:.1}",
                    b as f64 / BUCKETS as f64,
                    (b + 1) as f64 / BUCKETS as f64
                )
            })
            .collect(),
    };
    let model_out = ModelOut {
        lambda: model.lambda,
        fields: compared
            .iter()
            .enumerate()
            .map(|(f, (name, _))| {
                (
                    name.to_string(),
                    FieldModel {
                        m: model.m[f].clone(),
                        u: model.u[f].clone(),
                    },
                )
            })
            .collect(),
    };
    Ok(Output {
        entities,
        pairs,
        pending,
        conflicts,
        stats,
        model: model_out,
    })
}
