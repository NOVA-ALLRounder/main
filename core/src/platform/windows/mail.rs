use anyhow::{anyhow, Result};

use super::run_powershell;

const OUTLOOK_SCRIPT_PREAMBLE: &str = r#"
$outlook = $null
try { $outlook = New-Object -ComObject Outlook.Application } catch { Write-Output "OUTLOOK_UNAVAILABLE"; return }
$namespace = $null
try { $namespace = $outlook.GetNamespace('MAPI') } catch { Write-Output "OUTLOOK_UNAVAILABLE"; return }
$drafts = $null
try { $drafts = $namespace.GetDefaultFolder(16) } catch { Write-Output "OUTLOOK_UNAVAILABLE"; return }

function Is-DraftMail($item) {
  try { return ($null -ne $item -and $item.Class -eq 43 -and -not [bool]$item.Sent) } catch { return $false }
}

function Draft-EntryId($item) {
  try { return [string]$item.EntryID } catch { return "" }
}

function Draft-Subject($item) {
  try { return [string]$item.Subject } catch { return "" }
}

function Draft-Body($item) {
  try { return [string]$item.Body } catch { return "" }
}

function Draft-To($item) {
  try { return [string]$item.To } catch { return "" }
}

function Find-DraftByEntryId([string]$draftHint) {
  if ([string]::IsNullOrWhiteSpace($draftHint)) { return $null }
  for ($i = $drafts.Items.Count; $i -ge 1; $i--) {
    try {
      $candidate = $drafts.Items.Item($i)
      if ((Is-DraftMail $candidate) -and (Draft-EntryId $candidate) -eq $draftHint) { return $candidate }
    } catch { }
  }
  return $null
}

function Find-DraftByMarker([string]$markerHint) {
  if ([string]::IsNullOrWhiteSpace($markerHint)) { return $null }
  for ($i = $drafts.Items.Count; $i -ge 1; $i--) {
    try {
      $candidate = $drafts.Items.Item($i)
      if (-not (Is-DraftMail $candidate)) { continue }
      if ((Draft-Subject $candidate).Contains($markerHint) -or (Draft-Body $candidate).Contains($markerHint)) {
        return $candidate
      }
    } catch { }
  }
  return $null
}

function Find-LatestDraft() {
  for ($i = $drafts.Items.Count; $i -ge 1; $i--) {
    try {
      $candidate = $drafts.Items.Item($i)
      if (Is-DraftMail $candidate) { return $candidate }
    } catch { }
  }
  return $null
}

function Count-Drafts() {
  $count = 0
  for ($i = $drafts.Items.Count; $i -ge 1; $i--) {
    try {
      $candidate = $drafts.Items.Item($i)
      if (Is-DraftMail $candidate) { $count++ }
    } catch { }
  }
  return $count
}

function Ensure-Recipient($item, [string]$recipient) {
  if ($null -eq $item -or [string]::IsNullOrWhiteSpace($recipient)) { return }
  $current = Draft-To $item
  if ([string]::IsNullOrWhiteSpace($current)) {
    try { $item.To = $recipient } catch { }
    return
  }
  if ($current -notlike "*$recipient*") {
    $trimmed = $current.Trim()
    if ($trimmed.EndsWith(";")) {
      try { $item.To = ($trimmed + " " + $recipient) } catch { }
    } else {
      try { $item.To = ($trimmed + "; " + $recipient) } catch { }
    }
  }
}
"#;

fn escape_powershell_single_quoted(text: &str) -> String {
    text.replace('\'', "''")
}

fn run_mail_script(body: &str) -> Result<String> {
    let output = run_powershell(&format!("{}\n{}", OUTLOOK_SCRIPT_PREAMBLE, body))?;
    let trimmed = output.trim();
    if trimmed.eq("OUTLOOK_UNAVAILABLE") {
        Err(anyhow!("outlook_unavailable"))
    } else {
        Ok(trimmed.to_string())
    }
}

