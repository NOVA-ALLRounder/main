param(
  [string]$SourceText
)

$ErrorActionPreference = "Stop"

function Get-ExpectedTokensHeuristic {
  param(
    [Parameter(Mandatory = $true)]
    [string]$InputText
  )

  $seen = New-Object System.Collections.Specialized.OrderedDictionary

  function Add-Token([string]$token) {
    if (-not $token) { return }
    $value = $token.Trim()
    if ($value.Length -lt 3) { return }
    if ($value.Length -gt 120) { return }
    if (-not $seen.Contains($value)) {
      $seen.Add($value, $true)
    }
  }

  foreach ($m in [regex]::Matches($InputText, '"([^"]{3,})"')) {
    Add-Token $m.Groups[1].Value
  }
  foreach ($m in [regex]::Matches($InputText, "'([^']{3,})'")) {
    Add-Token $m.Groups[1].Value
  }
  foreach ($m in [regex]::Matches($InputText, '`([^`]{3,})`')) {
    Add-Token $m.Groups[1].Value
  }

  foreach ($m in [regex]::Matches($InputText, '([A-Za-z0-9_ \-]{1,24})\s*[:=]\s*([A-Za-z0-9_./@# \-]{3,80})')) {
    Add-Token $m.Groups[2].Value
    Add-Token ("{0}: {1}" -f $m.Groups[1].Value.Trim(), $m.Groups[2].Value.Trim())
  }

  foreach ($line in ($InputText -split '\r?\n')) {
    if ($line -match '^\s*(?:[-*]|\d+[.)])\s*(.+)$') {
      Add-Token $matches[1]
    }
  }

  foreach ($k in $seen.Keys) {
    $k
  }
}

if ($PSBoundParameters.ContainsKey("SourceText")) {
  Get-ExpectedTokensHeuristic -InputText $SourceText
}
