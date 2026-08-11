# SPDX-License-Identifier: GPL-3.0-or-later
#
# Verifies the built pgn-extract sidecar in four layers (architecture.md
# §16.2, §20.2-§20.4; design-01-engine-build.md §6.3-§6.4):
#
#   0. Pin provenance (optional, network): re-download the pgn-extract
#      source archive named in engine-src/upstream.lock and confirm its
#      SHA-256 still matches. This is the original Phase 0 check,
#      preserved and extended rather than replaced.
#   1. Identity: SHA-256 + size of the installed binary against
#      src-tauri/binaries/checksums.json, plus a `--version` probe.
#      Mirrors the Rust startup self-test's ENGINE_MISSING /
#      ENGINE_TAMPERED / ENGINE_START_FAILED taxonomy (Phase 1 scope) so
#      failures here map 1:1 onto what that code will report.
#   2. Upstream regression suite: runs pgn-extract's own test/Makefile
#      (~76 named targets, most with one or more oracle-diff assertions)
#      against the just-verified binary. Every oracle diff is a failure
#      unless explicitly and individually justified in
#      scripts/verify-skips.json.
#   3. PGN Studio supplemental goldens (fixtures/golden/regex/): the
#      upstream suite has NO `=~` coverage at all, so this layer proves
#      the platform regex engine (TRE on Windows, libc regex on macOS) is
#      actually wired in and functioning, not just linked.
#      Comparison is byte-exact first. If and only if that fails, the
#      case is retried with CRLF/LF normalized - the goldens are stored
#      CRLF and the macOS engine writes LF - and a pass by that route is
#      reported as [PASS~] per case, counted separately in the summary,
#      and recorded as `passedAfterNewlineNormalization` in the JSON
#      report. It is never silent, and byte-exactness is never weakened
#      on a platform where it already holds.
#
# Usage:
#   pwsh ./scripts/verify-engine.ps1
#   pwsh ./scripts/verify-engine.ps1 -SkipPinProvenance   # skip the network-heavy re-download for fast local iteration
#
# Exit code 0 = every layer that ran passed (or was justified-skipped).
# Non-zero = see the summary at the end and verify-report-<triple>.json.

#Requires -Version 7.0
[CmdletBinding()]
param(
    [string]$Triple,
    [string]$CacheDir,
    [switch]$SkipPinProvenance,
    [switch]$SkipUpstreamSuite,
    [switch]$SkipSupplementalGoldens
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
# Segment-at-a-time Join-Path, not embedded "a\b" literals: a backslash is
# an ordinary filename character on macOS, so "engine-src\upstream.lock"
# there names one file called `engine-src\upstream.lock` rather than a path
# - and this script is meant to run under pwsh on all three CI platforms
# (see the $IsWindows note below).
if (-not $CacheDir) { $CacheDir = Join-Path $RepoRoot "engine-src" ".build" }
$LockPath = Join-Path $RepoRoot "engine-src" "upstream.lock"
$BinariesDir = Join-Path $RepoRoot "src-tauri" "binaries"
$SkipsPath = Join-Path $PSScriptRoot "verify-skips.json"
$RegexFixturesDir = Join-Path $RepoRoot "fixtures" "golden" "regex"

. (Join-Path $PSScriptRoot "lib" "engine-common.ps1")

# SHA-256 of a file's bytes with every newline convention collapsed to LF.
#
# Layer 3 compares the engine's output against a committed golden. Those
# goldens were generated on Windows and are stored CRLF, and .gitattributes
# pins `fixtures/** -text` so git never rewrites them in either direction -
# deliberately, because several fixtures under fixtures/ exist precisely to
# test exact bytes (CRLF vs LF, a UTF-8 BOM); see fixtures/README.md. The
# macOS build of pgn-extract writes LF, so a byte-exact comparison failed
# all six cases there for a reason that has nothing to do with the regex
# engine: the outputs are content-identical and only the newline bytes
# differ. That was confirmed by reproducing each macOS run's actual hash
# exactly from the committed golden with CR stripped, all six of six.
#
# This is a fallback, never the primary check - see the call site.
function Get-NewlineNormalizedSha256 {
    param([Parameter(Mandatory)][string]$Path)

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $normalized = [System.Collections.Generic.List[byte]]::new($bytes.Length)
    for ($i = 0; $i -lt $bytes.Length; $i++) {
        if ($bytes[$i] -eq 13) {
            # CR: drop it when it is the CR of a CRLF pair; translate a lone
            # CR to LF. All three historical conventions therefore normalize
            # to the same bytes, so this cannot pass a file that merely
            # *looks* similar - only one whose content is identical once
            # line endings are set aside.
            if (($i + 1) -lt $bytes.Length -and $bytes[$i + 1] -eq 10) { continue }
            $normalized.Add([byte]10)
            continue
        }
        $normalized.Add($bytes[$i])
    }

    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return (($sha.ComputeHash($normalized.ToArray()) | ForEach-Object { $_.ToString('x2') }) -join '')
    }
    finally {
        $sha.Dispose()
    }
}

