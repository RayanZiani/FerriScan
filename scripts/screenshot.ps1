# Captures the FerriScan window itself (not the screen, so other windows cannot leak in).
# Usage: scripts/screenshot.ps1 [-Out path.png]
param([string]$Out = "$PSScriptRoot\..\shot.png")
Add-Type -AssemblyName System.Drawing

$proc = Get-Process ferriscan-app -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { Write-Output "no window"; exit 1 }

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win {
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
$r = New-Object Win+RECT
[Win]::GetClientRect($proc.MainWindowHandle, [ref]$r) | Out-Null
$w = $r.Right; $h = $r.Bottom
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$dc = $g.GetHdc()
# 1 = client area only, 2 = PW_RENDERFULLCONTENT (needed for GPU-composited windows)
[Win]::PrintWindow($proc.MainWindowHandle, $dc, 3) | Out-Null
$g.ReleaseHdc($dc)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "saved $Out (${w}x${h})"
