# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the pgn-extract sidecar for x86_64-pc-windows-msvc from the
# pinned sources in engine-src/upstream.lock:
#
#   1. Validate engine-src/upstream.lock (refuse placeholders).
#   2. Fetch pgn-extract and TRE into a gitignored cache dir
#      (engine-src/.build/), verifying `git rev-parse HEAD` AND
#      `HEAD^{tree}` against the lock; try upstream first, fall back to
#      the Nyvetra mirror; hard-fail on any mismatch.
#   3. Apply engine-src/patches/*.patch (lexical order) to pgn-extract.
#      There are currently none - see engine-src/patches/README.md.
#   4. Compile TRE v0.9.0 as a static library (engine-src/patches/tre-msvc/
#      README.md explains why win32/tre.vcxproj isn't used directly and
#      records the exact fallback recipe used here), then compile
#      pgn-extract's own unmodified sources against it, embedding the
#      UTF-8 manifest (engine-src/manifest/pgn-extract.manifest) so the
#      engine can open non-ASCII (e.g. Bengali) paths on Windows. Source
#      files are fed to cl.exe/lib.exe/link.exe in a fixed ordinal sort
#      (Get-OrdinalSorted, lib/engine-common.ps1), and every compile,
#      archive, and link invocation carries /Brepro (upstream.lock's
#      engineFlags/treFlags/linkFlags) - see engine-src/README.md
#      "Reproducibility" for why the build is byte-reproducible and how
#      to verify it yourself.
#   5. Smoke-check `--version` (argument array, never a shell) against
#      engine-src/upstream.lock's pinned version string.
#   6. Install to src-tauri/binaries/pgn-extract-<triple>.exe.
#   7. Write src-tauri/binaries/build-info-<triple>.json and
#      src-tauri/binaries/checksums.json.
#
# Requires: PowerShell 7+ (pwsh), git, and VS 2022 Build Tools with the
# "VC.Tools.x86.x64" component (MSVC cl.exe/link.exe/lib.exe), located
# via vswhere.exe. Nothing else - no MSYS2/MinGW/make is needed to BUILD
# the engine (make is only used by scripts/verify-engine.ps1's upstream
# test-suite layer).
#
# Usage:
#   pwsh ./scripts/build-pgn-extract.ps1
#   pwsh ./scripts/build-pgn-extract.ps1 -CacheDir D:/tmp/engine-cache
#
# Exit code 0 = binary built, smoke-checked, and installed.
# Non-zero = see the error message; nothing partially-built is left
# installed at src-tauri/binaries/ (install only happens after the
# smoke check passes).

#Requires -Version 7.0
[CmdletBinding()]
param(
    # Where pinned sources are fetched/built. Gitignored; safe to delete
    # between runs (a deleted cache is simply re-cloned).
    [string]$CacheDir,

    # Override the detected target triple (default: `rustc --print
    # host-tuple`, falling back to x86_64-pc-windows-msvc with a
    # warning if rustc is not on PATH).
    [string]$Triple,

    # Re-clone pgn-extract/TRE even if a checkout already exists in the
    # cache at the correct pinned commit.
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# --------------------------------------------------------------------
# Paths
# --------------------------------------------------------------------
$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not $CacheDir) { $CacheDir = Join-Path $RepoRoot "engine-src\.build" }
$LockPath = Join-Path $RepoRoot "engine-src\upstream.lock"
$ManifestPath = Join-Path $RepoRoot "engine-src\manifest\pgn-extract.manifest"
$PatchesDir = Join-Path $RepoRoot "engine-src\patches"
$BinariesDir = Join-Path $RepoRoot "src-tauri\binaries"

# Shared with scripts/verify-engine.ps1: Write-Section, placeholder/commit
# validation, PATH refresh, Invoke-Native, Get-PinnedCheckout, Get-HostTriple.
. (Join-Path $PSScriptRoot "lib\engine-common.ps1")

# --------------------------------------------------------------------
# Build-specific helpers
# --------------------------------------------------------------------

