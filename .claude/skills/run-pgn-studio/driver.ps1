#Requires -Version 7.0
<#
.SYNOPSIS
  Launches and drives the built PGN Studio desktop app (Tauri + WebView2)
  on Windows, so an agent can observe and interact with the real running
  application rather than only its test suite.

.DESCRIPTION
  Actions:
    smoke      launch, dump the UI tree, screenshot, terminate  (default)
    text       launch, print every named element in the UI, terminate
    screenshot launch, capture the window to a PNG, terminate
    flow       launch, type into the Destination fields, re-read the UI to
               prove React responded, screenshot, terminate

  Everything is per-invocation: the app is launched fresh and always
  terminated, including on failure. Nothing is left running.

.NOTES
  Paths are resolved from this script's location, so it works from any cwd.
#>
[CmdletBinding()]
param(
    [ValidateSet("smoke", "text", "screenshot", "flow")]
    [string]$Action = "smoke",

    # Where screenshots land. Defaults to the repo's target dir.
    [string]$OutDir,

    # Override the app binary (e.g. to drive a debug build).
    [string]$Exe,

    [int]$LaunchTimeoutSeconds = 60
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# .claude/skills/run-pgn-studio/driver.ps1 -> repo root is three up.
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
if (-not $Exe) { $Exe = Join-Path $RepoRoot "src-tauri\target\release\pgn-studio.exe" }
if (-not $OutDir) { $OutDir = Join-Path $RepoRoot "src-tauri\target\release" }

if (-not (Test-Path $Exe)) {
    throw "App binary not found at $Exe. Build it first: npm run tauri build (see SKILL.md)."
}

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class PgnStudioWin32 {
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

function Start-App {
    Write-Host "launching: $Exe"
    $p = Start-Process -FilePath $Exe -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds($LaunchTimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 500
        $p.Refresh()
        if ($p.HasExited) { throw "App exited during startup with code $($p.ExitCode)." }
        if ($p.MainWindowHandle -ne [IntPtr]::Zero) { break }
    }
    $p.Refresh()
    if ($p.MainWindowHandle -eq [IntPtr]::Zero) {
        throw "No window appeared within $LaunchTimeoutSeconds s."
    }
    Write-Host "  pid $($p.Id), window '$($p.MainWindowTitle)'"
    # SW_RESTORE, then try to front it. SetForegroundWindow is frequently
    # refused for a process launched from the background - that is fine and
    # expected here, because PrintWindow does not need the window in front.
    [void][PgnStudioWin32]::ShowWindow($p.MainWindowHandle, 9)
    [void][PgnStudioWin32]::SetForegroundWindow($p.MainWindowHandle)
    return $p
}

function Get-UiElements {
    param([Parameter(Mandatory)][IntPtr]$WindowHandle)

    $root = [System.Windows.Automation.AutomationElement]::FromHandle($WindowHandle)
    $cond = [System.Windows.Automation.Condition]::TrueCondition

    # THE non-obvious part. Chromium (and therefore WebView2) builds its
    # accessibility tree lazily, only after an AT client asks for it. The
    # FIRST FindAll returns just the two host panes; it is also the nudge
    # that triggers construction. Poll until the tree actually populates -
    # a single query will make you conclude, wrongly, that the DOM is
    # invisible to UI Automation.
    for ($i = 1; $i -le 10; $i++) {
        $all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)
        if ($all.Count -gt 25) { return $all }
        Start-Sleep -Seconds 2
    }
    Write-Warning "UI tree never grew past $($all.Count) elements - the webview may not have rendered."
    return $all
}

function Write-UiText {
    param([Parameter(Mandatory)]$Elements)
    $n = 0
    foreach ($el in $Elements) {
        $name = $el.Current.Name
        if ($name -and $name.Trim()) {
            $type = $el.Current.ControlType.ProgrammaticName -replace '^ControlType\.', ''
            Write-Host ("  {0,-12} {1}" -f $type, $name)
            $n++
        }
    }
    Write-Host "  ($n named elements)"
}

function Find-Element {
    param([Parameter(Mandatory)]$Elements, [Parameter(Mandatory)][string]$NameLike, [string]$Type)
    foreach ($el in $Elements) {
        $name = $el.Current.Name
        if (-not $name) { continue }
        if ($name -notlike $NameLike) { continue }
        if ($Type -and ($el.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '') -ne $Type) { continue }
        return $el
    }
    return $null
}

function Set-EditValue {
    param([Parameter(Mandatory)]$Element, [Parameter(Mandatory)][string]$Text)
    # ValuePattern writes straight into the input and fires the events React
    # listens for; SendKeys would need the window focused and in front.
    $pattern = $Element.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    $pattern.SetValue($Text)
}

function Save-WindowPng {
    param([Parameter(Mandatory)]$Process, [Parameter(Mandatory)][string]$Path)

    $h = $Process.MainWindowHandle
    $r = New-Object PgnStudioWin32+RECT
    [void][PgnStudioWin32]::GetWindowRect($h, [ref]$r)
    $w = $r.Right - $r.Left
    $ht = $r.Bottom - $r.Top

    $bmp = New-Object System.Drawing.Bitmap $w, $ht
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    # Flag 2 = PW_RENDERFULLCONTENT. Without it, a WebView2 surface captures
    # as a blank rectangle, because the content is composited by
    # DirectComposition rather than drawn into the window's own DC.
    $ok = [PgnStudioWin32]::PrintWindow($h, $hdc, 2)
    $g.ReleaseHdc($hdc)
    $g.Dispose()
    if (-not $ok) { Write-Warning "PrintWindow reported failure." }

    # Blank-frame guard: a capture that is one flat colour is a failed
    # capture, not a working app, and must not be reported as success.
    $seen = @{}
    for ($x = 5; $x -lt $w - 5; $x += 37) {
        for ($y = 5; $y -lt $ht - 5; $y += 29) { $seen[$bmp.GetPixel($x, $y).ToArgb()] = $true }
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()

    Write-Host "  screenshot: $Path  (${w}x${ht}, $($seen.Count) distinct sampled colours)"
    if ($seen.Count -lt 3) { throw "Screenshot looks blank ($($seen.Count) colours) - the window did not render." }
}

function Stop-App {
    param($Process)
    if ($Process -and -not $Process.HasExited) {
        $Process.Kill()
        $Process.WaitForExit(10000) | Out-Null
    }
    $leftover = Get-Process -Name "pgn-studio" -ErrorAction SilentlyContinue
    if ($leftover) { $leftover | Stop-Process -Force -ErrorAction SilentlyContinue }
}

# ---------------------------------------------------------------- main
$proc = $null
try {
    $proc = Start-App

    switch ($Action) {
        "screenshot" {
            Start-Sleep -Seconds 6
            Save-WindowPng -Process $proc -Path (Join-Path $OutDir "pgn-studio-window.png")
        }
        "text" {
            Write-Host "--- UI tree ---"
            Write-UiText -Elements (Get-UiElements -WindowHandle $proc.MainWindowHandle)
        }
        "smoke" {
            $els = Get-UiElements -WindowHandle $proc.MainWindowHandle
            Write-Host "--- UI tree ---"
            Write-UiText -Elements $els
            foreach ($expected in @("Add Files", "Add Folder", "Next: Operations")) {
                if (-not (Find-Element -Elements $els -NameLike $expected -Type "Button")) {
                    throw "Expected button '$expected' not present - the Files screen did not render."
                }
            }
            Write-Host "  OK: Files screen rendered with its expected controls"
            Save-WindowPng -Process $proc -Path (Join-Path $OutDir "pgn-studio-window.png")
        }
        "flow" {
            $els = Get-UiElements -WindowHandle $proc.MainWindowHandle
            $base = Find-Element -Elements $els -NameLike "Base filename*" -Type "Edit"
            if (-not $base) { throw "Base filename field not found." }
            Set-EditValue -Element $base -Text "driver-smoke"
            Start-Sleep -Seconds 2

            # Re-read the tree: if React did not process the input, the new
            # value will not be here, and this is a live app only if it is.
            $after = Get-UiElements -WindowHandle $proc.MainWindowHandle
            $edit = Find-Element -Elements $after -NameLike "Base filename*" -Type "Edit"
            $vp = $edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
            $value = $vp.Current.Value
            Write-Host "  Base filename now reads: '$value'"
            if ($value -ne "driver-smoke") { throw "Typed value did not stick (got '$value') - the UI is not live." }
            Write-Host "  OK: typed into the running app and the DOM reflected it"
            Save-WindowPng -Process $proc -Path (Join-Path $OutDir "pgn-studio-flow.png")
        }
    }
    Write-Host "RESULT: PASS"
}
finally {
    Stop-App -Process $proc
    Write-Host "terminated; pgn-studio still running: $((Get-Process -Name 'pgn-studio' -ErrorAction SilentlyContinue) -ne $null)"
}
