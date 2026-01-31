import sqlite3
import json
import datetime

conn = sqlite3.connect('steer.db')
cursor = conn.cursor()

with open('daily_report_workflow.json', 'r') as f:
    json_content = f.read()

cursor.execute('''
    INSERT INTO recommendations (
        created_at, status, title, summary, trigger, actions, n8n_prompt, fingerprint, confidence, workflow_json
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
''', (
    datetime.datetime.now().isoformat(),
    "pending",
    "📊 Daily Antigravity Usage Report",
    "매일 밤 11시에 Antigravity 등 앱 사용 통계를 텔레그램으로 보냅니다.",
    "Every Day at 11:00 PM",
    '["Query DB", "Summarize", "Telegram"]',
    "Report daily app usage from SQLite db",
    "report_daily_usage_v3",
    1.0,
    json_content
))

conn.commit()
conn.close()
print("Success")
