use std::collections::HashMap;

fn parse_pipe_fields_with_prefix(line: &str, prefix: &str) -> Option<HashMap<String, String>> {
    let trimmed = line.trim();
    if !trimmed
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
    {
        return None;
    }
    let mut out = HashMap::new();
    for segment in trimmed.split('|').skip(1) {
        if let Some((key, value)) = segment.split_once('=') {
            let key_norm = key.trim().to_ascii_lowercase();
            if key_norm.is_empty() {
                continue;
            }
            out.insert(key_norm, value.trim().to_string());
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

pub(crate) fn parse_evidence_fields(line: &str) -> Option<HashMap<String, String>> {
    parse_pipe_fields_with_prefix(line, "evidence|")
}

fn parse_run_scope_fields(line: &str) -> Option<HashMap<String, String>> {
    parse_pipe_fields_with_prefix(line, "run_scope|")
}

pub(crate) fn run_scoped_evidence_required() -> bool {
    std::env::var("STEER_REQUIRE_RUN_SCOPED_EVIDENCE")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true)
}

pub(crate) fn current_run_scope_id(logs: &[String]) -> Option<String> {
    logs.iter().rev().find_map(|line| {
        let fields = parse_run_scope_fields(line)?;
        fields
            .get("run_id")
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    })
}

pub(crate) fn evidence_fields_match_run_scope(
    fields: &HashMap<String, String>,
    run_scope_id: Option<&str>,
) -> bool {
    if !run_scoped_evidence_required() {
        return true;
    }
    let Some(expected_run_id) = run_scope_id else {
        return true;
    };
    fields
        .get("run_id")
        .map(|actual| actual.trim() == expected_run_id)
        .unwrap_or(false)
}

pub(crate) fn stamp_run_scope_evidence(logs: &mut Vec<String>, run_id: &str, plan_id: &str) {
    if logs.is_empty() {
        return;
    }
    if !logs.iter().any(|line| {
        line.to_ascii_lowercase()
            .contains(&format!("run_scope|run_id={}", run_id).to_ascii_lowercase())
    }) {
        logs.insert(
            0,
            format!("RUN_SCOPE|run_id={}|plan_id={}", run_id, plan_id),
        );
    }

    for line in logs.iter_mut() {
        let lower = line.to_ascii_lowercase();
        let is_evidence_line =
            lower.starts_with("evidence|") || lower.starts_with("mail_send_proof|");
        if !is_evidence_line {
            continue;
        }
        if !lower.contains("|run_id=") {
            line.push_str(&format!("|run_id={}", run_id));
        }
        if !lower.contains("|plan_id=") {
            line.push_str(&format!("|plan_id={}", plan_id));
        }
    }
}

pub(crate) fn logs_have_evidence_fields(logs: &[String], expected: &[(&str, &str)]) -> bool {
    let run_scope_id = current_run_scope_id(logs);
    logs.iter().any(|line| {
        let Some(fields) = parse_evidence_fields(line) else {
            return false;
        };
        if !evidence_fields_match_run_scope(&fields, run_scope_id.as_deref()) {
            return false;
        }
        expected.iter().all(|(key, value)| {
            fields
                .get(&key.to_ascii_lowercase())
                .map(|actual| actual.eq_ignore_ascii_case(value))
                .unwrap_or(false)
        })
    })
}

pub(crate) fn latest_evidence_fields(
    logs: &[String],
    target: &str,
    event: &str,
) -> Option<HashMap<String, String>> {
    let run_scope_id = current_run_scope_id(logs);
    logs.iter().rev().find_map(|line| {
        let fields = parse_evidence_fields(line)?;
        if !evidence_fields_match_run_scope(&fields, run_scope_id.as_deref()) {
            return None;
        }
        let target_ok = fields
            .get("target")
            .map(|v| v.eq_ignore_ascii_case(target))
            .unwrap_or(false);
        let event_ok = fields
            .get("event")
            .map(|v| v.eq_ignore_ascii_case(event))
            .unwrap_or(false);
        if target_ok && event_ok {
            Some(fields)
        } else {
            None
        }
    })
}

pub(crate) fn latest_evidence_field(
    logs: &[String],
    target: &str,
    event: &str,
    field: &str,
) -> Option<String> {
    latest_evidence_fields(logs, target, event).and_then(|fields| {
        fields
            .get(&field.to_ascii_lowercase())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

pub(crate) fn latest_legacy_mail_send_field(logs: &[String], field: &str) -> Option<String> {
    let run_scope_id = current_run_scope_id(logs);
    logs.iter().rev().find_map(|line| {
        let fields = parse_pipe_fields_with_prefix(line, "mail_send_proof|")?;
        if !evidence_fields_match_run_scope(&fields, run_scope_id.as_deref()) {
            return None;
        }
        fields
            .get(&field.to_ascii_lowercase())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

pub(crate) fn latest_evidence_int(
    logs: &[String],
    target: &str,
    event: &str,
    field: &str,
) -> Option<i64> {
    latest_evidence_field(logs, target, event, field).and_then(|v| v.parse::<i64>().ok())
}
