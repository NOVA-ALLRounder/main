use anyhow::Result;

use super::super::{ActionRunner, TextEditWriteResult};
use crate::platform::{current_platform, PlatformKind};

fn build_textedit_platform_body(existing_text: &str, body_text: &str, marker: &str) -> String {
    let mut existing = existing_text
        .trim_end_matches(|c| c == '\r' || c == '\n')
        .to_string();

    if !marker.is_empty() && !existing.contains(marker) {
        if existing.is_empty() {
            existing = marker.to_string();
        } else {
            existing.push('\n');
            existing.push_str(marker);
        }
    }

    if existing.is_empty() {
        body_text.to_string()
    } else if body_text.is_empty() {
        existing
    } else {
        format!("{}\n{}", existing, body_text)
    }
}

impl ActionRunner {
    pub(in crate::controller::actions) fn parse_textedit_write_result(
        raw: &str,
    ) -> TextEditWriteResult {
        let mut parts = raw.trim().split('|');
        let _status = parts.next().unwrap_or("").trim().to_string();
        TextEditWriteResult {
            doc_id: parts.next().unwrap_or("").trim().to_string(),
            doc_name: parts.next().unwrap_or("").trim().to_string(),
            body_len: parts
                .next()
                .and_then(|v| v.trim().parse::<i64>().ok())
                .unwrap_or(0),
        }
    }

    pub(in crate::controller::actions) fn textedit_append_text(
        text: &str,
        goal: Option<&str>,
    ) -> Result<TextEditWriteResult> {
        let marker = Self::preferred_run_scope_marker(goal).unwrap_or_default();
        if current_platform().kind() != PlatformKind::MacOS {
            let existing_text =
                Self::capture_front_text_via_platform_sync(true, 120, 120).unwrap_or_default();
            let final_text = build_textedit_platform_body(&existing_text, text, &marker);
            Self::replace_front_text_via_platform_sync(&final_text, true, 120, 150)?;
            let doc_name = current_platform()
                .frontmost_app_name()?
                .unwrap_or_else(|| "Text Editor".to_string());
            return Ok(TextEditWriteResult {
                doc_id: "platform_fallback".to_string(),
                doc_name,
                body_len: final_text.chars().count() as i64,
            });
        }

        let isolate_unscoped = std::env::var("STEER_TEXTEDIT_ISOLATE_UNSCOPED")
            .map(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(true);
        let lines = [
            "on run argv",
            "set bodyText to item 1 of argv",
            "set markerText to \"\"",
            "if (count of argv) > 1 then set markerText to item 2 of argv",
            "set isolateUnscoped to true",
            "if (count of argv) > 2 then",
            "set isolateArg to item 3 of argv",
            "set isolateUnscoped to (isolateArg is \"1\" or isolateArg is \"true\" or isolateArg is \"yes\" or isolateArg is \"on\")",
            "end if",
            "tell application \"TextEdit\"",
            "activate",
            "if (count of documents) = 0 then make new document",
            "set targetDoc to missing value",
            "if markerText is not \"\" then",
            "set docCount to count of documents",
            "repeat with idx from docCount to 1 by -1",
            "set candidateDoc to item idx of documents",
            "set candidateText to \"\"",
            "try",
            "set candidateText to text of candidateDoc as text",
            "end try",
            "if candidateText contains markerText then",
            "set targetDoc to candidateDoc",
            "exit repeat",
            "end if",
            "end repeat",
            "end if",
            "if targetDoc is missing value then",
            "if markerText is not \"\" or isolateUnscoped then",
            "make new document",
            "set targetDoc to front document",
            "else",
            "set targetDoc to front document",
            "end if",
            "end if",
            "set existingText to \"\"",
            "try",
            "set existingText to text of targetDoc as text",
            "end try",
            "if markerText is not \"\" and existingText does not contain markerText then",
            "if existingText is \"\" then",
            "set existingText to markerText",
            "else",
            "set existingText to existingText & return & markerText",
            "end if",
            "end if",
            "if existingText is \"\" then",
            "set text of targetDoc to bodyText",
            "else",
            "set text of targetDoc to existingText & return & bodyText",
            "end if",
            "set docId to \"\"",
            "set docName to \"\"",
            "set bodyLen to 0",
            "try",
            "set docId to id of targetDoc as text",
            "end try",
            "try",
            "set docName to name of targetDoc as text",
            "end try",
            "try",
            "set bodyLen to length of (text of targetDoc as text)",
            "end try",
            "end tell",
            "return \"ok|\" & docId & \"|\" & docName & \"|\" & bodyLen",
            "end run",
        ];
        let isolate_arg = if isolate_unscoped { "1" } else { "0" };
        let out = crate::applescript::run_with_args(
            &lines,
            &[text.to_string(), marker, isolate_arg.to_string()],
        )?;
        Ok(Self::parse_textedit_write_result(&out))
    }

    pub(in crate::controller::actions) fn textedit_read_text(goal: Option<&str>) -> Result<String> {
        let marker = Self::preferred_run_scope_marker(goal).unwrap_or_default();
        if current_platform().kind() != PlatformKind::MacOS {
            let out = Self::capture_front_text_via_platform_sync(true, 120, 120)?;
            if !marker.is_empty() && !out.contains(&marker) {
                return Err(anyhow::anyhow!("textedit marker not found"));
            }
            return Ok(out);
        }

        let lines = [
            "on run argv",
            "set markerText to \"\"",
            "if (count of argv) > 0 then set markerText to item 1 of argv",
            "tell application \"TextEdit\"",
            "if (count of documents) = 0 then return \"\"",
            "set targetDoc to front document",
            "set markerFound to false",
            "if markerText is not \"\" then",
            "set docCount to count of documents",
            "repeat with idx from docCount to 1 by -1",
            "set candidateDoc to item idx of documents",
            "set candidateText to \"\"",
            "try",
            "set candidateText to text of candidateDoc as text",
            "end try",
            "if candidateText contains markerText then",
            "set targetDoc to candidateDoc",
            "set markerFound to true",
            "exit repeat",
            "end if",
            "end repeat",
            "end if",
            "if markerText is not \"\" and markerFound is false then return \"__STEER_MARKER_NOT_FOUND__\"",
            "set outText to \"\"",
            "try",
            "set outText to text of targetDoc as text",
            "end try",
            "return outText",
            "end tell",
            "end run",
        ];
        let out = crate::applescript::run_with_args(&lines, &[marker])?;
        if out.trim() == "__STEER_MARKER_NOT_FOUND__" {
            return Err(anyhow::anyhow!("textedit marker not found"));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::build_textedit_platform_body;

    #[test]
    fn platform_body_inserts_marker_when_missing() {
        let merged = build_textedit_platform_body("hello", "world", "RUN_SCOPE_123");
        assert_eq!(merged, "hello\nRUN_SCOPE_123\nworld");
    }

    #[test]
    fn platform_body_reuses_existing_marker() {
        let merged = build_textedit_platform_body("hello\nRUN_SCOPE_123", "world", "RUN_SCOPE_123");
        assert_eq!(merged, "hello\nRUN_SCOPE_123\nworld");
    }
}
