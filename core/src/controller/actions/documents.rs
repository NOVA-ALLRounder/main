use anyhow::Result;

use super::{ActionRunner, NotesWriteResult, TextEditWriteResult};

impl ActionRunner {
    pub(super) fn parse_notes_write_result(raw: &str) -> NotesWriteResult {
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

    pub(super) fn notes_write_text(text: &str, goal: Option<&str>) -> Result<NotesWriteResult> {
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

    pub(super) fn notes_read_text(goal: Option<&str>) -> Result<String> {
        let marker = Self::preferred_run_scope_marker(goal).unwrap_or_default();
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

    pub(super) fn parse_textedit_write_result(raw: &str) -> TextEditWriteResult {
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

    pub(super) fn textedit_append_text(
        text: &str,
        goal: Option<&str>,
    ) -> Result<TextEditWriteResult> {
        let marker = Self::preferred_run_scope_marker(goal).unwrap_or_default();
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

    pub(super) fn textedit_read_text(goal: Option<&str>) -> Result<String> {
        let marker = Self::preferred_run_scope_marker(goal).unwrap_or_default();
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