pub(super) fn ensure_mail_draft(
    preferred_id: &str,
    recipient_hint: &str,
    marker_hint: &str,
) -> Result<String> {
    let body = format!(
        r#"
$preferredId = '{preferred_id}'
$recipientHint = '{recipient_hint}'
$markerHint = '{marker_hint}'
$item = $null
if ($preferredId -ne '') {{ $item = Find-DraftByEntryId $preferredId }}
if ($null -eq $item -and $markerHint -ne '') {{ $item = Find-DraftByMarker $markerHint }}
if ($null -eq $item -and $markerHint -eq '') {{ $item = Find-LatestDraft }}
$needsFresh = $false
if ($null -eq $item) {{
  $needsFresh = $true
}} elseif ($markerHint -ne '') {{
  $subject = Draft-Subject $item
  $bodyText = Draft-Body $item
  if (($subject -notlike "*$markerHint*") -and ($bodyText -notlike "*$markerHint*")) {{ $needsFresh = $true }}
}}
if ($needsFresh) {{ $item = $outlook.CreateItem(0) }}
if ($null -eq $item) {{ Write-Output ''; return }}
Ensure-Recipient $item $recipientHint
try {{ $item.Save() }} catch {{ }}
Write-Output (Draft-EntryId $item)
"#,
        preferred_id = escape_powershell_single_quoted(preferred_id),
        recipient_hint = escape_powershell_single_quoted(recipient_hint),
        marker_hint = escape_powershell_single_quoted(marker_hint),
    );
    let output = run_mail_script(&body)?;
    if output.trim().is_empty() {
        Err(anyhow!("mail ensure draft returned empty id"))
    } else {
        Ok(output)
    }
}

pub(super) fn set_mail_recipient_if_missing(recipient: &str, draft_hint: &str) -> Result<String> {
    let body = format!(
        r#"
$recipient = '{recipient}'
$draftHint = '{draft_hint}'
$item = $null
if ($draftHint -ne '') {{ $item = Find-DraftByEntryId $draftHint }}
if ($null -eq $item) {{
  if ($draftHint -ne '') {{
    Write-Output ('draft_not_found|' + $draftHint)
    return
  }}
  $item = Find-LatestDraft
}}
if ($null -eq $item) {{
  Write-Output 'no_draft|'
  return
}}
Ensure-Recipient $item $recipient
try {{ $item.Save() }} catch {{ }}
Write-Output ('ok|' + (Draft-EntryId $item))
"#,
        recipient = escape_powershell_single_quoted(recipient),
        draft_hint = escape_powershell_single_quoted(draft_hint),
    );
    run_mail_script(&body)
}

pub(super) fn outgoing_mail_draft_count() -> Result<i64> {
    let output = run_mail_script("Write-Output (Count-Drafts)")?;
    Ok(output.trim().parse::<i64>().unwrap_or(0))
}

pub(super) fn cleanup_outgoing_mail_drafts(marker_hint: &str, keep_draft_id: &str) -> Result<i64> {
    let body = format!(
        r#"
$markerHint = '{marker_hint}'
$keepDraftId = '{keep_draft_id}'
if ($markerHint -eq '') {{ Write-Output '0'; return }}
$removed = 0
for ($i = $drafts.Items.Count; $i -ge 1; $i--) {{
  try {{
    $candidate = $drafts.Items.Item($i)
    if (-not (Is-DraftMail $candidate)) {{ continue }}
    $candidateId = Draft-EntryId $candidate
    if ($keepDraftId -ne '' -and $candidateId -eq $keepDraftId) {{ continue }}
    if ((Draft-Subject $candidate).Contains($markerHint) -or (Draft-Body $candidate).Contains($markerHint)) {{
      try {{ $candidate.Delete(); $removed++ }} catch {{ }}
    }}
  }} catch {{ }}
}}
Write-Output ($removed.ToString())
"#,
        marker_hint = escape_powershell_single_quoted(marker_hint),
        keep_draft_id = escape_powershell_single_quoted(keep_draft_id),
    );
    let output = run_mail_script(&body)?;
    Ok(output.trim().parse::<i64>().unwrap_or(0))
}

