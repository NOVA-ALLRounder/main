use anyhow::Result;

use super::super::{ActionRunner, NotesWriteResult};
use crate::platform::{current_platform, PlatformKind};

fn notes_platform_plaintext_body(raw: &str) -> String {
    ActionRunner::decode_xml_entities(
        &raw.replace("</div><div>", "\n")
            .replace("</div>\n<div>", "\n")
            .replace("<div>", "")
            .replace("</div>", "")
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<br />", "\n"),
    )
    .trim()
    .to_string()
}

impl ActionRunner {
    pub(in crate::controller::actions) fn parse_notes_write_result(raw: &str) -> NotesWriteResult {
        let mut parts = raw.trim().split('|');
        let _status = parts.next().unwrap_or("").trim().to_string();
        NotesWriteResult {
            note_id: parts.next().unwrap_or("").trim().to_string(),
            note_name: parts.next().unwrap_or("").trim().to_string(),
            body_len: parts
                .next()
                .and_then(|v| v.trim().parse::<i64>().ok())
                .unwrap_or(0),
        }
    }

    pub(in crate::controller::actions) fn notes_write_text(
        text: &str,
        goal: Option<&str>,
    ) -> Result<NotesWriteResult> {
        let marker = Self::preferred_run_scope_marker(goal).unwrap_or_default();
        let goal_text = goal.unwrap_or_default().to_lowercase();
        let must_append_marker = !marker.is_empty()
            && (goal_text.contains("마지막 줄")
                || goal_text.contains("last line")
                || goal_text.contains("정확히 입력")
                || goal_text.contains("exactly input"));
        let mut body_text = text.to_string();
        if must_append_marker && !body_text.contains(&marker) {
            let first_line = body_text
                .lines()
                .map(|line| line.trim())
                .find(|line| !line.is_empty())
                .unwrap_or_else(|| body_text.trim());
            if first_line.is_empty() {
                body_text = marker.clone();
            } else {
                // Notes body accepts lightweight HTML; using two div blocks preserves line split
                // instead of collapsing plain-text newlines into a single line.
                body_text = format!("<div>{}</div><div>{}</div>", first_line, marker);
            }
        }
        if current_platform().kind() != PlatformKind::MacOS {
            let platform_body = notes_platform_plaintext_body(&body_text);
            Self::replace_front_text_via_platform_sync(&platform_body, true, 120, 150)?;
            let note_name = current_platform()
                .frontmost_app_name()?
                .unwrap_or_else(|| "Notes".to_string());
            return Ok(NotesWriteResult {
                note_id: "platform_fallback".to_string(),
                note_name,
                body_len: platform_body.chars().count() as i64,
            });
        }

        let lines = [
            "on run argv",
            "set bodyText to item 1 of argv",
            "set markerText to \"\"",
            "if (count of argv) > 1 then set markerText to item 2 of argv",
            "set noteTitle to \"\"",
            "if noteTitle is \"\" then",
            "try",
            "if bodyText contains return then",
            "set noteTitle to text 1 thru ((offset of return in bodyText) - 1) of bodyText",
            "else",
            "set noteTitle to bodyText",
            "end if",
            "on error",
            "set noteTitle to bodyText",
            "end try",
            "end if",
            "if noteTitle is \"\" then set noteTitle to \"Steer Note\"",
            "tell application \"Notes\"",
            "activate",
            "if (count of accounts) = 0 then return \"ok|||0\"",
            "set fd to missing value",
            "set ac to missing value",
            "if fd is missing value then",
            "try",
            "set ac to first account whose name is \"iCloud\"",
            "end try",
            "if ac is not missing value then",
            "if (count of folders of ac) = 0 then",
            "set fd to make new folder at ac with properties {name:\"Notes\"}",
            "else",
            "try",
            "set fd to first folder of ac whose name is \"메모\"",
            "end try",
            "if fd is missing value then",
            "try",
            "set fd to first folder of ac whose name is \"Notes\"",
            "end try",
            "end if",
            "if fd is missing value then set fd to item 1 of folders of ac",
            "end if",
            "end if",
            "end if",
            "if fd is missing value then",
            "try",
            "if (count of selection) > 0 then",
            "set selectedNote to item 1 of selection",
            "set fd to container of selectedNote",
            "end if",
            "end try",
            "end if",
            "if fd is missing value then",
            "if ac is missing value then set ac to item 1 of accounts",
            "if (count of folders of ac) = 0 then",
            "set fd to make new folder at ac with properties {name:\"Notes\"}",
            "else",
            "try",
            "set fd to first folder of ac whose name is \"메모\"",
            "end try",
            "if fd is missing value then",
            "try",
            "set fd to first folder of ac whose name is \"Notes\"",
            "end try",
            "end if",
            "if fd is missing value then set fd to item 1 of folders of ac",
            "end if",
            "end if",
            "end if",
            "set targetNote to missing value",
            "if markerText is not \"\" then",
            "try",
            "set targetNote to first note of fd whose name is markerText",
            "end try",
            "if targetNote is missing value then",
            "set noteCount to count of notes of fd",
            "repeat with idx from 1 to noteCount by 1",
            "set candidate to item idx of notes of fd",
            "set cName to \"\"",
            "set cBody to \"\"",
            "try",
            "set cName to name of candidate as text",
            "end try",
            "try",
            "set cBody to body of candidate as text",
            "end try",
            "if cName contains markerText or cBody contains markerText then",
            "set targetNote to candidate",
            "exit repeat",
            "end if",
            "end repeat",
            "end if",
            "end if",
            "if targetNote is missing value then",
            "set targetNote to make new note at fd with properties {body:bodyText}",
            "else",
            "if bodyText is not \"\" then set body of targetNote to bodyText",
            "end if",
            "set noteId to \"\"",
            "set noteNameOut to \"\"",
            "set noteBodyLen to 0",
            "try",
            "set noteId to id of targetNote as text",
            "end try",
            "try",
            "set noteNameOut to name of targetNote as text",
            "end try",
            "try",
            "set noteBodyLen to length of (body of targetNote as text)",
            "end try",
            "end tell",
            "return \"ok|\" & noteId & \"|\" & noteNameOut & \"|\" & noteBodyLen",
            "end run",
        ];
        let out = crate::applescript::run_with_args(&lines, &[body_text, marker])?;
        Ok(Self::parse_notes_write_result(&out))
    }

