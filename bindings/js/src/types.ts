export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

export type Compare = "exact" | "string" | "tokens" | "number" | "none";
export type Normalize = "text" | "exact" | "digits" | "email";
export type Transform = "exact" | "prefix" | "tokens" | "digits";
export type MergeRule = "source_priority" | "most_common" | "longest" | "newest" | "union" | "judge";

export interface FieldConfig {
  compare?: Compare;
  normalize?: Normalize;
  semantic?: boolean;
  thresholds?: number[];
  merge?: MergeRule;
}

export interface BlockRule {
  field: string;
  transform?: Transform;
  n?: number;
}

export type Veto =
  | { rule: "same_source" }
  | { rule: "different" | "numbers_differ"; field: string };

export interface Routing {
  accept?: number;
  reject?: number;
  budget?: number;
  min_fields?: number;
  conflict?: number;
  trust_labels?: number;
  trust_agreement?: number;
  max_block?: number;
  em_iterations?: number;
  u_sample?: number;
}

export interface Config {
  entity?: string;
  fields?: Record<string, FieldConfig>;
  keys?: string[];
  blocking?: BlockRule[];
  vetoes?: Veto[];
  sources?: string[];
  merge?: { default?: MergeRule; updated_field?: string };
  routing?: Routing;
}

export interface InputRecord {
  id: string;
  source: string;
  fields?: Record<string, Json>;
}

export interface Answer {
  id: string;
  answers: Json;
}

export interface Input {
  config?: Config;
  records: InputRecord[];
  answers?: Answer[];
}

export type Flag = "low_evidence" | "conflict" | "semantic_mismatch" | "cluster_conflict";

export interface Pair {
  id: string;
  a: string;
  b: string;
  p: number;
  judge?: number;
  decision: "match" | "no_match";
  by: "key" | "veto" | "judge" | "fs";
  unsure: boolean;
  flags: Flag[];
  weights: Record<string, number>;
}

export interface Entity {
  id: string;
  records: string[];
  fields: Record<string, Json>;
  from: Record<string, string[]>;
}

export interface Question {
  id: string;
  kind: "pair" | "value";
  priority: number;
  state: Json;
  questions: Record<string, Json>;
}

export interface Output {
  entities: Entity[];
  pairs: Pair[];
  pending: Question[];
  conflicts: { pair: string; blocked_by: string }[];
  stats: { records: number; candidates: number; judged: number; unsure: number; trusted: string[] };
  model: { lambda: number; fields: Record<string, { m: number[]; u: number[] }> };
}
