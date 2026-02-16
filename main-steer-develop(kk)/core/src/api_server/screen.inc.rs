pub(super) fn is_screen_summary_request(lower: &str) -> bool {
    let screen = lower.contains("screen")
        || lower.contains("current screen")
        || lower.contains("window")
        || lower.contains("\u{D654}\u{BA74}")
        || lower.contains("\u{CC3D}");
    let summarize = lower.contains("summary")
        || lower.contains("summarize")
        || lower.contains("\u{C694}\u{C57D}")
        || lower.contains("\u{C815}\u{B9AC}");
    screen && summarize
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScreenSummaryTarget {
    Notepad,
    Clipboard,
    File,
    Notion,
    Word,
}

fn parse_screen_summary_target(lower: &str) -> ScreenSummaryTarget {
    if lower.contains("clipboard") || lower.contains("\u{D074}\u{B9BD}\u{BCF4}\u{B4DC}") {
        return ScreenSummaryTarget::Clipboard;
    }
    if lower.contains("file")
        || lower.contains("save")
        || lower.contains("\u{D30C}\u{C77C}")
        || lower.contains("\u{C800}\u{C7A5}")
    {
        return ScreenSummaryTarget::File;
    }
    if lower.contains("notion") || lower.contains("\u{B178}\u{C158}") {
        return ScreenSummaryTarget::Notion;
    }
    if lower.contains("word") || lower.contains("docx") || lower.contains("\u{C6CC}\u{B4DC}") {
        return ScreenSummaryTarget::Word;
    }
    ScreenSummaryTarget::Notepad
}

#[derive(Debug, Clone)]
struct ActiveWindowTextSnapshot {
    app: String,
    title: String,
    text: String,
    line_count: usize,
}

#[cfg(target_os = "windows")]
fn read_active_window_text_snapshot() -> Option<ActiveWindowTextSnapshot> {
    let script = r#"
$ErrorActionPreference='SilentlyContinue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class Win32 {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@;
$h = [Win32]::GetForegroundWindow()
if ($h -eq [IntPtr]::Zero) { return }
$root = [System.Windows.Automation.AutomationElement]::FromHandle($h)
if ($null -eq $root) { return }

$pid = 0
[void][Win32]::GetWindowThreadProcessId($h, [ref]$pid)
$app = ''
try { $app = (Get-Process -Id $pid -ErrorAction Stop).ProcessName } catch {}
$title = ''
try { $title = $root.Current.Name } catch {}

$lines = New-Object System.Collections.Generic.List[string]
function Add-Line([string]$s) {
  if ([string]::IsNullOrWhiteSpace($s)) { return }
  $t = $s.Trim()
  if ($t.Length -gt 220) { $t = $t.Substring(0,220) }
  if ($t -match '^(?:\W|_)+$') { return }
  if ($lines.Count -ge 260) { return }
  if (-not $lines.Contains($t)) { [void]$lines.Add($t) }
}

Add-Line $title
try {
  $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
  $stack = New-Object System.Collections.Stack
  $first = $walker.GetFirstChild($root)
  if ($first -ne $null) { $stack.Push($first) }

  while ($stack.Count -gt 0 -and $lines.Count -lt 260) {
    $node = $stack.Pop()
    if ($null -eq $node) { continue }
    try {
      if (-not $node.Current.IsOffscreen) {
        Add-Line $node.Current.Name
        $vpObj = $null
        if ($node.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$vpObj)) {
          if ($vpObj -ne $null) {
            Add-Line (($vpObj -as [System.Windows.Automation.ValuePattern]).Current.Value)
          }
        }
      }

      $sib = $walker.GetNextSibling($node)
      if ($sib -ne $null) { $stack.Push($sib) }
      $child = $walker.GetFirstChild($node)
      if ($child -ne $null) { $stack.Push($child) }
    } catch {}
  }
} catch {}

$result = [pscustomobject]@{
  app = $app
  title = $title
  line_count = $lines.Count
  text = ($lines -join "`n")
}
$result | ConvertTo-Json -Compress -Depth 4
"#;

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let raw_all = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw_all.is_empty() {
        return None;
    }
    let raw_json = raw_all
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with('{') && line.trim_end().ends_with('}'))
        .map(|s| s.trim())
        .unwrap_or(raw_all.trim());

    let parsed: serde_json::Value = serde_json::from_str(raw_json).ok()?;
    let app = parsed
        .get("app")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let title = parsed
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let text = parsed
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let line_count = parsed
        .get("line_count")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or_else(|| text.lines().count());

    if text.is_empty() {
        None
    } else {
        Some(ActiveWindowTextSnapshot {
            app,
            title,
            text,
            line_count,
        })
    }
}

