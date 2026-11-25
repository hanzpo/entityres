use crate::config::{Compare, FieldConfig, Normalize};
use serde_json::Value;
use std::collections::BTreeSet;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

pub fn value_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => {
            let t = s.trim();
            (!t.is_empty()).then(|| t.to_string())
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().filter_map(value_text).collect();
            (!parts.is_empty()).then(|| parts.join(" "))
        }
        Value::Object(_) => Some(v.to_string()),
    }
}

pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
    {
        out.push(if c.is_alphanumeric() { c } else { ' ' });
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn normalize(v: &Value, how: Normalize) -> Option<String> {
    let t = value_text(v)?;
    let n = match how {
        Normalize::Text => fold(&t),
        Normalize::Exact => t,
        Normalize::Digits => t.chars().filter(char::is_ascii_digit).collect(),
        Normalize::Email => t.to_lowercase(),
    };
    (!n.is_empty()).then_some(n)
}

pub fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().replace([',', '_', ' '], "").parse().ok(),
        _ => None,
    }
}

pub fn digit_groups(v: &Value) -> BTreeSet<String> {
    let Some(t) = value_text(v) else {
        return BTreeSet::new();
    };
    t.split(|c: char| !c.is_ascii_digit())
        .filter(|g| !g.is_empty())
        .map(|g| {
            let s = g.trim_start_matches('0');
            if s.is_empty() { "0" } else { s }.to_string()
        })
        .collect()
}

#[derive(Debug, Clone)]
pub enum Feature {
    Text(String),
    Tokens(BTreeSet<String>),
    Number(f64),
}

pub fn feature(v: Option<&Value>, fc: &FieldConfig) -> Option<Feature> {
    let v = v?;
    match fc.compare {
        Compare::None => None,
        Compare::Exact | Compare::String => normalize(v, fc.normalize).map(Feature::Text),
        Compare::Tokens => {
            let t = normalize(v, fc.normalize)?;
            Some(Feature::Tokens(
                fold(&t).split(' ').map(str::to_string).collect(),
            ))
        }
        Compare::Number => number(v).map(Feature::Number),
    }
}

pub fn thresholds(fc: &FieldConfig) -> Vec<f64> {
    if let Some(t) = &fc.thresholds {
        return t.clone();
    }
    match fc.compare {
        Compare::String => vec![0.92, 0.8],
        Compare::Tokens => vec![0.6, 0.3],
        Compare::Number => vec![0.05, 0.25],
        Compare::Exact | Compare::None => vec![],
    }
}

pub fn level_count(fc: &FieldConfig) -> usize {
    match fc.compare {
        Compare::Exact => 2,
        _ => thresholds(fc).len() + 2,
    }
}

pub fn level(a: &Feature, b: &Feature, fc: &FieldConfig) -> u8 {
    let th = thresholds(fc);
    let above = |s: f64| {
        th.iter()
            .position(|t| s >= *t)
            .map_or(th.len() + 1, |i| i + 1) as u8
    };
    let below = |d: f64| {
        th.iter()
            .position(|t| d <= *t)
            .map_or(th.len() + 1, |i| i + 1) as u8
    };
    match (a, b, fc.compare) {
        (Feature::Text(x), Feature::Text(y), Compare::Exact) => u8::from(x != y),
        (Feature::Text(x), Feature::Text(y), _) => {
            if x == y {
                0
            } else {
                above(strsim::jaro_winkler(x, y))
            }
        }
        (Feature::Tokens(x), Feature::Tokens(y), _) => {
            if x == y {
                0
            } else {
                let inter = x.intersection(y).count() as f64;
                let union = x.union(y).count() as f64;
                above(inter / union)
            }
        }
        (Feature::Number(x), Feature::Number(y), _) => {
            if x == y {
                0
            } else {
                below((x - y).abs() / x.abs().max(y.abs()).max(1e-9))
            }
        }
        _ => level_count(fc) as u8 - 1,
    }
}
