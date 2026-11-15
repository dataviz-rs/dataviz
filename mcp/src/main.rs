//! `dataviz-mcp`: a Model Context Protocol server (stdio, JSON-RPC 2.0, one
//! message per line) that draws charts with dataviz and returns them as
//! images, so an AI assistant can make a chart, look at it and refine it.
//!
//! Tools: `plot` (a figure from a JSON spec), `plot_file` (a chart of a
//! CSV/TSV/JSON file), `molecule` (a PDB structure in 3D) and
//! `chart_types` (what `plot` can draw).

mod data;
mod spec;
mod tools;

use serde_json::{json, Value};
use std::io::{BufRead, Write};

/// Protocol versions this server speaks (newest first).
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("dataviz-mcp {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&msg),
            Err(e) => Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(r) = reply {
            // One message per line (no embedded newlines in compact JSON).
            let _ = writeln!(out, "{r}");
            let _ = out.flush();
        }
    }
}

/// The reply to one message (`None` for notifications).
fn handle(msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let id = id?; // notifications (initialized, cancelled, …) get no reply
    let result = match method {
        "initialize" => {
            let asked = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(VERSIONS[0]);
            let version = VERSIONS
                .iter()
                .find(|v| **v == asked)
                .copied()
                .unwrap_or(VERSIONS[0]);
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "dataviz", "title": "dataviz charts", "version": env!("CARGO_PKG_VERSION") },
                "instructions": tools::INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools::list() }),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            tools::call(name, &args)
        }
        _ => return Some(error(id, -32601, &format!("method not found: {method}"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