if (-not (Test-Path $LockPath)) { throw "Could not find $LockPath" }
$lock = Get-Content -Raw -LiteralPath $LockPath | ConvertFrom-Json

$triple = Get-HostTriple -Override $Triple
# Windows sidecars carry a .exe suffix (architecture.md §10.2); macOS
# ones do not. $IsWindows/$IsMacOS/$IsLinux are PS7+ automatic variables,
# so this one script stays runnable via pwsh on any of the three CI
# platforms rather than assuming Windows throughout.
$exeSuffix = if ($IsWindows) { ".exe" } else { "" }
$binaryPath = Join-Path $BinariesDir "pgn-extract-$triple$exeSuffix"

Write-Section "PGN Studio engine verification"
Write-Host "Repo root : $RepoRoot"
Write-Host "Triple    : $triple"
Write-Host "Binary    : $binaryPath"

$layerResults = [ordered]@{}
$hardFailure = $null

# ========================================================================
# Layer 0 - pin provenance (optional; original Phase 0 check, extended)
# ========================================================================
if (-not $SkipPinProvenance) {
    Write-Section "Layer 0: pin provenance (re-download + checksum pgn-extract source archive)"
    try {
        Write-Host "  repository : $($lock.engine.repository)"
        Write-Host "  commit     : $($lock.engine.commit)"
        Write-Host "  archive    : $($lock.engine.sourceArchiveUrl)"
        Write-Host "  expected   : $($lock.engine.sourceArchiveSha256)"

        $tempFile = New-TemporaryFile
        try {
            Invoke-WebRequest -Uri $lock.engine.sourceArchiveUrl -OutFile $tempFile.FullName -UseBasicParsing
            $actualHash = (Get-FileHash -Path $tempFile.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            $expectedHash = $lock.engine.sourceArchiveSha256.ToLowerInvariant()
            Write-Host "  actual     : $actualHash"
            if ($actualHash -ne $expectedHash) {
                throw "CHECKSUM MISMATCH: downloaded pgn-extract archive does not match upstream.lock. Do not build or bundle from this state."
            }
            Write-Host "  OK: source archive checksum matches upstream.lock"
            $layerResults["0-pin-provenance"] = [ordered]@{ status = "pass"; archiveSha256 = $actualHash }
        }
        finally {
            Remove-Item -Path $tempFile.FullName -Force -ErrorAction SilentlyContinue
        }
    }
    catch {
        Write-Host "  FAILED: $($_.Exception.Message)"
        $layerResults["0-pin-provenance"] = [ordered]@{ status = "fail"; error = $_.Exception.Message }
    }
}
else {
    Write-Section "Layer 0: pin provenance - SKIPPED (-SkipPinProvenance)"
    $layerResults["0-pin-provenance"] = [ordered]@{ status = "skipped" }
}

# ========================================================================
# Layer 1 - identity (hash + size + --version)
# ========================================================================
Write-Section "Layer 1: identity"
if (-not (Test-Path $binaryPath)) {
    Write-Host "  ENGINE_MISSING: $binaryPath does not exist."
    Write-Host "  Run scripts/build-pgn-extract.ps1 first."
    $layerResults["1-identity"] = [ordered]@{ status = "fail"; error = "ENGINE_MISSING: $binaryPath does not exist" }
    $hardFailure = "ENGINE_MISSING - cannot run any further verification layers without a built binary"
}
else {
    try {
        $checksumsPath = Join-Path $BinariesDir "checksums.json"
        if (-not (Test-Path $checksumsPath)) { throw "checksums.json not found at $checksumsPath - run scripts/build-pgn-extract.ps1 first" }
        $checksums = Get-Content -Raw -LiteralPath $checksumsPath | ConvertFrom-Json
        $binaryName = Split-Path -Leaf $binaryPath
        $entry = $checksums.$binaryName
        if (-not $entry) { throw "checksums.json has no entry for '$binaryName'" }

        $actualHash = (Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash.ToLowerInvariant()
        $actualSize = (Get-Item -LiteralPath $binaryPath).Length
        Write-Host "  sha256 (actual)   : $actualHash"
        Write-Host "  sha256 (expected) : $($entry.sha256.ToLowerInvariant())"
        Write-Host "  size   (actual)   : $actualSize bytes"
        Write-Host "  size   (expected) : $($entry.sizeBytes) bytes"

        if ($actualHash -ne $entry.sha256.ToLowerInvariant()) {
            throw "ENGINE_TAMPERED: sha256 mismatch (installed binary does not match checksums.json)"
        }
        if ($actualSize -ne $entry.sizeBytes) {
            throw "ENGINE_TAMPERED: size mismatch (installed binary does not match checksums.json)"
        }

        # NOTE: pgn-extract writes --version to GlobalState.logfile, which
        # defaults to stderr (main.c) - must merge via 2>&1.
        $versionOutput = & $binaryPath "--version" 2>&1
        $versionExit = $LASTEXITCODE
        $versionText = ($versionOutput | Out-String).Trim()
        $expectedVersion = "pgn-extract $($lock.engine.version)"
        Write-Host "  --version exit    : $versionExit"
        Write-Host "  --version output  : $versionText"
        if ($versionExit -ne 0) {
            throw "ENGINE_START_FAILED: --version exited $versionExit (expected 0)"
        }
        if ($versionText -ne $expectedVersion) {
            throw "ENGINE_TAMPERED: --version printed '$versionText', expected exactly '$expectedVersion'"
        }

        Write-Host "  OK"
        $layerResults["1-identity"] = [ordered]@{
            status  = "pass"
            sha256  = $actualHash
            sizeBytes = $actualSize
            version = $versionText
        }
    }
    catch {
        Write-Host "  FAILED: $($_.Exception.Message)"
        $layerResults["1-identity"] = [ordered]@{ status = "fail"; error = $_.Exception.Message }
        $hardFailure = $_.Exception.Message
    }
}

# ========================================================================
# Layer 2 - upstream regression suite (test/Makefile)
# ========================================================================
if ($hardFailure) {
    Write-Section "Layer 2: upstream regression suite - SKIPPED (Layer 1 hard failure: $hardFailure)"
    $layerResults["2-upstream-suite"] = [ordered]@{ status = "skipped"; reason = $hardFailure }
}
elseif ($SkipUpstreamSuite) {
    Write-Section "Layer 2: upstream regression suite - SKIPPED (-SkipUpstreamSuite)"
    $layerResults["2-upstream-suite"] = [ordered]@{ status = "skipped" }
}
else {
    Write-Section "Layer 2: upstream regression suite (test/Makefile)"
    try {
        Update-PathFromRegistry
        $makeCmd = Get-Command make -ErrorAction SilentlyContinue
        if (-not $makeCmd) {
            throw "GNU make not found on PATH. Install it (e.g. 'winget install --id ezwinports.make -e' or 'choco install make -y') and retry, or pass -SkipUpstreamSuite."
        }
        # Collected then sliced, never `& make --version | Select-Object
        # -First 1` - see the long note in Get-HostTriple
        # (lib/engine-common.ps1) for why piping a native command straight
        # into Select-Object -First can leave $LASTEXITCODE unset.
        $makeVersionLines = & make --version
        $makeVersionBanner = @($makeVersionLines) | Select-Object -First 1
        Write-Host "  make: $makeVersionBanner"

        # On Windows this `make` (ezwinports GNU Make) runs recipe lines
        # via cmd.exe by default, which (a) does not search Git's
        # usr\bin, so diff/cmp/rm are "not found", and (b) cmd's builtin
        # `echo` preserves literal quote characters instead of stripping
        # them like a POSIX shell - breaking both CMP and the
        # target-boundary parsing below. Point make at Git Bash's sh.exe
        # (SHELL=) and add usr\bin to PATH so recipes run the way
        # test/Makefile's own SEP=/ RM="rm -f" CMP=cmp defaults assume.
        # On macOS/Linux, make already uses /bin/sh and diff/cmp/rm are
        # already on PATH, so none of this is needed.
        $shellArgs = @()
        if ($IsWindows) {
            $gitCmd = Get-Command git.exe -ErrorAction SilentlyContinue
            if (-not $gitCmd) { throw "git.exe not found on PATH." }
            $gitRoot = Split-Path -Parent (Split-Path -Parent $gitCmd.Source)
            $gitUsrBin = Join-Path $gitRoot "usr\bin"
            $gitShExe = Join-Path $gitRoot "bin\sh.exe"
            if (-not (Test-Path $gitShExe)) { throw "Expected Git Bash's sh.exe at $gitShExe (derived from git.exe at $($gitCmd.Source)) but it does not exist." }
            if (-not (Test-Path (Join-Path $gitUsrBin "diff.exe"))) { throw "Expected diff.exe at $gitUsrBin but it does not exist - Git for Windows install looks incomplete." }
            $env:PATH = "$gitUsrBin;$env:PATH"
            $shellForMake = ($gitShExe -replace '\\', '/')
            $shellArgs = @("SHELL=$shellForMake")
            Write-Host "  SHELL for make: $shellForMake"
            Write-Host "  added to PATH : $gitUsrBin"
        }

        # Reuse (or fetch, if this script is run standalone before any
        # build) the exact pinned pgn-extract checkout the binary was
        # built from, so the suite's infiles/outfiles oracles are the
        # ones that shipped with THIS commit.
        $pgnSrcDir = Join-Path $CacheDir "pgn-extract"
        Get-PinnedCheckout -Name "pgn-extract" `
            -PrimaryUrl $lock.engine.repository -MirrorUrl $lock.engine.mirror `
            -Commit $lock.engine.commit -ExpectedTree $lock.engine.gitTree `
            -Dest $pgnSrcDir | Out-Null

        $testDir = Join-Path $pgnSrcDir "test"
        if (-not (Test-Path $testDir)) { throw "Upstream test/ directory not found at $testDir" }

        Write-Host "  cleaning previous run artifacts"
        & make -C $testDir @shellArgs clean 2>&1 | Out-Null

        $binForMake = $binaryPath -replace '\\', '/'
        $ecoForMake = (Join-Path $pgnSrcDir "eco.pgn") -replace '\\', '/'
        Write-Host "  running: make -k -C test all PGN_EXTRACT=$binForMake ECO_FILE=$ecoForMake CMP=diff $($shellArgs -join ' ')"

        $logLines = & make -k -C $testDir all "PGN_EXTRACT=$binForMake" "ECO_FILE=$ecoForMake" "CMP=diff" @shellArgs 2>&1
        $makeExitCode = $LASTEXITCODE
        $logPath = Join-Path $BinariesDir "verify-upstream-suite-$triple.log"
        New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null
        ($logLines | ForEach-Object { $_.ToString() }) | Set-Content -LiteralPath $logPath -Encoding utf8NoBOM
        Write-Host "  full log: $logPath"

        # ---- Parse the log: a line that is EXACTLY "target-name:" is
        # this Makefile's own `echo "target-name:"` recipe line firing -
        # every target's recipe starts with that, per test/Makefile. This
        # is intentionally NOT deduplicated: several real targets share
        # one announcement string (e.g. test-bl45/test-bu45/test-bu30 all
        # echo "test-b:"; test-pl90/test-pu90/test-pu60 all echo
        # "test-p:") so each occurrence is one real recipe invocation, and
        # counting distinct real invocations (not distinct strings) is
        # what "exact pass/fail count" means here. A
        # "make: *** [file:line: target] Error N" line (the fatal form -
        # NOT the "... Error N (ignored)" form used for the one
        # intentionally-`-`-prefixed recipe line, test-h's `-$(PGN_EXTRACT)
        # -h`, which upstream itself does not treat as a failure) is a
        # failure for that specific real target name.
        $attemptedTargets = [System.Collections.Generic.List[string]]::new()
        $failedTargets = [System.Collections.Generic.List[string]]::new()
        foreach ($rawLine in $logLines) {
            # Defensive \r? in case a CRLF ever slips through despite
            # SHELL= pointing at sh.exe (e.g. a differently-built make).
            $s = $rawLine.ToString().TrimEnd("`r")
            if ($s -match '^([A-Za-z0-9_#.-]+):$') {
                $attemptedTargets.Add($Matches[1])
            }
            elseif ($s -match '^make(\[\d+\])?:\s+\*\*\*\s+\[[^:\]]*:\d+:\s*([^\]]+)\]\s+Error\s+\d+') {
                $failedName = $Matches[2].Trim()
                if (-not $failedTargets.Contains($failedName)) { $failedTargets.Add($failedName) }
            }
        }

        # ---- Load justified skips ----
        $skipEntries = @()
        if (Test-Path $SkipsPath) {
            $skipsDoc = Get-Content -Raw -LiteralPath $SkipsPath | ConvertFrom-Json
            $skipEntries = @($skipsDoc.skips)
        }
        $skipMap = @{}
        foreach ($se in $skipEntries) { $skipMap[$se.target] = $se.reason }

        $realFailures = [System.Collections.Generic.List[string]]::new()
        $justifiedSkips = [System.Collections.Generic.List[object]]::new()
        foreach ($t in $failedTargets) {
            if ($skipMap.ContainsKey($t)) {
                $justifiedSkips.Add([PSCustomObject]@{ target = $t; reason = $skipMap[$t] })
            }
            else {
                $realFailures.Add($t)
            }
        }

        $passCount = $attemptedTargets.Count - $failedTargets.Count
        Write-Host ""
        Write-Host "  attempted targets : $($attemptedTargets.Count)"
        Write-Host "  passed            : $passCount"
        Write-Host "  failed (total)    : $($failedTargets.Count)"
        Write-Host "  failed (justified-skip) : $($justifiedSkips.Count)"
        Write-Host "  failed (real)     : $($realFailures.Count)"
        if ($justifiedSkips.Count -gt 0) {
            Write-Host "  justified skips:"
            foreach ($js in $justifiedSkips) { Write-Host "    - $($js.target): $($js.reason)" }
        }
        if ($realFailures.Count -gt 0) {
            Write-Host "  REAL FAILURES:"
            foreach ($f in $realFailures) { Write-Host "    - $f" }
        }
        # test-odds must be in the passing (or justified-skip) set - it
        # is the only upstream target that exercises grammar.c's regex
        # call site, and losing coverage of it silently would be exactly
        # the kind of regression this layer exists to catch.
        if ($attemptedTargets.Contains("test-odds") -and $realFailures.Contains("test-odds")) {
            Write-Host "  NOTE: test-odds (grammar.c regex call site) is among the REAL FAILURES above - this is a release blocker per design-01-engine-build.md §6.4."
        }
        elseif (-not $attemptedTargets.Contains("test-odds")) {
            Write-Host "  WARNING: test-odds was not observed running at all - the log parser or the Makefile's target list may have changed."
        }

        $status = if ($realFailures.Count -eq 0) { "pass" } else { "fail" }
        $layerResults["2-upstream-suite"] = [ordered]@{
            status            = $status
            makeVersion       = $makeVersionBanner
            makeExitCode      = $makeExitCode
            attemptedTargets  = $attemptedTargets.Count
            passedTargets     = $passCount
            failedTargetsTotal = $failedTargets.Count
            justifiedSkips    = $justifiedSkips
            realFailures      = $realFailures
            logFile           = $logPath
        }
    }
    catch {
        Write-Host "  FAILED: $($_.Exception.Message)"
        $layerResults["2-upstream-suite"] = [ordered]@{ status = "fail"; error = $_.Exception.Message }
    }
}

# ========================================================================
# Layer 3 - PGN Studio supplemental regex goldens
# ========================================================================
if ($hardFailure) {
    Write-Section "Layer 3: supplemental regex goldens - SKIPPED (Layer 1 hard failure)"
    $layerResults["3-supplemental-goldens"] = [ordered]@{ status = "skipped"; reason = $hardFailure }
}
elseif ($SkipSupplementalGoldens) {
    Write-Section "Layer 3: supplemental regex goldens - SKIPPED (-SkipSupplementalGoldens)"
    $layerResults["3-supplemental-goldens"] = [ordered]@{ status = "skipped" }
}
else {
    Write-Section "Layer 3: PGN Studio supplemental regex goldens (fixtures/golden/regex/)"
    try {
        $manifestPath = Join-Path $RegexFixturesDir "manifest.json"
        if (-not (Test-Path $manifestPath)) { throw "manifest.json not found at $manifestPath" }
        $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json

        $caseResults = [System.Collections.Generic.List[object]]::new()
        foreach ($case in $manifest.cases) {
            $inputFile = Join-Path $RegexFixturesDir $case.input
            $expectedFile = Join-Path $RegexFixturesDir $case.expected
            $tempOut = Join-Path ([System.IO.Path]::GetTempPath()) "pgnstudio-golden-$($case.name)-$([guid]::NewGuid().ToString('N')).pgn"

            $cliArgs = [System.Collections.Generic.List[string]]::new()
            if ($case.criteria) { $cliArgs.Add("-t$(Join-Path $RegexFixturesDir $case.criteria)") }
            foreach ($a in @($case.args)) { if ($a) { $cliArgs.Add($a) } }
            $cliArgs.Add("-o$tempOut")
            $cliArgs.Add("--quiet")
            $cliArgs.Add($inputFile)

            $null = & $binaryPath @cliArgs 2>&1
            $exit = $LASTEXITCODE

            $pass = $false
            $newlineNormalized = $false
            $detail = ""
            if ($exit -ne 0) {
                $detail = "exit code $exit (expected 0)"
            }
            elseif (-not (Test-Path $tempOut)) {
                $detail = "no output file was produced"
            }
            else {
                # Byte-exact is still the primary assertion and still the
                # strongest result this layer can report - it is tried first
                # and, where it holds (Windows), nothing below runs. Only
                # when it fails do we ask the weaker question "is this the
                # same content with different line endings?", and a pass by
                # that route is labelled differently everywhere it appears.
                # Normalizing unconditionally would have been less code and
                # strictly worse: it would silently stop detecting a real
                # CRLF/LF regression on Windows, which is exactly the kind
                # of byte-level change fixtures/ exists to catch.
                $actualHash = (Get-FileHash -LiteralPath $tempOut -Algorithm SHA256).Hash.ToLowerInvariant()
                $expectedHash = (Get-FileHash -LiteralPath $expectedFile -Algorithm SHA256).Hash.ToLowerInvariant()
                if ($actualHash -eq $expectedHash) {
                    $pass = $true
                }
                else {
                    $actualNormalized = Get-NewlineNormalizedSha256 -Path $tempOut
                    $expectedNormalized = Get-NewlineNormalizedSha256 -Path $expectedFile
                    if ($actualNormalized -eq $expectedNormalized) {
                        $pass = $true
                        $newlineNormalized = $true
                        $detail = "line endings differ only - the committed golden is CRLF, this build writes LF. Content is identical after CRLF/LF normalization (sha256 $actualNormalized)."
                    }
                    else {
                        $detail = "output does not match $($case.expected), and not merely in its line endings (raw sha256 $actualHash vs $expectedHash; newline-normalized $actualNormalized vs $expectedNormalized)"
                    }
                }
            }
            Remove-Item -LiteralPath $tempOut -ErrorAction SilentlyContinue

            $caseResults.Add([PSCustomObject]@{
                    name              = $case.name
                    pass              = $pass
                    newlineNormalized = $newlineNormalized
                    detail            = $detail
                })
            if ($pass -and $newlineNormalized) {
                Write-Host "  [PASS~] $($case.name) - $($case.description)"
                Write-Host "          note: $detail"
            }
            elseif ($pass) {
                Write-Host "  [PASS] $($case.name) - $($case.description)"
            }
            else {
                Write-Host "  [FAIL] $($case.name) - $detail"
            }
        }

        $failedCases = @($caseResults | Where-Object { -not $_.pass })
        $normalizedCases = @($caseResults | Where-Object { $_.pass -and $_.newlineNormalized })
        $status = if ($failedCases.Count -eq 0) { "pass" } else { "fail" }
        Write-Host ""
        Write-Host "  $($caseResults.Count - $failedCases.Count) / $($caseResults.Count) supplemental golden cases passed"
        if ($normalizedCases.Count -gt 0) {
            # Stated at the summary level too, not just per-case: a reader
            # skimming for "6 / 6 passed" should not be able to miss that
            # some of those were line-ending-normalized rather than
            # byte-exact.
            Write-Host "  of which $($normalizedCases.Count) matched only after CRLF/LF normalization (content identical, line endings differ):"
            foreach ($nc in $normalizedCases) { Write-Host "    - $($nc.name)" }
        }
        $layerResults["3-supplemental-goldens"] = [ordered]@{
            status                        = $status
            total                         = $caseResults.Count
            passed                        = $caseResults.Count - $failedCases.Count
            passedByteExact               = $caseResults.Count - $failedCases.Count - $normalizedCases.Count
            passedAfterNewlineNormalization = $normalizedCases.Count
            cases                         = $caseResults
        }
    }
    catch {
        Write-Host "  FAILED: $($_.Exception.Message)"
        $layerResults["3-supplemental-goldens"] = [ordered]@{ status = "fail"; error = $_.Exception.Message }
    }
}

# ========================================================================
# Report + summary
# ========================================================================
Write-Section "Verification summary"
$anyFail = $false
foreach ($key in $layerResults.Keys) {
    $r = $layerResults[$key]
    $statusText = $r.status.ToUpperInvariant()
    Write-Host "  $key : $statusText"
    if ($r.status -eq "fail") { $anyFail = $true }
}

$reportPath = Join-Path $BinariesDir "verify-report-$triple.json"
New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null
[ordered]@{
    triple         = $triple
    binary         = $binaryPath
    generatedAtUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    engineVersion  = $lock.engine.version
    engineCommit   = $lock.engine.commit
    layers         = $layerResults
} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $reportPath -Encoding utf8NoBOM
Write-Host ""
Write-Host "  full report: $reportPath"

if ($anyFail) {
    Write-Host ""
    Write-Host "RESULT: FAIL"
    exit 1
}
else {
    Write-Host ""
    Write-Host "RESULT: PASS"
    exit 0
}
