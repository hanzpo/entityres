mod blocking;
mod cluster;
mod compare;
mod config;
mod fs;
mod merge;
mod resolve;

pub use config::*;
pub use resolve::*;

pub fn run_json(input: &str) -> String {
    let result = serde_json::from_str::<Input>(input)
        .map_err(|e| Error(format!("invalid input: {e}")))
        .and_then(|i| run(&i));
    match result {
        Ok(out) => serde_json::to_string(&out).unwrap_or_else(|e| error_json(&e.to_string())),
        Err(e) => error_json(&e.0),
    }
}

fn error_json(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}