#[cfg(not(target_os = "windows"))]
fn read_active_window_text_snapshot() -> Option<ActiveWindowTextSnapshot> {
    None
}


async fn summarize_window_text_with_llm(
    state: &AppState,
    user_message: &str,
    snapshot: &ActiveWindowTextSnapshot,
) -> Result<String, String> {
    let Some(brain) = &state.llm_client else {
        return Err("LLM client unavailable".to_string());
    };

    let clipped_text: String = snapshot.text.chars().take(14000).collect();
    let prompt = format!(
        "占쏙옙占쏙옙占?占쏙옙청: {user_message}\n\
활占쏙옙 창 占쏙옙占쏙옙:\n\
- 占쏙옙: {app}\n\
- 창 占쏙옙占쏙옙: {title}\n\
- 占쏙옙占쏙옙 占쏙옙占쏙옙 占쏙옙: {line_count}\n\n\
화占쏙옙 占쏙옙占쏙옙트(UI 占쏙옙占쏙옙화 占쏙옙占쏙옙):\n{ui_text}\n\n\
占쏙옙청:\n\
1) 占쏙옙 占쌔쏙옙트占쏙옙 占쏙옙占쏙옙占쏙옙占쏙옙 占쏙옙占쏙옙 화占쏙옙占쏙옙 占싼깍옙占쏙옙占?占쏙옙占?\n\
2) 占쌩울옙占쏙옙 占싣띰옙, TODO/占쏙옙占쏙옙 占썅동, 占쏙옙占쏙옙/占쏙옙占쏙옙占쏙옙 占쎌선.\n\
3) 占쌔쏙옙트占쏙옙 占쌀몌옙확占싹몌옙 '占쏙옙확占쏙옙/확占쏙옙 占십울옙'占쏙옙 占쏙옙占?\n\
4) 占쌘듸옙占쏙옙 占쏙옙占쏙옙 占쏙옙占쏙옙占쏙옙, 占쌍댐옙 10占쏙옙 bullet.\n\
5) 占쏙옙占쏙옙:\n\
占쏙옙占쏙옙 占쏙옙占?\n- ...\n\
占쌔억옙 占쏙옙 占쏙옙:\n- ...\n\
占쏙옙확占쏙옙/확占쏙옙 占십울옙:\n- ...",
        app = if snapshot.app.is_empty() { "unknown" } else { &snapshot.app },
        title = if snapshot.title.is_empty() { "(占쏙옙占쏙옙 占쏙옙占쏙옙)" } else { &snapshot.title },
        line_count = snapshot.line_count,
        ui_text = clipped_text
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are Jarvis-like desktop copilot. Summarize noisy desktop text into precise Korean notes. Never output garbled characters intentionally."
        }),
        json!({
            "role": "user",
            "content": prompt
        }),
    ];

    let out = brain
        .chat_completion(messages)
        .await
        .map_err(|e| format!("UI text summary failed: {}", e))?;
    let cleaned = normalize_screen_summary(&out);
    if !is_summary_usable(&cleaned) {
        return Err("UI text summary quality too low".to_string());
    }
    Ok(cleaned)
}

