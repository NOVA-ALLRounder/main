use rusqlite::{params, Connection, Result};
use std::fs;

fn main() -> Result<()> {
    let conn = Connection::open("steer.db")?;
    let json_content = fs::read_to_string("daily_report_workflow.json").unwrap();
    
    conn.execute(
        "INSERT INTO recommendations (
            created_at, status, title, summary, trigger, actions, n8n_prompt, fingerprint, confidence, workflow_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            chrono::Utc::now().to_rfc3339(),
            "pending",
            "📊 Daily Antigravity Usage Report",
            "매일 밤 11시에 'Antigravity' 및 기타 앱 사용량을 분석하여 텔레그램으로 보냅니다.",
            "Every Day at 11:00 PM",
            "[\"Query DB\", \"Summarize\", \"Telegram\"]",
            "Report daily app usage from SQLite db",
            "report_daily_usage_v1",
            1.0,
            json_content
        ],
    )?;
    println!("Inserted recommendation.");
    Ok(())
}
