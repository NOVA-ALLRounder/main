use anyhow::{Context, Result};
use serde::Deserialize;

use super::run_powershell;
use crate::platform::types::{UiAutomationProbe, UiBounds, UiSnapshotElement};

const FOREGROUND_WINDOW_SCRIPT: &str = r#"
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class WinForeground {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
}
"@;
$hwnd = [WinForeground]::GetForegroundWindow();
if ($hwnd -eq [IntPtr]::Zero) { return "" }
$pid = 0
[void][WinForeground]::GetWindowThreadProcessId($hwnd, [ref]$pid)
$proc = Get-Process -Id $pid -ErrorAction SilentlyContinue
$buffer = New-Object System.Text.StringBuilder 1024
[void][WinForeground]::GetWindowText($hwnd, $buffer, $buffer.Capacity)
$name = if ($proc) { $proc.ProcessName } else { "" }
$title = $buffer.ToString()
Write-Output ($name + " :: " + $title)
"#;

const BROWSER_SNAPSHOT_SCRIPT: &str = r#"
Add-Type -AssemblyName UIAutomationClient;
Add-Type -AssemblyName UIAutomationTypes;
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class WinForeground {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}
"@;

$hwnd = [WinForeground]::GetForegroundWindow();
if ($hwnd -eq [IntPtr]::Zero) {
  @{ items = @() } | ConvertTo-Json -Depth 4 -Compress
  return
}

$root = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
if ($null -eq $root) {
  @{ items = @() } | ConvertTo-Json -Depth 4 -Compress
  return
}

$items = New-Object System.Collections.Generic.List[object]
$condition = [System.Windows.Automation.Condition]::TrueCondition
$scope = [System.Windows.Automation.TreeScope]::Subtree

try {
  $elements = $root.FindAll($scope, $condition)
  $limit = [Math]::Min($elements.Count, 250)
  for ($i = 0; $i -lt $limit; $i++) {
    $element = $elements.Item($i)
    if ($null -eq $element) { continue }

    try {
      $current = $element.Current
      if ($current.IsOffscreen) { continue }

      $name = $current.Name
      if ([string]::IsNullOrWhiteSpace($name)) { $name = $current.AutomationId }
      if ([string]::IsNullOrWhiteSpace($name)) { $name = $current.HelpText }
      if ([string]::IsNullOrWhiteSpace($name)) { continue }

      $role = $current.ControlType.ProgrammaticName
      if ([string]::IsNullOrWhiteSpace($role)) { $role = $current.LocalizedControlType }
      if ([string]::IsNullOrWhiteSpace($role)) { $role = "Control" }
      if ($role.StartsWith("ControlType.")) {
        $role = $role.Substring("ControlType.".Length)
      }

      $rect = $current.BoundingRectangle
      $hasRect = ($rect.Width -gt 0 -and $rect.Height -gt 0)

      $items.Add([PSCustomObject]@{
        role = $role
        name = $name
        x = if ($hasRect) { [int][Math]::Round($rect.Left) } else { $null }
        y = if ($hasRect) { [int][Math]::Round($rect.Top) } else { $null }
        width = if ($hasRect) { [int][Math]::Round($rect.Width) } else { $null }
        height = if ($hasRect) { [int][Math]::Round($rect.Height) } else { $null }
      }) | Out-Null
    } catch {
      continue
    }
  }
} catch {
}

@{ items = @($items) } | ConvertTo-Json -Depth 4 -Compress
"#;

const SELECTED_TEXT_SCRIPT: &str = r#"
Add-Type -AssemblyName UIAutomationClient;
Add-Type -AssemblyName UIAutomationTypes;

function Get-SelectedText($element) {
  if ($null -eq $element) { return "" }

  $textPatternObj = $null
  if ($element.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$textPatternObj)) {
    try {
      $textPattern = [System.Windows.Automation.TextPattern]$textPatternObj
      $ranges = $textPattern.GetSelection()
      if ($ranges -and $ranges.Length -gt 0) {
        $parts = New-Object System.Collections.Generic.List[string]
        foreach ($range in $ranges) {
          try {
            $text = $range.GetText(-1)
            if (-not [string]::IsNullOrWhiteSpace($text)) {
              $parts.Add($text) | Out-Null
            }
          } catch {
          }
        }
        if ($parts.Count -gt 0) {
          return ($parts -join "`n")
        }
      }
    } catch {
    }
  }

  return ""
}

$walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$current = [System.Windows.Automation.AutomationElement]::FocusedElement
$text = ""

for ($depth = 0; $depth -lt 6 -and $null -ne $current; $depth++) {
  $text = Get-SelectedText $current
  if (-not [string]::IsNullOrWhiteSpace($text)) { break }
  try {
    $current = $walker.GetParent($current)
  } catch {
    $current = $null
  }
}

@{ text = $text } | ConvertTo-Json -Depth 3 -Compress
"#;

const ELEMENT_CENTER_AT_POINT_SCRIPT_TEMPLATE: &str = r#"
Add-Type -AssemblyName UIAutomationClient;
Add-Type -AssemblyName UIAutomationTypes;
Add-Type -AssemblyName WindowsBase;

$X = {x};
$Y = {y};

try {
  $point = New-Object System.Windows.Point -ArgumentList $X, $Y
  $element = [System.Windows.Automation.AutomationElement]::FromPoint($point)
  if ($null -eq $element) {
    @{ found = $false } | ConvertTo-Json -Depth 3 -Compress
    return
  }

  $current = $element.Current
  if ($current.IsOffscreen) {
    @{ found = $false } | ConvertTo-Json -Depth 3 -Compress
    return
  }

  $rect = $current.BoundingRectangle
  if ($rect.Width -le 0 -or $rect.Height -le 0) {
    @{ found = $false } | ConvertTo-Json -Depth 3 -Compress
    return
  }

  @{
    found = $true
    x = [int][Math]::Round($rect.Left)
    y = [int][Math]::Round($rect.Top)
    width = [int][Math]::Round($rect.Width)
    height = [int][Math]::Round($rect.Height)
  } | ConvertTo-Json -Depth 3 -Compress
} catch {
  @{ found = $false } | ConvertTo-Json -Depth 3 -Compress
}
"#;

#[derive(Debug, Deserialize)]
struct BrowserSnapshotPayload {
    #[serde(default)]
    items: Vec<RawUiSnapshotElement>,
}

#[derive(Debug, Deserialize)]
struct RawUiSnapshotElement {
    role: String,
    name: String,
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SelectedTextPayload {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ElementAtPointPayload {
    #[serde(default)]
    found: bool,
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
}

pub(super) fn frontmost_app_name() -> Result<Option<String>> {
    let raw = run_powershell(FOREGROUND_WINDOW_SCRIPT)?;
    let app_name = raw
        .split(" :: ")
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(app_name)
}

pub(super) fn ui_automation_probe() -> Result<UiAutomationProbe> {
    let raw = run_powershell(FOREGROUND_WINDOW_SCRIPT)?;
    let mut parts = raw.splitn(2, " :: ");
    let app_name = parts.next().unwrap_or_default().trim().to_string();
    let window_title = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(UiAutomationProbe {
        app_name,
        window_title,
    })
}

pub(super) fn browser_snapshot() -> Result<Vec<UiSnapshotElement>> {
    let raw = run_powershell(BROWSER_SNAPSHOT_SCRIPT)
        .context("windows browser snapshot powershell failed")?;
    parse_browser_snapshot_payload(&raw)
}

pub(super) fn selected_text() -> Result<Option<String>> {
    let raw =
        run_powershell(SELECTED_TEXT_SCRIPT).context("windows selected text powershell failed")?;
    parse_selected_text_payload(&raw)
}

pub(super) fn ui_element_center_at(x: i32, y: i32) -> Result<Option<(i32, i32)>> {
    let script = ELEMENT_CENTER_AT_POINT_SCRIPT_TEMPLATE
        .replace("{x}", &x.to_string())
        .replace("{y}", &y.to_string());
    let raw = run_powershell(&script).context("windows ui hit-test powershell failed")?;
    parse_element_center_payload(&raw)
}

fn parse_browser_snapshot_payload(raw: &str) -> Result<Vec<UiSnapshotElement>> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    let payload: BrowserSnapshotPayload =
        serde_json::from_str(raw).context("invalid windows browser snapshot json")?;

    Ok(payload
        .items
        .into_iter()
        .map(|item| {
            let bounds = normalize_bounds(&item);
            UiSnapshotElement {
                role: item.role,
                name: item.name,
                bounds,
            }
        })
        .collect())
}

fn parse_selected_text_payload(raw: &str) -> Result<Option<String>> {
    if raw.trim().is_empty() {
        return Ok(None);
    }

    let payload: SelectedTextPayload =
        serde_json::from_str(raw).context("invalid windows selected text json")?;
    Ok(payload
        .text
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty()))
}