async fn summarize_current_screen_text(
    state: &AppState,
    user_message: &str,
) -> Result<String, String> {
    let ui_min_quality = std::env::var("SCREEN_SUMMARY_UI_MIN_QUALITY")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.46);
    let ui_min_chars = std::env::var("SCREEN_SUMMARY_UI_MIN_CHARS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(120);

    if let Some(snapshot) = read_active_window_text_snapshot() {
        let text_len = snapshot.text.chars().count();
        let quality = score_text_signal_quality(&snapshot.text);
        let overlay_like = looks_like_steer_overlay_text(&snapshot.text);
        if !overlay_like && text_len >= ui_min_chars && quality >= ui_min_quality {
            if let Ok(summary) = summarize_window_text_with_llm(state, user_message, &snapshot).await {
                return Ok(summary);
            }
        }
    }

    let (b64, _scale) = crate::visual_driver::VisualDriver::capture_screen()
        .map_err(|e| format!("Screen capture failed: {}", e))?;

    let prompt = format!(
        "You are a desktop assistant. Read the current screen and summarize it in Korean.\n\
User request: {}\n\
Output requirements:\n\
- 5~10 bullet points maximum\n\
- Focus on actionable items, deadlines, errors, and next steps\n\
- Plain text only (no markdown code fences)\n\
- If the screen is unclear, state what is unclear and what to open next.",
        user_message
    );

    let provider = std::env::var("SCREEN_SUMMARY_PROVIDER")
        .unwrap_or_else(|_| {
            if std::env::var("GLM_OCR_API_KEY").is_ok() || std::env::var("GLM_API_KEY").is_ok() {
                "glm".to_string()
            } else {
                "openai".to_string()
            }
        })
        .to_lowercase();

    let min_quality = std::env::var("SCREEN_SUMMARY_MIN_OUTPUT_QUALITY")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.56);

    let providers: Vec<&str> = if provider == "glm" || provider == "glm_ocr" {
        vec!["glm", "openai"]
    } else {
        vec!["openai", "glm"]
    };
    let mut errors: Vec<String> = Vec::new();

    for p in providers {
        let raw = if p == "glm" {
            match summarize_current_screen_text_glm(&prompt, &b64).await {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("{} failed: {}", p, e));
                    continue;
                }
            }
        } else {
            let Some(brain) = &state.llm_client else {
                errors.push("openai failed: LLM client unavailable".to_string());
                continue;
            };
            match brain.analyze_screen(&prompt, &b64).await {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("{} failed: {}", p, e));
                    continue;
                }
            }
        };

        let cleaned = normalize_screen_summary(&raw);
        let quality = score_summary_output_quality(&cleaned);
        if !cleaned.is_empty() && is_summary_usable(&cleaned) && quality >= min_quality {
            return Ok(cleaned);
        }
        errors.push(format!(
            "{} low quality (score={:.2}, chars={})",
            p,
            quality,
            cleaned.chars().count()
        ));
    }

    Err(format!(
        "Vision summary quality too low after provider fallback: {}",
        errors.join(" | ")
    ))
}