    pub(in crate::controller::actions) fn notes_read_text(goal: Option<&str>) -> Result<String> {
        let marker = Self::preferred_run_scope_marker(goal).unwrap_or_default();
        if current_platform().kind() != PlatformKind::MacOS {
            let out =
                notes_platform_plaintext_body(&Self::capture_front_text_via_platform_sync(
                    true, 120, 120,
                )?);
            if !marker.is_empty() && !out.contains(&marker) {
                return Err(anyhow::anyhow!("notes marker not found"));
            }
            return Ok(out);
        }

        let lines = [
            "on run argv",
            "set markerText to \"\"",
            "if (count of argv) > 0 then set markerText to item 1 of argv",
            "tell application \"Notes\"",
            "if (count of accounts) = 0 then return \"\"",
            "set ac to item 1 of accounts",
            "try",
            "set ac to first account whose name is \"iCloud\"",
            "end try",
            "if (count of folders of ac) = 0 then return \"\"",
            "set fd to missing value",
            "try",
            "set fd to first folder of ac whose name is \"메모\"",
            "end try",
            "if fd is missing value then",
            "try",
            "set fd to first folder of ac whose name is \"Notes\"",
            "end try",
            "end if",
            "if fd is missing value then set fd to item 1 of folders of ac",
            "if (count of notes of fd) = 0 then return \"\"",
            "set n to missing value",
            "if markerText is not \"\" then",
            "set noteCount to count of notes of fd",
            "repeat with idx from 1 to noteCount by 1",
            "set candidate to item idx of notes of fd",
            "set cName to \"\"",
            "set cBody to \"\"",
            "try",
            "set cName to name of candidate as text",
            "end try",
            "try",
            "set cBody to body of candidate as text",
            "end try",
            "if cName contains markerText or cBody contains markerText then",
            "set n to candidate",
            "exit repeat",
            "end if",
            "end repeat",
            "end if",
            "if n is missing value then",
            "if markerText is not \"\" then return \"__STEER_MARKER_NOT_FOUND__\"",
            "set n to first note of fd",
            "end if",
            "set nName to \"\"",
            "set nBody to \"\"",
            "try",
            "set nName to name of n as text",
            "end try",
            "try",
            "set nBody to body of n as text",
            "end try",
            "if nBody is \"\" then return nName",
            "return nName & return & nBody",
            "end tell",
            "end run",
        ];
        let out = crate::applescript::run_with_args(&lines, &[marker])?;
        if out.trim() == "__STEER_MARKER_NOT_FOUND__" {
            return Err(anyhow::anyhow!("notes marker not found"));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::notes_platform_plaintext_body;

    #[test]
    fn platform_body_preserves_div_boundaries() {
        let plain = notes_platform_plaintext_body("<div>Hello</div><div>RUN_SCOPE_123</div>");
        assert_eq!(plain, "Hello\nRUN_SCOPE_123");
    }

    #[test]
    fn platform_body_handles_br_tags() {
        let plain = notes_platform_plaintext_body("Hello<br/>World");
        assert_eq!(plain, "Hello\nWorld");
    }
}