pub(super) fn set_mail_subject(subject: &str, draft_hint: &str) -> Result<String> {
    let body = format!(
        r#"
$subjectText = '{subject}'
$draftHint = '{draft_hint}'
$item = $null
if ($draftHint -ne '') {{ $item = Find-DraftByEntryId $draftHint }}
if ($null -eq $item) {{
  if ($draftHint -ne '') {{
    Write-Output ('draft_not_found|' + $draftHint)
    return
  }}
  $item = Find-LatestDraft
}}
if ($null -eq $item) {{
  Write-Output 'no_draft|'
  return
}}
try {{ $item.Subject = $subjectText }} catch {{ }}
try {{ $item.Save() }} catch {{ }}
Write-Output ('ok|' + (Draft-EntryId $item))
"#,
        subject = escape_powershell_single_quoted(subject),
        draft_hint = escape_powershell_single_quoted(draft_hint),
    );
    run_mail_script(&body)
}

pub(super) fn append_mail_body(text: &str, draft_hint: &str) -> Result<(String, i64)> {
    let body = format!(
        r#"
$bodyText = '{body_text}'
$draftHint = '{draft_hint}'
$item = $null
if ($draftHint -ne '') {{ $item = Find-DraftByEntryId $draftHint }}
if ($null -eq $item) {{
  if ($draftHint -ne '') {{
    Write-Output ('draft_not_found|' + $draftHint + '|0')
    return
  }}
  $item = Find-LatestDraft
}}
if ($null -eq $item) {{
  Write-Output 'no_draft||0'
  return
}}
$existing = Draft-Body $item
if ([string]::IsNullOrEmpty($existing)) {{
  try {{ $item.Body = $bodyText }} catch {{ }}
}} else {{
  try {{ $item.Body = ($existing + \"`r`n\" + $bodyText) }} catch {{ }}
}}
try {{ $item.Save() }} catch {{ }}
$draftId = Draft-EntryId $item
$bodyLen = (Draft-Body $item).Length
Write-Output ('ok|' + $draftId + '|' + $bodyLen)
"#,
        body_text = escape_powershell_single_quoted(text),
        draft_hint = escape_powershell_single_quoted(draft_hint),
    );
    let output = run_mail_script(&body)?;
    let mut parts = output.trim().split('|');
    let status = parts.next().unwrap_or("").trim();
    if status != "ok" {
        return Err(anyhow!("mail body target unavailable: {}", output.trim()));
    }
    let id = parts.next().unwrap_or("").trim().to_string();
    let body_len = parts
        .next()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(0);
    Ok((id, body_len))
}

pub(super) fn create_filled_mail_draft(
    body_text: &str,
    subject_hint: &str,
    recipient_hint: &str,
) -> Result<(String, i64)> {
    let body = format!(
        r#"
$bodyText = '{body_text}'
$subjectHint = '{subject_hint}'
$recipientHint = '{recipient_hint}'
$item = $outlook.CreateItem(0)
if ($null -eq $item) {{ Write-Output '|0'; return }}
try {{ $item.Body = $bodyText }} catch {{ }}
if ($subjectHint -ne '') {{ try {{ $item.Subject = $subjectHint }} catch {{ }} }}
Ensure-Recipient $item $recipientHint
try {{ $item.Save() }} catch {{ }}
$draftId = Draft-EntryId $item
$bodyLen = (Draft-Body $item).Length
Write-Output ($draftId + '|' + $bodyLen)
"#,
        body_text = escape_powershell_single_quoted(body_text),
        subject_hint = escape_powershell_single_quoted(subject_hint),
        recipient_hint = escape_powershell_single_quoted(recipient_hint),
    );
    let output = run_mail_script(&body)?;
    let mut parts = output.trim().split('|');
    let id = parts.next().unwrap_or("").trim().to_string();
    let body_len = parts
        .next()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(0);
    if id.is_empty() {
        Err(anyhow!("mail draft creation returned empty id"))
    } else {
        Ok((id, body_len))
    }
}

pub(super) fn send_mail_draft(
    fallback_address: &str,
    subject_hint: &str,
    marker_hint: &str,
    draft_hint: &str,
    strict_draft_check: bool,
) -> Result<String> {
    let strict = if strict_draft_check { "$true" } else { "$false" };
    let body = format!(
        r#"
$fallbackAddress = '{fallback_address}'
$subjectHint = '{subject_hint}'
$markerHint = '{marker_hint}'
$draftHint = '{draft_hint}'
$strictDraftCheck = {strict_draft_check}
$beforeOutgoing = Count-Drafts
if ($beforeOutgoing -eq 0 -and $draftHint -eq '') {{
  Write-Output 'no_draft|0|0||'
  return
}}
$item = $null
if ($draftHint -ne '') {{ $item = Find-DraftByEntryId $draftHint }}
if ($null -eq $item) {{
  if ($draftHint -ne '') {{
    Write-Output ('draft_not_found|' + $beforeOutgoing + '|' + $beforeOutgoing + '|||' + $draftHint + '|0')
    return
  }}
  $item = Find-LatestDraft
}}
if ($null -eq $item) {{
  Write-Output ('no_draft|' + $beforeOutgoing + '|' + $beforeOutgoing + '||| |0')
  return
}}
$subject = Draft-Subject $item
if ($subjectHint -ne '' -and [string]::IsNullOrWhiteSpace($subject)) {{
  try {{ $item.Subject = $subjectHint }} catch {{ }}
  $subject = $subjectHint
}}
Ensure-Recipient $item $fallbackAddress
$recipient = Draft-To $item
$draftId = Draft-EntryId $item
$bodyText = Draft-Body $item
$bodyLen = $bodyText.Length
if ([string]::IsNullOrWhiteSpace($recipient)) {{
  Write-Output ('missing_recipient|' + $beforeOutgoing + '|' + $beforeOutgoing + '||' + $subject)
  return
}}
if ($bodyLen -le 2) {{
  Write-Output ('empty_body|' + $beforeOutgoing + '|' + $beforeOutgoing + '|' + $recipient + '|' + $subject + '|' + $draftId + '|' + $bodyLen)
  return
}}
if ($strictDraftCheck -and $markerHint -ne '' -and (-not $subject.Contains($markerHint)) -and (-not $bodyText.Contains($markerHint))) {{
  Write-Output ('missing_marker|' + $beforeOutgoing + '|' + $beforeOutgoing + '|' + $recipient + '|' + $subject + '|' + $draftId + '|' + $bodyLen)
  return
}}
try {{
  $item.Send()
}} catch {{
  Write-Output ('send_failed|' + $beforeOutgoing + '|' + $beforeOutgoing + '|' + $recipient + '|' + $subject + '|' + $draftId + '|' + $bodyLen)
  return
}}
Start-Sleep -Milliseconds 1000
$afterOutgoing = Count-Drafts
$draftStillExists = $false
if ($draftId -ne '') {{
  $draftStillExists = ($null -ne (Find-DraftByEntryId $draftId))
}}
$status = 'sent_pending'
if (($afterOutgoing -lt $beforeOutgoing) -or (-not $draftStillExists)) {{
  $status = 'sent_confirmed'
}}
Write-Output ($status + '|' + $beforeOutgoing + '|' + $afterOutgoing + '|' + $recipient + '|' + $subject + '|' + $draftId + '|' + $bodyLen)
"#,
        fallback_address = escape_powershell_single_quoted(fallback_address),
        subject_hint = escape_powershell_single_quoted(subject_hint),
        marker_hint = escape_powershell_single_quoted(marker_hint),
        draft_hint = escape_powershell_single_quoted(draft_hint),
        strict_draft_check = strict,
    );
    run_mail_script(&body)
}