fn extract_json_text_field(v: &serde_json::Value) -> Option<String> {
    let pointers = [
        "/result/markdown",
        "/result/text",
        "/result/content",
        "/data/markdown",
        "/data/text",
        "/data/content",
        "/output/markdown",
        "/output/text",
        "/output/content",
        "/text",
        "/content",
        "/choices/0/message/content",
    ];
    for p in pointers {
        if let Some(node) = v.pointer(p) {
            if let Some(s) = node.as_str() {
                let t = s.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
            if let Some(arr) = node.as_array() {
                let mut combined = String::new();
                for item in arr {
                    if let Some(s) = item.as_str() {
                        if !s.trim().is_empty() {
                            if !combined.is_empty() {
                                combined.push('\n');
                            }
                            combined.push_str(s.trim());
                        }
                        continue;
                    }
                    if let Some(s) = item.get("text").and_then(|x| x.as_str()) {
                        if !s.trim().is_empty() {
                            if !combined.is_empty() {
                                combined.push('\n');
                            }
                            combined.push_str(s.trim());
                        }
                    }
                }
                if !combined.trim().is_empty() {
                    return Some(combined.trim().to_string());
                }
            }
        }
    }
    None
}

fn collect_layout_text(node: &serde_json::Value, out: &mut Vec<String>) {
    match node {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if let Some(s) = v.as_str() {
                    let key = k.to_lowercase();
                    let t = s.trim();
                    if t.is_empty() {
                        continue;
                    }
                    if t.starts_with("data:image/") || t.len() > 2000 {
                        continue;
                    }
                    let preferred = matches!(
                        key.as_str(),
                        "text"
                            | "content"
                            | "ocr_text"
                            | "recognized_text"
                            | "value"
                            | "markdown"
                            | "caption"
                            | "title"
                            | "line"
                            | "lines"
                            | "word"
                            | "words"
                    );
                    let looks_sentence =
                        t.contains(' ') || t.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c));
                    if preferred || looks_sentence {
                        if !out.iter().any(|x| x == t) {
                            out.push(t.to_string());
                        }
                    }
                } else {
                    collect_layout_text(v, out);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_layout_text(v, out);
            }
        }
        _ => {}
    }
}

fn extract_layout_parsing_text(v: &serde_json::Value) -> Option<String> {
    let candidate_nodes = [
        "/layout_details",
        "/result/layout_details",
        "/data/layout_details",
        "/result",
        "/data",
    ];
    let mut lines = Vec::new();
    for p in candidate_nodes {
        if let Some(node) = v.pointer(p) {
            collect_layout_text(node, &mut lines);
        }
    }
    if lines.is_empty() {
        return None;
    }
    let text = lines
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .take(200)
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

async fn summarize_current_screen_text_glm(prompt: &str, image_b64: &str) -> Result<String, String> {
    let api_key = std::env::var("GLM_OCR_API_KEY")
        .or_else(|_| std::env::var("GLM_API_KEY"))
        .map_err(|_| "GLM_OCR_API_KEY (or GLM_API_KEY) not set".to_string())?;

    let layout_url = std::env::var("GLM_OCR_BASE_URL")
        .unwrap_or_else(|_| "https://open.bigmodel.cn/api/paas/v4/layout_parsing".to_string());
    let layout_model = std::env::var("GLM_OCR_MODEL").unwrap_or_else(|_| "glm-ocr".to_string());
    let data_url = format!("data:image/jpeg;base64,{}", image_b64);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("GLM client init failed: {}", e))?;

    let layout_body = json!({
        "model": layout_model,
        "file": data_url,
        "prompt": prompt
    });

    let layout_resp = client
        .post(&layout_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&layout_body)
        .send()
        .await
        .map_err(|e| format!("GLM OCR request failed: {}", e))?;

    let layout_status = layout_resp.status();
    let layout_text = layout_resp
        .text()
        .await
        .map_err(|e| format!("GLM OCR read failed: {}", e))?;

    if layout_status.is_success() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&layout_text) {
            if let Some(out) = extract_json_text_field(&v) {
                let summarized = summarize_ocr_text_with_glm(
                    &client,
                    &api_key,
                    prompt,
                    &out,
                )
                .await;
                return Ok(summarized.unwrap_or(out));
            }
            if let Some(out) = extract_layout_parsing_text(&v) {
                let summarized = summarize_ocr_text_with_glm(
                    &client,
                    &api_key,
                    prompt,
                    &out,
                )
                .await;
                return Ok(summarized.unwrap_or(out));
            }
        }
    }

    let chat_url = std::env::var("GLM_VISION_CHAT_URL")
        .unwrap_or_else(|_| "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string());
    let chat_model = std::env::var("GLM_VISION_MODEL")
        .ok()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| "glm-4.5".to_string());
    let chat_body = json!({
        "model": chat_model,
        "messages": [
            {
                "role": "user",
                "content": format!(
                    "{}\n\nOCR raw response (truncated):\n{}",
                    prompt,
                    layout_text.chars().take(12000).collect::<String>()
                )
            }
        ],
        "max_tokens": 500
    });
    let chat_resp = client
        .post(&chat_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&chat_body)
        .send()
        .await
        .map_err(|e| format!("GLM vision fallback request failed: {}", e))?;
    let chat_status = chat_resp.status();
    let chat_text = chat_resp
        .text()
        .await
        .map_err(|e| format!("GLM vision fallback read failed: {}", e))?;

    if chat_status.is_success() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&chat_text) {
            if let Some(out) = extract_json_text_field(&v) {
                return Ok(out);
            }
        }
    }

    Err(format!(
        "GLM OCR failed (status {}): {} | GLM vision fallback failed (status {}): {}",
        layout_status.as_u16(),
        layout_text.chars().take(240).collect::<String>(),
        chat_status.as_u16(),
        chat_text.chars().take(240).collect::<String>()
    ))
}

