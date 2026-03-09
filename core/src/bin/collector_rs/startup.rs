use super::support;
use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;

#[derive(Default)]
struct PatternAccumulator {
    count: u64,
    examples: Vec<Value>,
}

pub(super) fn run_startup_workflow_generation(
    db_path: &PathBuf,
    output_dir: &PathBuf,
    min_events: usize,
    pattern_threshold: u64,
) -> anyhow::Result<Option<PathBuf>> {
    fs::create_dir_all(output_dir)?;

    let yesterday = (Utc::now() - ChronoDuration::days(1))
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    let output_path = output_dir.join(format!("workflow_{yesterday}.json"));
    if output_path.exists() {
        return Ok(None);
    }

    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT ts, app, payload_json
         FROM events_v2
         WHERE date(ts) <= ?1
         ORDER BY ts ASC",
    )?;

    let rows = stmt.query_map(params![yesterday], |row| {
        let ts: String = row.get(0)?;
        let app: String = row.get(1)?;
        let payload_json: String = row.get(2)?;
        Ok((ts, app, payload_json))
    })?;

    let mut total_events = 0usize;
    let mut patterns: HashMap<(String, String, String), PatternAccumulator> = HashMap::new();

    for row in rows {
        let (ts, app, payload_json) = row?;
        total_events += 1;

        let payload: Value = serde_json::from_str(&payload_json).unwrap_or(Value::Null);
        let Some(obj) = payload.as_object() else {
            continue;
        };

        let control_type = obj
            .get("control_type")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let element_name = obj
            .get("element_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        if control_type.is_empty() || element_name.is_empty() {
            continue;
        }

        let app_short = support::normalize_app_for_pattern(&app);
        let key = (
            app_short,
            control_type.to_string(),
            element_name.to_string(),
        );
        let entry = patterns.entry(key).or_default();
        entry.count += 1;

        if entry.examples.len() < 3 {
            entry.examples.push(json!({
                "ts": ts,
                "window_title": obj
                    .get("window_title")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default(),
                "automation_id": obj
                    .get("automation_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
            }));
        }
    }

    if total_events < min_events {
        return Ok(None);
    }

    let mut ranked: Vec<(String, String, String, u64, Vec<Value>)> = patterns
        .into_iter()
        .filter_map(|((app, control_type, element_name), acc)| {
            if acc.count < pattern_threshold {
                None
            } else {
                Some((app, control_type, element_name, acc.count, acc.examples))
            }
        })
        .collect();

    ranked.sort_by(|a, b| b.3.cmp(&a.3));
    ranked.truncate(20);

    if ranked.is_empty() {
        return Ok(None);
    }

    let mut top_apps = BTreeSet::new();
    let mut steps = Vec::with_capacity(ranked.len());

    for (idx, (app, control_type, element_name, frequency, examples)) in ranked.iter().enumerate() {
        if top_apps.len() < 5 {
            top_apps.insert(app.clone());
        }

        let first_example = examples.first().cloned().unwrap_or_else(|| json!({}));
        let window_title = first_example
            .get("window_title")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let automation_id = first_example
            .get("automation_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        steps.push(json!({
            "step_number": idx + 1,
            "action_type": support::infer_action_type(control_type),
            "target": {
                "app": app,
                "control_type": control_type,
                "element_name": element_name,
                "window_title": window_title,
                "automation_id": automation_id
            },
            "frequency": frequency,
            "description": format!("{} {} '{}' ({} repeats)", app, control_type, element_name, frequency)
        }));
    }

    let events_analyzed: u64 = ranked.iter().map(|(_, _, _, count, _)| *count).sum();

    let workflow = json!({
        "workflow_name": format!("Daily Patterns - {yesterday}"),
        "description": format!("Behavior pattern analysis through {yesterday}"),
        "created_at": Utc::now().to_rfc3339(),
        "analysis_period": {
            "until": yesterday,
            "events_analyzed": events_analyzed,
            "events_scanned": total_events
        },
        "patterns": steps,
        "metadata": {
            "total_patterns": ranked.len(),
            "top_apps": top_apps.into_iter().collect::<Vec<_>>(),
            "generated_by": "collector_rs_startup_generator"
        }
    });

    fs::write(&output_path, serde_json::to_string_pretty(&workflow)?)?;
    println!("[Workflow] Generated: {}", output_path.display());

    Ok(Some(output_path))
}
