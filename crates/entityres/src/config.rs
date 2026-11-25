use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    #[serde(default)]
    pub config: Config,
    pub records: Vec<Record>,
    #[serde(default)]
    pub answers: Vec<Answer>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: String,
    pub source: String,
    #[serde(default)]
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    pub id: String,
    pub answers: Value,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub entity: Option<String>,
    pub fields: BTreeMap<String, FieldConfig>,
    pub keys: Vec<String>,
    pub blocking: Vec<BlockRule>,
    pub vetoes: Vec<Veto>,
    pub sources: Vec<String>,
    pub merge: MergeConfig,
    pub routing: Routing,
}

impl Config {
    pub fn entity(&self) -> &str {
        self.entity.as_deref().unwrap_or("record")
    }

    pub fn normalize_of(&self, field: &str) -> Normalize {
        self.fields
            .get(field)
            .map(|f| f.normalize)
            .unwrap_or_default()
    }

    pub fn merge_of(&self, field: &str) -> MergeRule {
        self.fields
            .get(field)
            .and_then(|f| f.merge)
            .unwrap_or(self.merge.default)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FieldConfig {
    pub compare: Compare,
    pub normalize: Normalize,
    pub semantic: bool,
    pub thresholds: Option<Vec<f64>>,
    pub merge: Option<MergeRule>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compare {
    Exact,
    #[default]
    String,
    Tokens,
    Number,
    None,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Normalize {
    #[default]
    Text,
    Exact,
    Digits,
    Email,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlockRule {
    pub field: String,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default = "default_prefix")]
    pub n: usize,
}

fn default_prefix() -> usize {
    3
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Transform {
    #[default]
    Exact,
    Prefix,
    Tokens,
    Digits,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Veto {
    pub rule: VetoRule,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VetoRule {
    Different,
    NumbersDiffer,
    SameSource,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MergeConfig {
    pub default: MergeRule,
    pub updated_field: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeRule {
    #[default]
    SourcePriority,
    MostCommon,
    Longest,
    Newest,
    Union,
    Judge,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Routing {
    pub accept: f64,
    pub reject: f64,
    pub budget: usize,
    pub min_fields: usize,
    pub conflict: f64,
    pub trust_labels: usize,
    pub trust_agreement: f64,
    pub max_block: usize,
    pub em_iterations: usize,
    pub u_sample: usize,
}

impl Default for Routing {
    fn default() -> Self {
        Routing {
            accept: 0.98,
            reject: 0.02,
            budget: 50,
            min_fields: 2,
            conflict: 3.0,
            trust_labels: 20,
            trust_agreement: 0.99,
            max_block: 500,
            em_iterations: 30,
            u_sample: 200_000,
        }
    }
}