async fn summarize_ocr_text_with_glm(
    client: &reqwest::Client,
    api_key: &str,
    user_prompt: &str,
    ocr_text: &str,
) -> Option<String> {
    let chat_url = std::env::var("GLM_VISION_CHAT_URL")
        .unwrap_or_else(|_| "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string());
    let chat_model = std::env::var("GLM_VISION_MODEL")
        .ok()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| "glm-4.5".to_string());

    let body = json!({
        "model": chat_model,
        "messages": [
            {
                "role": "system",
                "content": "You summarize noisy OCR from desktop screenshots into concise Korean bullets. If text is corrupted, explicitly say some text was unclear and avoid gibberish."
            },
            {
                "role": "user",
                "content": format!(
                    "Original user request:\n{}\n\nOCR text:\n{}\n\nReturn only 5~10 Korean bullet points with actionable items, key context, and uncertainty notes.",
                    user_prompt,
                    ocr_text.chars().take(12000).collect::<String>()
                )
            }
        ],
        "max_tokens": 500
    });

    let resp = match client
        .post(&chat_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return None,
    };

    if !resp.status().is_success() {
        return None;
    }

    let text = match resp.text().await {
        Ok(t) => t,
        Err(_) => return None,
    };

    let parsed = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let out = extract_json_text_field(&parsed)?;
    let cleaned = normalize_screen_summary(&out);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

pub(super) async fn deliver_screen_summary(
    state: &AppState,
    user_message: &str,
) -> Result<(String, String), String> {
    let summary = summarize_current_screen_text(state, user_message).await?;
    let lower = user_message.to_lowercase();
    let target = parse_screen_summary_target(&lower);

    let final_text = format!(
        "[Screen Summary - {}]\n\n{}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        summary
    );

    match target {
        ScreenSummaryTarget::Clipboard => {
            crate::tool_chaining::CrossAppBridge::copy_to_clipboard(&final_text)
                .map_err(|e| format!("Failed to copy summary to clipboard: {}", e))?;
            Ok(("clipboard".to_string(), final_text))
        }
        ScreenSummaryTarget::File => {
            let base = std::env::var("STEER_HOME")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
            let out_dir = base.join("artifacts").join("screen-summaries");
            std::fs::create_dir_all(&out_dir)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
            let out_path = out_dir.join(format!(
                "screen-summary-{}.txt",
                chrono::Local::now().format("%Y%m%d-%H%M%S")
            ));
            std::fs::write(&out_path, &final_text)
                .map_err(|e| format!("Failed to write summary file: {}", e))?;
            let _ = open_path_windows(&out_path);
            Ok((format!("file:{}", out_path.display()), final_text))
        }
        ScreenSummaryTarget::Notion => {
            let db = std::env::var("NOTION_DATABASE_ID")
                .map_err(|_| "NOTION_DATABASE_ID missing".to_string())?;
            if !is_valid_notion_database_id(&db) {
                return Err("Invalid NOTION_DATABASE_ID format".to_string());
            }
            let client = integrations::notion::NotionClient::from_env()
                .map_err(|e| format!("Notion client init failed: {}", e))?;
            let title = format!("Screen Summary {}", chrono::Local::now().format("%Y-%m-%d %H:%M"));
            let body: String = final_text.chars().take(1800).collect();
            let page_id = client
                .create_page(&db, &title, &body)
                .await
                .map_err(|e| format!("Notion create failed: {}", e))?;
            Ok((format!("notion:{}", page_id), final_text))
        }
        ScreenSummaryTarget::Word => {
            crate::tool_chaining::CrossAppBridge::copy_to_clipboard(&final_text)
                .map_err(|e| format!("Failed to stage clipboard for Word: {}", e))?;
            crate::windows::actions::launch_app("winword")
                .map_err(|e| format!("Failed to launch Word: {}", e))?;
            tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;
            crate::windows::actions::type_text(&final_text)
                .map_err(|e| format!("Failed to type into Word: {}", e))?;
            Ok(("word".to_string(), final_text))
        }
        ScreenSummaryTarget::Notepad => {
            crate::windows::actions::launch_app("notepad")
                .map_err(|e| format!("Failed to launch Notepad: {}", e))?;
            tokio::time::sleep(tokio::time::Duration::from_millis(700)).await;
            crate::windows::actions::type_text(&final_text)
                .map_err(|e| format!("Failed to type into Notepad: {}", e))?;
            Ok(("notepad".to_string(), final_text))
        }
    }
}

fn free_chat_system_prompt() -> &'static str {
    "You are Steer, a practical desktop copilot.
- Default to natural conversation in the user's language.
- Give concise, useful answers.
- If a request implies automation, explain what you can do now and what command or phrase to run next.
- If blocked by runtime mode or missing credentials, explain the exact missing piece briefly.
- Do not claim actions were executed unless explicit execution result was provided."
}