fn parse_element_center_payload(raw: &str) -> Result<Option<(i32, i32)>> {
    if raw.trim().is_empty() {
        return Ok(None);
    }

    let payload: ElementAtPointPayload =
        serde_json::from_str(raw).context("invalid windows ui hit-test json")?;
    if !payload.found {
        return Ok(None);
    }

    let (Some(x), Some(y), Some(width), Some(height)) =
        (payload.x, payload.y, payload.width, payload.height)
    else {
        return Ok(None);
    };

    let width = width.round() as i32;
    let height = height.round() as i32;
    if width <= 0 || height <= 0 {
        return Ok(None);
    }

    Ok(Some((
        x.round() as i32 + (width / 2),
        y.round() as i32 + (height / 2),
    )))
}

fn normalize_bounds(item: &RawUiSnapshotElement) -> Option<UiBounds> {
    let (Some(x), Some(y), Some(width), Some(height)) = (item.x, item.y, item.width, item.height)
    else {
        return None;
    };

    let width = width.round() as i32;
    let height = height.round() as i32;
    if width <= 0 || height <= 0 {
        return None;
    }

    Some(UiBounds {
        x: x.round() as i32,
        y: y.round() as i32,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_browser_snapshot_payload() {
        let raw = r#"{"items":[{"role":"Button","name":"Send","x":100,"y":200,"width":80,"height":24},{"role":"Edit","name":"Search","x":null,"y":null,"width":null,"height":null}]}"#;
        let elements = parse_browser_snapshot_payload(raw).expect("payload should parse");

        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].role, "Button");
        assert_eq!(elements[0].name, "Send");
        let bounds = elements[0].bounds.as_ref().expect("bounds should exist");
        assert_eq!(bounds.x, 100);
        assert_eq!(bounds.y, 200);
        assert_eq!(bounds.width, 80);
        assert_eq!(bounds.height, 24);
        assert!(elements[1].bounds.is_none());
    }

    #[test]
    fn empty_windows_snapshot_payload_returns_empty_elements() {
        let elements =
            parse_browser_snapshot_payload(r#"{"items":[]}"#).expect("payload should parse");
        assert!(elements.is_empty());
    }

    #[test]
    fn parses_selected_text_payload() {
        let text = parse_selected_text_payload(r#"{"text":"  hello world  "}"#)
            .expect("payload should parse");
        assert_eq!(text.as_deref(), Some("hello world"));
    }

    #[test]
    fn empty_selected_text_payload_returns_none() {
        let text = parse_selected_text_payload(r#"{"text":""}"#).expect("payload should parse");
        assert!(text.is_none());
    }

    #[test]
    fn parses_element_center_payload() {
        let center = parse_element_center_payload(
            r#"{"found":true,"x":100,"y":200,"width":81,"height":25}"#,
        )
        .expect("payload should parse");
        assert_eq!(center, Some((140, 212)));
    }

    #[test]
    fn missing_element_center_payload_returns_none() {
        let center =
            parse_element_center_payload(r#"{"found":false}"#).expect("payload should parse");
        assert!(center.is_none());
    }
}
