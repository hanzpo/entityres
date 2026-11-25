use entityres::{run, Answer, Input, Kind, Output, Question};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn fixtures() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut dirs: Vec<PathBuf> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("input.json").exists())
        .collect();
    dirs.sort();
    dirs
}

fn load(p: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

fn fake_judge(judge: &Value, q: &Question) -> Value {
    match q.kind {
        Kind::Pair => {
            let key = format!(
                "{}|{}",
                q.state["a"]["id"].as_str().unwrap(),
                q.state["b"]["id"].as_str().unwrap()
            );
            let p = judge["pairs"][&key].as_f64().unwrap_or(0.0);
            json!({ "same": { "type": "noul", "noul": p } })
        }
        Kind::Value => {
            let want = &judge["values"][q.state["id"].as_str().unwrap()]
                [q.state["field"].as_str().unwrap()];
            let criteria = q.questions["pick"]["criteria"].as_object().unwrap();
            let choice = criteria
                .iter()
                .find(|(_, v)| *v == want)
                .map_or("0".to_string(), |(k, _)| k.clone());
            json!({ "pick": { "type": "choice", "choice": choice } })
        }
    }
}

fn resolve(input: &mut Input, judge: &Value) -> (Output, usize) {
    let mut calls = 0;
    for _ in 0..10 {
        let out = run(input).unwrap();
        if out.pending.is_empty() {
            return (out, calls);
        }
        for q in &out.pending {
            calls += 1;
            input.answers.push(Answer {
                id: q.id.clone(),
                answers: fake_judge(judge, q),
            });
        }
    }
    panic!("did not settle in 10 rounds");
}

#[test]
fn shared_fixtures() {
    for dir in fixtures() {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let mut input: Input = serde_json::from_value(load(&dir.join("input.json"))).unwrap();
        let judge = load(&dir.join("judge.json"));
        let expected = load(&dir.join("expected.json"));
        let (out, calls) = resolve(&mut input, &judge);
        let got: BTreeSet<Vec<String>> = out.entities.iter().map(|e| e.records.clone()).collect();
        let want: BTreeSet<Vec<String>> = expected["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| serde_json::from_value(e["records"].clone()).unwrap())
            .collect();
        if std::env::var("ENTITYRES_DEBUG").is_ok() {
            eprintln!(
                "{name}: {calls} judge calls, answers {:?}",
                input.answers.iter().map(|a| &a.id).collect::<Vec<_>>()
            );
            for p in &out.pairs {
                eprintln!(
                    "  {} p={:.4} {:?} {:?} unsure={} {:?}",
                    p.id, p.p, p.decision, p.by, p.unsure, p.flags
                );
            }
        }
        assert_eq!(got, want, "{name}: clusters");
        for e in expected["entities"].as_array().unwrap() {
            let Some(fields) = e["fields"].as_object() else {
                continue;
            };
            let records: Vec<String> = serde_json::from_value(e["records"].clone()).unwrap();
            let entity = out.entities.iter().find(|x| x.records == records).unwrap();
            for (k, v) in fields {
                assert_eq!(
                    entity.fields.get(k),
                    Some(v),
                    "{name}: {} field {k}",
                    entity.id
                );
            }
        }
        if let Some(max) = expected["max_calls"].as_u64() {
            assert!(
                calls as u64 <= max,
                "{name}: {calls} judge calls, max {max}"
            );
        }
        assert!(
            out.conflicts.is_empty(),
            "{name}: conflicts {:?}",
            out.conflicts
        );
    }
}

#[test]
fn bad_input_returns_error_json() {
    let out: Value = serde_json::from_str(&entityres::run_json("{}")).unwrap();
    assert!(out["error"].as_str().unwrap().contains("records"));
    let dup = json!({ "records": [{ "id": "1", "source": "a" }, { "id": "1", "source": "a" }] });
    let out: Value = serde_json::from_str(&entityres::run_json(&dup.to_string())).unwrap();
    assert_eq!(out["error"], "duplicate record a:1");
}