pub(super) async fn generate_free_chat_reply(state: &AppState, message: &str) -> Result<String, String> {
    let Some(brain) = &state.llm_client else {
        return Err("LLM client unavailable".to_string());
    };

    let history_limit = context_pruning::history_fetch_limit().min(16);
    let history = db::get_recent_chat_history(history_limit).unwrap_or_default();

    let mut messages: Vec<serde_json::Value> = Vec::with_capacity(history.len() + 2);
    messages.push(json!({
        "role": "system",
        "content": free_chat_system_prompt()
    }));

    for h in history {
        if h.content.trim().is_empty() {
            continue;
        }
        let role = match h.role.as_str() {
            "user" => "user",
            "assistant" => "assistant",
            _ => continue,
        };
        messages.push(json!({
            "role": role,
            "content": h.content
        }));
    }

    messages.push(json!({
        "role": "user",
        "content": message
    }));

    match brain.chat_completion(messages).await {
        Ok(s) => Ok(s.trim().to_string()),
        Err(e) => {
            let err_text = e.to_string();
            let lower = err_text.to_lowercase();
            if lower.contains("rate limit") || lower.contains("rate_limit_exceeded") {
                // Retry once with minimal context to reduce token pressure.
                let minimal = vec![
                    json!({
                        "role": "system",
                        "content": "You are a concise assistant. Reply in the user's language in 2-4 short sentences."
                    }),
                    json!({
                        "role": "user",
                        "content": message
                    }),
                ];
                return brain
                    .chat_completion(minimal)
                    .await
                    .map(|s| s.trim().to_string())
                    .map_err(|e2| e2.to_string());
            }
            Err(err_text)
        }
    }
}



