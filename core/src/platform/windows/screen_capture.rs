use anyhow::Result;

use super::run_powershell;

pub(super) fn screen_capture_probe() -> Result<String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms;
Add-Type -AssemblyName System.Drawing;
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds;
$bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height;
$gfx = [System.Drawing.Graphics]::FromImage($bmp);
$gfx.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size);
$path = Join-Path $env:TEMP ("allvia-screen-probe-" + [Guid]::NewGuid().ToString() + ".png");
$bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png);
$gfx.Dispose();
$bmp.Dispose();
Remove-Item $path -Force -ErrorAction SilentlyContinue;
Write-Output "ok"
"#;
    run_powershell(script)
}
