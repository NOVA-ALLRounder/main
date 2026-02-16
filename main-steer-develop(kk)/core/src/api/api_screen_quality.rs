pub fn score_text_signal_quality(text: &str) -> f32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0.0;
    }

    let total_chars = trimmed.chars().count().max(1);
    let mut readable_chars = 0usize;
    let mut weird_chars = 0usize;
    let mut meaningful_lines = 0usize;
    let mut total_lines = 0usize;

    for ch in trimmed.chars() {
        let is_hangul = ('\u{AC00}'..='\u{D7A3}').contains(&ch);
        if ch.is_ascii_alphanumeric()
            || is_hangul
            || ch.is_whitespace()
            || ",.;:!?()[]{}+-_/@#'\"%&".contains(ch)
        {
            readable_chars += 1;
        }
        if ch == '\u{FFFD}' || (ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t') {
            weird_chars += 1;
        }
    }

    for line in trimmed.lines() {
        total_lines += 1;
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let alpha = t
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || ('\u{AC00}'..='\u{D7A3}').contains(c))
            .count();
        if alpha >= 2 {
            meaningful_lines += 1;
        }
    }

    let readability = readable_chars as f32 / total_chars as f32;
    let weird_ratio = weird_chars as f32 / total_chars as f32;
    let density = (trimmed.chars().count().min(2000) as f32 / 2000.0).min(1.0);
    let line_score = (meaningful_lines.min(40) as f32 / 40.0).min(1.0);

    let mut score = 0.45 * readability + 0.30 * line_score + 0.25 * density - 0.55 * weird_ratio;
    if total_lines <= 2 {
        score -= 0.15;
    }
    score.clamp(0.0, 1.0)
}

pub fn is_summary_usable(summary: &str) -> bool {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lines = trimmed.lines().filter(|l| !l.trim().is_empty()).count();
    if lines < 2 {
        return false;
    }
    score_text_signal_quality(trimmed) >= 0.35
}

pub fn looks_like_steer_overlay_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    let markers = [
        "control center",
        "refresh preflight",
        "mode:observe",
        "mode:copilot",
        "mode:autopilot",
        "automation:enabled",
        "status:running",
        "rust_webview_eval_ok",
        "suggestions",
    ];
    let hits = markers.iter().filter(|m| lower.contains(**m)).count();
    hits >= 2
}

fn line_has_cyrillic(text: &str) -> bool {
    text.chars()
        .any(|c| ('\u{0400}'..='\u{04FF}').contains(&c) || ('\u{0500}'..='\u{052F}').contains(&c))
}

fn line_has_hangul(text: &str) -> bool {
    text.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
}

fn line_noise_ratio(text: &str) -> f32 {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return 1.0;
    }
    let total = chars.len() as f32;
    let weird = chars
        .iter()
        .filter(|c| {
            **c == '\u{FFFD}'
                || (c.is_control() && **c != '\n' && **c != '\r' && **c != '\t')
                || (!c.is_ascii_alphanumeric()
                    && !c.is_whitespace()
                    && !('가'..='힣').contains(&**c)
                    && !",.;:!?()[]{}+-_/@#'\"%&•*-".contains(**c))
        })
        .count() as f32;
    weird / total
}

pub fn is_overlay_or_noise_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    let overlay_markers = [
        "control center",
        "refresh preflight",
        "mode:observe",
        "mode:copilot",
        "mode:autopilot",
        "automation:enabled",
        "status:running",
        "rust_webview_eval_ok",
        "suggestions",
        "e-stop:off",
        "notion: true",
        "gmail: true",
        "필류 모드",
    ];
    if overlay_markers.iter().any(|m| lower.contains(m)) {
        return true;
    }
    if line_has_hangul(line) && line_has_cyrillic(line) {
        return true;
    }
    line_noise_ratio(line) > 0.34
}

pub fn normalize_screen_summary(raw: &str) -> String {
    let mut lines: Vec<String> = raw
        .replace("```", "")
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with('#'))
        .filter(|l| !is_overlay_or_noise_line(l))
        .map(|l| l.to_string())
        .collect();

    if lines.is_empty() {
        return String::new();
    }

    let mut dedup = Vec::new();
    for line in lines {
        let normalized = line.to_lowercase().replace(' ', "");
        if normalized.len() < 2 {
            continue;
        }
        if dedup
            .iter()
            .any(|existing: &String| existing.to_lowercase().replace(' ', "") == normalized)
        {
            continue;
        }
        dedup.push(line);
    }
    lines = dedup;

    let has_bullets = lines
        .iter()
        .any(|l| l.starts_with("- ") || l.starts_with("* ") || l.starts_with("• "));

    if !has_bullets {
        lines = lines
            .into_iter()
            .take(10)
            .map(|l| format!("- {}", l))
            .collect();
    }

    lines.into_iter().take(12).collect::<Vec<_>>().join("\n")
}

pub fn score_summary_output_quality(summary: &str) -> f32 {
    if summary.trim().is_empty() {
        return 0.0;
    }
    let lines: Vec<&str> = summary
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return 0.0;
    }
    let clean_lines = lines.iter().filter(|l| !is_overlay_or_noise_line(l)).count();
    let clean_ratio = clean_lines as f32 / lines.len() as f32;
    let text_quality = score_text_signal_quality(summary);
    let line_count_score = ((lines.len().min(12) as f32) / 8.0).min(1.0);
    (0.55 * text_quality + 0.30 * clean_ratio + 0.15 * line_count_score).clamp(0.0, 1.0)
}
