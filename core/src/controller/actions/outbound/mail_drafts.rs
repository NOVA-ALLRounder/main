use anyhow::Result;
use crate::platform::{current_platform, PlatformKind};

use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn mail_ensure_draft(
        goal: Option<&str>,
        history: &mut Vec<String>,
    ) -> Result<String> {
        let preferred_id = Self::mail_current_draft_id(history).unwrap_or_default();
        let recipient_hint = Self::preferred_mail_recipient(goal).unwrap_or_default();
        let marker_hint = Self::preferred_run_scope_marker(goal).unwrap_or_default();
        Self::mail_guard_outgoing_drafts(goal, Some(preferred_id.as_str()))?;
        if current_platform().kind() == PlatformKind::Windows {
            let draft_id =
                current_platform().ensure_mail_draft(&preferred_id, &recipient_hint, &marker_hint)?;
            let trimmed = draft_id.trim().to_string();
            Self::remember_mail_draft_id(history, &trimmed);
            return Ok(trimmed);
        }
        let lines = [
            "on run argv",
            "set preferredId to \"\"",
            "set recipientHint to \"\"",
            "set markerHint to \"\"",
            "if (count of argv) >= 1 then set preferredId to item 1 of argv",
            "if (count of argv) >= 2 then set recipientHint to item 2 of argv",
            "if (count of argv) >= 3 then set markerHint to item 3 of argv",
            "tell application \"Mail\"",
            "activate",
            "repeat with candidate in outgoing messages",
            "try",
            "set visible of candidate to false",
            "end try",
            "end repeat",
            "set _msg to missing value",
            "if preferredId is not \"\" then",
            "repeat with candidate in outgoing messages",
            "try",
            "if (id of candidate as text) is preferredId then",
            "set _msg to candidate",
            "exit repeat",
            "end if",
            "end try",
            "end repeat",
            "end if",
            "if _msg is missing value and markerHint is not \"\" then",
            "repeat with candidate in outgoing messages",
            "try",
            "set candidateSubject to \"\"",
            "set candidateBody to \"\"",
            "try",
            "set candidateSubject to subject of candidate as text",
            "end try",
            "try",
            "set candidateBody to content of candidate as text",
            "end try",
            "if candidateSubject contains markerHint or candidateBody contains markerHint then",
            "set _msg to candidate",
            "exit repeat",
            "end if",
            "end try",
            "end repeat",
            "end if",
            "if _msg is missing value then",
            "if (count of outgoing messages) > 0 then",
            "set _msg to (last outgoing message)",
            "else",
            "set _msg to make new outgoing message with properties {visible:false}",
            "end if",
            "end if",
            "if markerHint is not \"\" then",
            "set markerMatch to false",
            "try",
            "set currentSubject to subject of _msg as text",
            "if currentSubject contains markerHint then set markerMatch to true",
            "end try",
            "if markerMatch is false then",
            "try",
            "set currentBody to content of _msg as text",
            "if currentBody contains markerHint then set markerMatch to true",
            "end try",
            "end if",
            "if markerMatch is false then",
            "set _msg to make new outgoing message with properties {visible:false}",
            "end if",
            "end if",
            "set visible of _msg to false",
            "if recipientHint is not \"\" then",
            "set hasTarget to false",
            "repeat with r in to recipients of _msg",
            "try",
            "if (address of r as text) is recipientHint then set hasTarget to true",
            "end try",
            "end repeat",
            "if hasTarget is false then",
            "make new to recipient at end of to recipients of _msg with properties {address:recipientHint}",
            "end if",
            "end if",
            "set draftId to \"\"",
            "try",
            "set draftId to (id of _msg as text)",
            "end try",
            "end tell",
            "return draftId",
            "end run",
        ];
        let draft_id = crate::applescript::run_with_args(
            &lines,
            &[preferred_id, recipient_hint, marker_hint],
        )?;
        let trimmed = draft_id.trim().to_string();
        Self::remember_mail_draft_id(history, &trimmed);
        Ok(trimmed)
    }

    pub(in crate::controller::actions) fn mail_set_recipient_if_missing(
        goal: Option<&str>,
        draft_id: Option<&str>,
    ) -> Result<()> {
        let Some(address) = Self::preferred_mail_recipient(goal) else {
            return Ok(());
        };
        let draft_hint = draft_id.unwrap_or_default().to_string();
        if current_platform().kind() == PlatformKind::Windows {
            let out = current_platform().set_mail_recipient_if_missing(&address, &draft_hint)?;
            let mut parts = out.trim().split('|');
            let status = parts.next().unwrap_or("").trim();
            if status != "ok" {
                return Err(anyhow::anyhow!(
                    "mail recipient target unavailable: {}",
                    out.trim()
                ));
            }
            return Ok(());
        }
        let lines = [
            "on run argv",
            "set toAddress to item 1 of argv",
            "set draftHint to \"\"",
            "if (count of argv) >= 2 then set draftHint to item 2 of argv",
            "tell application \"Mail\"",
            "activate",
            "set _msg to missing value",
            "if draftHint is not \"\" then",
            "repeat with candidate in outgoing messages",
            "try",
            "if (id of candidate as text) is draftHint then",
            "set _msg to candidate",
            "exit repeat",
            "end if",
            "end try",
            "end repeat",
            "end if",
            "if _msg is missing value then",
            "if draftHint is not \"\" then return \"draft_not_found|\" & draftHint",
            "if (count of outgoing messages) > 0 then",
            "set _msg to (last outgoing message)",
            "else",
            "return \"no_draft|\"",
            "end if",
            "end if",
            "set visible of _msg to false",
            "set hasTarget to false",
            "repeat with r in to recipients of _msg",
            "try",
            "if (address of r as text) is toAddress then set hasTarget to true",
            "end try",
            "end repeat",
            "if hasTarget is false then",
            "make new to recipient at end of to recipients of _msg with properties {address:toAddress}",
            "end if",
            "set draftId to \"\"",
            "try",
            "set draftId to (id of _msg as text)",
            "end try",
            "end tell",
            "return \"ok|\" & draftId",
            "end run",
        ];
        let out = crate::applescript::run_with_args(&lines, &[address, draft_hint])?;
        let mut parts = out.trim().split('|');
        let status = parts.next().unwrap_or("").trim();
        if status != "ok" {
            return Err(anyhow::anyhow!(
                "mail recipient target unavailable: {}",
                out.trim()
            ));
        }
        Ok(())
    }

    pub(in crate::controller::actions) fn mail_outgoing_count() -> Result<i64> {
        if current_platform().kind() == PlatformKind::Windows {
            return current_platform().outgoing_mail_draft_count();
        }
        let lines = [
            "tell application \"Mail\"",
            "return (count of outgoing messages)",
            "end tell",
        ];
        let out = crate::applescript::run_with_args(&lines, &Vec::<String>::new())?;
        Ok(out.trim().parse::<i64>().unwrap_or(0))
    }

    pub(in crate::controller::actions) fn mail_max_outgoing_for_auto_draft() -> i64 {
        std::env::var("STEER_MAIL_MAX_OUTGOING_FOR_AUTO_DRAFT")
            .ok()
            .and_then(|v| v.trim().parse::<i64>().ok())
            .map(|v| v.clamp(1, 64))
            .unwrap_or(8)
    }

    pub(in crate::controller::actions) fn mail_guard_outgoing_drafts(
        goal: Option<&str>,
        keep_draft_id: Option<&str>,
    ) -> Result<()> {
        let limit = Self::mail_max_outgoing_for_auto_draft();
        let before = Self::mail_outgoing_count().unwrap_or(0);
        if before <= limit {
            return Ok(());
        }

        let _ = Self::mail_cleanup_marker_outgoing(goal, keep_draft_id);
        let after = Self::mail_outgoing_count().unwrap_or(before);
        if after <= limit {
            return Ok(());
        }

        Err(anyhow::anyhow!(
            "ambiguous_draft: outgoing drafts {} exceed limit {} (set STEER_MAIL_MAX_OUTGOING_FOR_AUTO_DRAFT or clean Mail drafts)",
            after,
            limit
        ))
    }

    pub(in crate::controller::actions) fn mail_cleanup_marker_outgoing(
        goal: Option<&str>,
        keep_draft_id: Option<&str>,
    ) -> Result<i64> {
        let marker_hint = Self::preferred_run_scope_marker(goal).unwrap_or_default();
        if marker_hint.trim().is_empty() {
            return Ok(0);
        }
        let keep_hint = keep_draft_id.unwrap_or_default().trim().to_string();
        if current_platform().kind() == PlatformKind::Windows {
            return current_platform().cleanup_outgoing_mail_drafts(&marker_hint, &keep_hint);
        }
        let lines = [
            "on run argv",
            "set markerHint to item 1 of argv",
            "set keepDraftId to \"\"",
            "if (count of argv) >= 2 then set keepDraftId to item 2 of argv",
            "if markerHint is \"\" then return \"0\"",
            "set removedCount to 0",
            "tell application \"Mail\"",
            "set totalOutgoing to (count of outgoing messages)",
            "if totalOutgoing > 0 then",
            "repeat with idx from totalOutgoing to 1 by -1",
            "set candidate to item idx of outgoing messages",
            "set candidateId to \"\"",
            "set candidateSubject to \"\"",
            "set candidateBody to \"\"",
            "try",
            "set candidateId to (id of candidate as text)",
            "end try",
            "if keepDraftId is not \"\" and candidateId is keepDraftId then",
            "-- keep this draft",
            "else",
            "try",
            "set candidateSubject to (subject of candidate as text)",
            "end try",
            "try",
            "set candidateBody to (content of candidate as text)",
            "end try",
            "if candidateBody is missing value then set candidateBody to \"\"",
            "if candidateSubject contains markerHint or candidateBody contains markerHint then",
            "try",
            "delete candidate",
            "set removedCount to removedCount + 1",
            "end try",
            "end if",
            "end if",
            "end repeat",
            "end if",
            "end tell",
            "return removedCount as text",
            "end run",
        ];
        let out = crate::applescript::run_with_args(&lines, &[marker_hint, keep_hint])?;
        Ok(out.trim().parse::<i64>().unwrap_or(0))
    }

    pub(in crate::controller::actions) fn mail_set_subject(
        subject: &str,
        draft_id: Option<&str>,
    ) -> Result<String> {
        let draft_hint = draft_id.unwrap_or_default().to_string();
        if current_platform().kind() == PlatformKind::Windows {
            let out = current_platform().set_mail_subject(subject, &draft_hint)?;
            let trimmed = out.trim();
            let mut parts = trimmed.split('|');
            let status = parts.next().unwrap_or("").trim();
            if status != "ok" {
                return Err(anyhow::anyhow!(
                    "mail subject target unavailable: {}",
                    trimmed
                ));
            }
            return Ok(parts.next().unwrap_or("").trim().to_string());
        }
        let lines = [
            "on run argv",
            "set subjectText to item 1 of argv",
            "set draftHint to \"\"",
            "if (count of argv) >= 2 then set draftHint to item 2 of argv",
            "tell application \"Mail\"",
            "activate",
            "set _msg to missing value",
            "if draftHint is not \"\" then",
            "repeat with candidate in outgoing messages",
            "try",
            "if (id of candidate as text) is draftHint then",
            "set _msg to candidate",
            "exit repeat",
            "end if",
            "end try",
            "end repeat",
            "end if",
            "if _msg is missing value then",
            "if draftHint is not \"\" then return \"draft_not_found|\" & draftHint",
            "if (count of outgoing messages) > 0 then",
            "set _msg to (last outgoing message)",
            "else",
            "return \"no_draft|\"",
            "end if",
            "end if",
            "set visible of _msg to false",
            "set subject of _msg to subjectText",
            "set draftId to \"\"",
            "try",
            "set draftId to (id of _msg as text)",
            "end try",
            "end tell",
            "return \"ok|\" & draftId",
            "end run",
        ];
        let out = crate::applescript::run_with_args(&lines, &[subject.to_string(), draft_hint])?;
        let trimmed = out.trim();
        let mut parts = trimmed.split('|');
        let status = parts.next().unwrap_or("").trim();
        if status != "ok" {
            return Err(anyhow::anyhow!(
                "mail subject target unavailable: {}",
                trimmed
            ));
        }
        Ok(parts.next().unwrap_or("").trim().to_string())
    }

    pub(in crate::controller::actions) fn mail_append_body(
        text: &str,
        draft_id: Option<&str>,
    ) -> Result<(String, i64)> {
        let draft_hint = draft_id.unwrap_or_default().to_string();
        if current_platform().kind() == PlatformKind::Windows {
            return current_platform().append_mail_body(text, &draft_hint);
        }
        let lines = [
            "on run argv",
            "set bodyText to item 1 of argv",
            "set draftHint to \"\"",
            "if (count of argv) >= 2 then set draftHint to item 2 of argv",
            "tell application \"Mail\"",
            "activate",
            "set _msg to missing value",
            "if draftHint is not \"\" then",
            "repeat with candidate in outgoing messages",
            "try",
            "if (id of candidate as text) is draftHint then",
            "set _msg to candidate",
            "exit repeat",
            "end if",
            "end try",
            "end repeat",
            "end if",
            "if _msg is missing value then",
            "if draftHint is not \"\" then return \"draft_not_found|\" & draftHint & \"|0\"",
            "if (count of outgoing messages) > 0 then",
            "set _msg to (last outgoing message)",
            "else",
            "return \"no_draft||0\"",
            "end if",
            "end if",
            "set visible of _msg to false",
            "set existingContent to content of _msg",
            "if existingContent is missing value then set existingContent to \"\"",
            "if existingContent is \"\" then",
            "set content of _msg to bodyText",
            "else",
            "set content of _msg to existingContent & return & bodyText",
            "end if",
            "set draftId to \"\"",
            "set bodyLen to 0",
            "try",
            "set draftId to (id of _msg as text)",
            "end try",
            "try",
            "set bodyLen to (length of (content of _msg as text))",
            "end try",
            "end tell",
            "return \"ok|\" & draftId & \"|\" & bodyLen",
            "end run",
        ];
        let out = crate::applescript::run_with_args(&lines, &[text.to_string(), draft_hint])?;
        let trimmed = out.trim();
        let mut parts = trimmed.split('|');
        let status = parts.next().unwrap_or("").trim().to_string();
        if status != "ok" {
            return Err(anyhow::anyhow!("mail body target unavailable: {}", trimmed));
        }
        let id = parts.next().unwrap_or("").trim().to_string();
        let body_len = parts
            .next()
            .and_then(|v| v.trim().parse::<i64>().ok())
            .unwrap_or(0);
        Ok((id, body_len))
    }

    pub(in crate::controller::actions) fn mail_create_filled_draft(
        goal: Option<&str>,
        body_text: &str,
    ) -> Result<(String, i64)> {
        let subject_hint = Self::preferred_mail_subject(goal).unwrap_or_default();
        let recipient_hint = Self::preferred_mail_recipient(goal).unwrap_or_default();
        if current_platform().kind() == PlatformKind::Windows {
            return current_platform().create_filled_mail_draft(
                body_text,
                &subject_hint,
                &recipient_hint,
            );
        }
        let lines = [
            "on run argv",
            "set bodyText to item 1 of argv",
            "set subjectHint to \"\"",
            "set recipientHint to \"\"",
            "if (count of argv) >= 2 then set subjectHint to item 2 of argv",
            "if (count of argv) >= 3 then set recipientHint to item 3 of argv",
            "tell application \"Mail\"",
            "activate",
            "set _msg to make new outgoing message with properties {visible:false, content:bodyText}",
            "if subjectHint is not \"\" then set subject of _msg to subjectHint",
            "if recipientHint is not \"\" then",
            "set hasTarget to false",
            "repeat with r in to recipients of _msg",
            "try",
            "if (address of r as text) is recipientHint then set hasTarget to true",
            "end try",
            "end repeat",
            "if hasTarget is false then",
            "make new to recipient at end of to recipients of _msg with properties {address:recipientHint}",
            "end if",
            "end if",
            "set draftId to \"\"",
            "set bodyLen to 0",
            "try",
            "set draftId to (id of _msg as text)",
            "end try",
            "try",
            "set bodyLen to (length of (content of _msg as text))",
            "end try",
            "end tell",
            "return draftId & \"|\" & bodyLen",
            "end run",
        ];
        let out = crate::applescript::run_with_args(
            &lines,
            &[
                body_text.to_string(),
                subject_hint.to_string(),
                recipient_hint.to_string(),
            ],
        )?;
        let trimmed = out.trim();
        let mut parts = trimmed.split('|');
        let id = parts.next().unwrap_or("").trim().to_string();
        let body_len = parts
            .next()
            .and_then(|v| v.trim().parse::<i64>().ok())
            .unwrap_or(0);
        Ok((id, body_len))
    }
}