# Imports MSVC's environment (INCLUDE/LIB/PATH for cl.exe, link.exe,
# lib.exe) into *this* PowerShell process by running vcvarsall.bat in a
# child cmd.exe and capturing its resulting `set` output - the standard
# technique for consuming vcvarsall from PowerShell instead of cmd.exe.
function Import-VcVarsEnvironment {
    param([string]$Arch = "x64")

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        throw "vswhere.exe not found at '$vswhere'. Is Visual Studio / VS Build Tools installed?"
    }
    $vsInstallPath = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    if (-not $vsInstallPath) {
        throw "vswhere found no VS installation with the Microsoft.VisualStudio.Component.VC.Tools.x86.x64 component installed."
    }
    $vcvarsall = Join-Path $vsInstallPath "VC\Auxiliary\Build\vcvarsall.bat"
    if (-not (Test-Path $vcvarsall)) {
        throw "vcvarsall.bat not found at '$vcvarsall'."
    }

    Write-Host "  VS install : $vsInstallPath"
    Write-Host "  vcvarsall  : $vcvarsall $Arch"

    $envDump = [System.IO.Path]::GetTempFileName()
    try {
        cmd.exe /c "`"$vcvarsall`" $Arch >nul 2>&1 && set" > $envDump
        if ($LASTEXITCODE -ne 0) {
            throw "vcvarsall.bat $Arch exited with code $LASTEXITCODE"
        }
        $importedCount = 0
        foreach ($line in Get-Content -LiteralPath $envDump) {
            if ($line -match '^([^=]+)=(.*)$') {
                [System.Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], "Process")
                $importedCount++
            }
        }
        Write-Host "  imported $importedCount environment variables from vcvarsall"
    }
    finally {
        Remove-Item -LiteralPath $envDump -ErrorAction SilentlyContinue
    }

    if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
        throw "cl.exe still not on PATH after importing vcvarsall environment."
    }
    # cl.exe with no args prints its version banner AND usage text,
    # interleaved across stdout/stderr - merge ordering via 2>&1 is not
    # guaranteed, so search for the known banner line by pattern rather
    # than assuming it is line 0.
    $bannerLines = (& cl.exe 2>&1 | Out-String) -split "`r?`n"
    $bannerLine = $bannerLines | Where-Object { $_ -match '^Microsoft.*Compiler Version' } | Select-Object -First 1
    if (-not $bannerLine) {
        $bannerLine = ($bannerLines | Where-Object { $_.Trim() -ne "" } | Select-Object -First 1)
    }
    Write-Host "  compiler   : $bannerLine"
    return $bannerLine
}

function Invoke-Patches {
    param([Parameter(Mandatory)][string]$PatchDir, [Parameter(Mandatory)][string]$RepoDir, [Parameter(Mandatory)][string]$Label)
    if (-not (Test-Path $PatchDir)) {
        Write-Host "  $PatchDir does not exist - nothing to apply to $Label"
        return
    }
    $patches = Get-ChildItem -LiteralPath $PatchDir -Filter "*.patch" -File -ErrorAction SilentlyContinue | Sort-Object Name
    if (-not $patches -or $patches.Count -eq 0) {
        Write-Host "  no *.patch files in $PatchDir for $Label (expected - see engine-src/patches/README.md)"
        return
    }
    foreach ($p in $patches) {
        Write-Host "  applying $($p.Name) to $Label"
        Invoke-Native -Exe "git" -Arguments @("-C", $RepoDir, "apply", "--whitespace=nowarn", $p.FullName) -FailureContext "git apply $($p.Name)"
    }
}

# Compiles TRE's lib/*.c as a static library and stages an "installed"
# header layout. See engine-src/patches/tre-msvc/README.md for the full
# rationale of every step here.
function Build-TreStaticLib {
    param(
        [Parameter(Mandatory)][string]$TreSrc,
        [Parameter(Mandatory)][string]$WorkDir,
        [Parameter(Mandatory)][string]$InstallDir,
        [Parameter(Mandatory)][string[]]$Flags,
        [string[]]$LinkFlags = @()
    )
    Write-Section "Compiling TRE v0.9.0 as a static library"

    $objDir = Join-Path $WorkDir "tre-obj"
    New-Item -ItemType Directory -Force -Path $objDir | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "include\tre") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "lib") | Out-Null

    # Sorted (not raw Get-ChildItem order) so the exact sequence of
    # source files hitting cl.exe's command line is reproducible - see
    # Get-OrdinalSorted's own comment in lib/engine-common.ps1.
    $libSrcs = Get-OrdinalSorted -Items (Get-ChildItem -LiteralPath (Join-Path $TreSrc "lib") -Filter "*.c" | Select-Object -ExpandProperty FullName)
    if (-not $libSrcs -or $libSrcs.Count -eq 0) {
        throw "No .c files found under $TreSrc\lib - TRE checkout looks wrong."
    }
    Write-Host "  compiling $($libSrcs.Count) TRE lib/*.c files"

    $prevLocation = Get-Location
    Set-Location -LiteralPath $objDir
    try {
        # /c = compile only, do not link (these are library object files
        # with no entry point; the flags recorded in upstream.lock are
        # the toolchain-choice flags, not this structural switch).
        # /Brepro (part of $Flags - see upstream.lock's treFlags) makes
        # cl.exe's own output byte-reproducible AND neutralizes
        # __DATE__/__TIME__/__TIMESTAMP__ to a fixed placeholder instead
        # of the real compile time - see engine-src/README.md
        # "Reproducibility". TRE itself doesn't use those macros (no
        # source edits either way), but the flag is applied uniformly to
        # every compile in this build.
        $clArgs = @("/c") + @($Flags) + @(
            "/DHAVE_CONFIG_H",
            "/I", (Join-Path $TreSrc "win32"),
            "/I", (Join-Path $TreSrc "lib")
        ) + $libSrcs
        & cl.exe @clArgs
        if ($LASTEXITCODE -ne 0) { throw "TRE compilation failed (cl.exe exit $LASTEXITCODE)" }

        $objs = Get-OrdinalSorted -Items (Get-ChildItem -Filter "*.obj" | Select-Object -ExpandProperty FullName)
        if (-not $objs -or $objs.Count -ne $libSrcs.Count) {
            throw "Expected $($libSrcs.Count) TRE .obj files, found $($objs.Count)."
        }
        $treLib = Join-Path $InstallDir "lib\tre.lib"
        # /Brepro on lib.exe (upstream.lock's linkFlags) - same flag,
        # librarian-side: makes the archive's own repro marker
        # content-derived instead of wall-clock-derived. Built as ONE
        # array via `+` before the single @libArgs splat below (not
        # multiple separate splats on the call line) - see
        # engine-src/README.md "Reproducibility" for why: PowerShell can
        # silently collapse a single-element array flowing through an
        # `if/else` expression into a bare string, and splatting *that*
        # directly and separately from other splats on the same call
        # mis-parses the argument list. Concatenating into one array
        # first sidesteps the issue regardless of how $LinkFlags was
        # produced upstream.
        $libArgs = @("/NOLOGO") + @($LinkFlags) + @("/OUT:$treLib") + $objs
        & lib.exe @libArgs
        if ($LASTEXITCODE -ne 0) { throw "TRE archive (lib.exe) failed (exit $LASTEXITCODE)" }
        Write-Host "  archived: $treLib"
    }
    finally {
        Set-Location $prevLocation
    }

    # Stage TRE's own unmodified files into a synthesized install
    # prefix. These are verbatim copies, not edits - see
    # engine-src/patches/tre-msvc/README.md.
    Copy-Item -LiteralPath (Join-Path $TreSrc "local_includes\regex.h") -Destination (Join-Path $InstallDir "include\regex.h") -Force
    Copy-Item -LiteralPath (Join-Path $TreSrc "local_includes\tre.h") -Destination (Join-Path $InstallDir "include\tre\tre.h") -Force
    Copy-Item -LiteralPath (Join-Path $TreSrc "win32\tre-config.h") -Destination (Join-Path $InstallDir "include\tre\tre-config.h") -Force
    Write-Host "  staged install headers under $InstallDir\include"
}

# Compiles pgn-extract's own unmodified *.c files against the staged
# TRE install layout and links pgn-extract.exe with the UTF-8 manifest
# embedded.
function Build-PgnExtractExe {
    param(
        [Parameter(Mandatory)][string]$PgnSrc,
        [Parameter(Mandatory)][string]$WorkDir,
        [Parameter(Mandatory)][string]$TreInstallDir,
        [Parameter(Mandatory)][string]$ManifestFile,
        [Parameter(Mandatory)][string]$OutExePath,
        [Parameter(Mandatory)][string[]]$Flags,
        [string[]]$LinkFlags = @()
    )
    Write-Section "Compiling pgn-extract against TRE"

    $objDir = Join-Path $WorkDir "pgn-obj"
    New-Item -ItemType Directory -Force -Path $objDir | Out-Null

    # Sorted for the same reproducibility reason as TRE's $libSrcs above.
    $srcs = Get-OrdinalSorted -Items (Get-ChildItem -LiteralPath $PgnSrc -Filter "*.c" | Select-Object -ExpandProperty FullName)
    if (-not $srcs -or $srcs.Count -eq 0) {
        throw "No .c files found in $PgnSrc - pgn-extract checkout looks wrong."
    }
    Write-Host "  compiling $($srcs.Count) pgn-extract *.c files (zero source edits)"

    $prevLocation = Get-Location
    Set-Location -LiteralPath $objDir
    try {
        # /c = compile only, do not link (linking happens explicitly
        # below via link.exe, so the manifest can be embedded). /Brepro
        # (part of $Flags - upstream.lock's engineFlags) also neutralizes
        # argsfile.c's `__DATE__` use in the --help banner to a fixed
        # placeholder instead of the real compile date - confirmed by
        # `pgn-extract --help` printing "(1)" where the date would
        # otherwise appear, and confirmed NOT to affect
        # `--version`(argsfile.c's other, date-free CURRENT_VERSION
        # site, the one the smoke check below and verify-engine.ps1
        # Layer 1 check) or upstream's test-h target (test/Makefile's
        # `-$(PGN_EXTRACT) -h` recipe line has no oracle-diff on the
        # output). See engine-src/README.md "Reproducibility".
        $clArgs = @("/c") + @($Flags) + @(
            "/I", (Join-Path $TreInstallDir "include"),
            "/I", (Join-Path $TreInstallDir "include\tre")
        ) + $srcs
        & cl.exe @clArgs
        if ($LASTEXITCODE -ne 0) { throw "pgn-extract compilation failed (cl.exe exit $LASTEXITCODE)" }

        $objs = Get-OrdinalSorted -Items (Get-ChildItem -Filter "*.obj" | Select-Object -ExpandProperty FullName)
        if (-not $objs -or $objs.Count -ne $srcs.Count) {
            throw "Expected $($srcs.Count) pgn-extract .obj files, found $($objs.Count)."
        }

        if (-not (Test-Path $ManifestFile)) {
            throw "UTF-8 manifest not found at $ManifestFile"
        }
        $treLib = Join-Path $TreInstallDir "lib\tre.lib"
        if (-not (Test-Path $treLib)) {
            throw "tre.lib not found at $treLib - TRE build step must run first."
        }

        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutExePath) | Out-Null
        # /Brepro on link.exe (upstream.lock's linkFlags) is what actually
        # matters for the shipped artifact: without it, link.exe stamps
        # the PE header's TimeDateStamp field with the real build
        # wall-clock time on every run, so the installed .exe's SHA-256
        # would never repeat even with byte-identical inputs. With it,
        # that field (and the embedded manifest resource's directory
        # timestamps) become content-hash-derived instead - see
        # engine-src/README.md "Reproducibility" for the before/after
        # evidence. Built as one array via `+` before a single splat, for
        # the same PowerShell-splatting reason noted in
        # Build-TreStaticLib above.
        $linkArgs = @("/NOLOGO") + @($LinkFlags) + @(
            "/MANIFEST:EMBED",
            "/MANIFESTINPUT:$ManifestFile",
            "/OUT:$OutExePath"
        ) + $objs + @($treLib)
        & link.exe @linkArgs
        if ($LASTEXITCODE -ne 0) { throw "pgn-extract link failed (link.exe exit $LASTEXITCODE)" }
        Write-Host "  linked: $OutExePath"
    }
    finally {
        Set-Location $prevLocation
    }
}

# ========================================================================
# Main
# ========================================================================
Write-Section "PGN Studio engine build (Windows / MSVC)"
Write-Host "Repo root : $RepoRoot"
Write-Host "Cache dir : $CacheDir"

Update-PathFromRegistry

if (-not (Test-Path $LockPath)) { throw "Could not find $LockPath" }
$lock = Get-Content -Raw -LiteralPath $LockPath | ConvertFrom-Json

# ---- 1. Validate the lock ---------------------------------------------
Write-Section "Validating engine-src/upstream.lock"
foreach ($f in @(
        @{ Value = $lock.engine.repository; Name = "engine.repository" },
        @{ Value = $lock.engine.commit; Name = "engine.commit" },
        @{ Value = $lock.engine.version; Name = "engine.version" },
        @{ Value = $lock.engine.license; Name = "engine.license" },
        @{ Value = $lock.regex.windows.repository; Name = "regex.windows.repository" },
        @{ Value = $lock.regex.windows.commit; Name = "regex.windows.commit" },
        @{ Value = $lock.regex.windows.license; Name = "regex.windows.license" }
    )) {
    Test-NotPlaceholder -Value $f.Value -FieldName $f.Name
}
Assert-CommitShape -Value $lock.engine.commit -FieldName "engine.commit"
Assert-CommitShape -Value $lock.regex.windows.commit -FieldName "regex.windows.commit"
Write-Host "  OK: pgn-extract $($lock.engine.version) @ $($lock.engine.commit)"
Write-Host "  OK: TRE $($lock.regex.windows.tag) @ $($lock.regex.windows.commit)"

# ---- 2. MSVC environment -----------------------------------------------
Write-Section "Locating MSVC toolchain"
$compilerBanner = Import-VcVarsEnvironment -Arch "x64"

# ---- 3. Fetch pinned sources --------------------------------------------
$pgnSrcDir = Join-Path $CacheDir "pgn-extract"
$treSrcDir = Join-Path $CacheDir "tre"

$pgnCheckout = Get-PinnedCheckout -Name "pgn-extract" `
    -PrimaryUrl $lock.engine.repository -MirrorUrl $lock.engine.mirror `
    -Commit $lock.engine.commit -ExpectedTree $lock.engine.gitTree `
    -Dest $pgnSrcDir -ForceRefresh:$Force

$treCheckout = Get-PinnedCheckout -Name "TRE" `
    -PrimaryUrl $lock.regex.windows.repository -MirrorUrl $lock.regex.windows.mirror `
    -Commit $lock.regex.windows.commit -ExpectedTree $lock.regex.windows.gitTree `
    -Dest $treSrcDir -ForceRefresh:$Force

# ---- 4. Apply patches (pgn-extract only; TRE gets zero source edits) ---
Write-Section "Applying patches"
Invoke-Patches -PatchDir $PatchesDir -RepoDir $pgnSrcDir -Label "pgn-extract"
Write-Host "  TRE: zero source edits (engine-src/patches/tre-msvc/README.md documents the build-recipe fallback used instead)"

# ---- 5. Compile ----------------------------------------------------------
$workDir = Join-Path $CacheDir "work"
if (Test-Path $workDir) { Remove-Item -LiteralPath $workDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $workDir | Out-Null
$treInstallDir = Join-Path $CacheDir "tre-install"
if (Test-Path $treInstallDir) { Remove-Item -LiteralPath $treInstallDir -Recurse -Force }

$treFlags = $lock.toolchains.'x86_64-pc-windows-msvc'.treFlags
$engineFlags = $lock.toolchains.'x86_64-pc-windows-msvc'.engineFlags
# linkFlags is applied to BOTH lib.exe (archiving tre.lib) and link.exe
# (linking pgn-extract.exe) - see engine-src/README.md "Reproducibility".
# Wrapped in @(...) so a single-flag list (currently just /Brepro) stays
# a real array rather than collapsing to a bare string - see the
# splatting note in Build-TreStaticLib.
$linkFlags = @($lock.toolchains.'x86_64-pc-windows-msvc'.linkFlags)

Build-TreStaticLib -TreSrc $treSrcDir -WorkDir $workDir -InstallDir $treInstallDir -Flags $treFlags -LinkFlags $linkFlags

$builtExePath = Join-Path $workDir "pgn-extract.exe"
Build-PgnExtractExe -PgnSrc $pgnSrcDir -WorkDir $workDir -TreInstallDir $treInstallDir `
    -ManifestFile $ManifestPath -OutExePath $builtExePath -Flags $engineFlags -LinkFlags $linkFlags

# ---- 6. Smoke check --------------------------------------------------------
Write-Section "Smoke check: --version"
$expectedVersion = "pgn-extract $($lock.engine.version)"
# NOTE: pgn-extract writes --version (and most diagnostic output) to
# GlobalState.logfile, which defaults to stderr (main.c: `GlobalState.
# logfile = stderr;`) - NOT stdout. Must merge streams via 2>&1 or the
# capture is silently empty even though the text is visible on-screen
# (PowerShell still passes a native child's inherited stderr straight
# through to the console).
$versionOutput = & $builtExePath "--version" 2>&1
$versionExit = $LASTEXITCODE
$versionText = (($versionOutput | Out-String)).Trim()
Write-Host "  exit code : $versionExit"
Write-Host "  output    : $versionText"
if ($versionExit -ne 0) {
    throw "Smoke check FAILED: --version exited $versionExit (expected 0). Nothing installed to src-tauri/binaries/."
}
if ($versionText -ne $expectedVersion) {
    throw "Smoke check FAILED: --version printed '$versionText', expected exactly '$expectedVersion'. Nothing installed to src-tauri/binaries/."
}
Write-Host "  OK"

# dumpbin /dependents, recorded for the build-info file and printed for
# human review (architecture.md §20.5 'no external runtime required').
$dumpbin = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
$dependents = $null
if ($dumpbin) {
    $dumpOut = & dumpbin.exe /NOLOGO /DEPENDENTS $builtExePath
    $dependents = ($dumpOut | Select-String -Pattern '^\s+\S+\.dll\s*$' | ForEach-Object { $_.ToString().Trim() })
    Write-Host "  DLL dependents: $($dependents -join ', ')"
}

# ---- 7. Install -------------------------------------------------------------
Write-Section "Installing sidecar"
$triple = Get-HostTriple -Override $Triple
$installedName = "pgn-extract-$triple.exe"
$installedPath = Join-Path $BinariesDir $installedName
New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null
Copy-Item -LiteralPath $builtExePath -Destination $installedPath -Force
Write-Host "  installed: $installedPath"

# ---- 8. checksums.json + build-info-<triple>.json --------------------------
Write-Section "Recording checksums and build info"
$hash = (Get-FileHash -LiteralPath $installedPath -Algorithm SHA256).Hash.ToLowerInvariant()
$size = (Get-Item -LiteralPath $installedPath).Length
Write-Host "  sha256 : $hash"
Write-Host "  size   : $size bytes"

$checksumsPath = Join-Path $BinariesDir "checksums.json"
$checksums = [ordered]@{}
if (Test-Path $checksumsPath) {
    # Preserve any existing entries (e.g. a macOS build merged in from a
    # separate CI leg) rather than clobbering them.
    (Get-Content -Raw -LiteralPath $checksumsPath | ConvertFrom-Json -AsHashtable) |
        ForEach-Object { foreach ($k in $_.Keys) { $checksums[$k] = $_[$k] } }
}
$checksums[$installedName] = [ordered]@{
    sha256        = $hash
    sizeBytes     = $size
    engineVersion = $lock.engine.version
    commit        = $lock.engine.commit
}
$checksums["eco.pgn"] = [ordered]@{
    sha256    = $lock.engine.resources.'eco.pgn'.sha256
    sizeBytes = $lock.engine.resources.'eco.pgn'.sizeBytes
}
$checksums | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $checksumsPath -Encoding utf8NoBOM
Write-Host "  wrote: $checksumsPath"

$lockDigest = (Get-FileHash -LiteralPath $LockPath -Algorithm SHA256).Hash.ToLowerInvariant()
$builder = if ($env:GITHUB_ACTIONS -eq "true") {
    "ci:github-actions:run=$($env:GITHUB_RUN_ID):repo=$($env:GITHUB_REPOSITORY)"
}
else {
    "local:$env:USERNAME@$env:COMPUTERNAME"
}
$buildInfo = [ordered]@{
    triple            = $triple
    binary            = $installedName
    sha256            = $hash
    sizeBytes         = $size
    engineVersion     = $lock.engine.version
    engineCommit      = $lock.engine.commit
    treCommit         = $lock.regex.windows.commit
    upstreamLockSha256 = $lockDigest
    compiler          = $compilerBanner
    engineFlags       = $engineFlags
    treFlags          = $treFlags
    linkFlags         = $linkFlags
    manifestEmbedded  = (Split-Path -Leaf $ManifestPath)
    dllDependents     = $dependents
    builtAtUtc        = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    builder           = $builder
}
$buildInfoPath = Join-Path $BinariesDir "build-info-$triple.json"
$buildInfo | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $buildInfoPath -Encoding utf8NoBOM
Write-Host "  wrote: $buildInfoPath"

Write-Section "Build complete"
Write-Host "  $installedPath"
Write-Host "  sha256: $hash"
Write-Host "  Run scripts/verify-engine.ps1 next."
