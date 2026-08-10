# SPDX-License-Identifier: GPL-3.0-or-later
#
# Shared helpers for scripts/build-pgn-extract.ps1 and
# scripts/verify-engine.ps1 - dot-sourced by both, so pin validation and
# the fetch/verify-checkout logic exist in exactly one place. Not meant
# to be run directly.
#
# Callers must set $ErrorActionPreference = "Stop" and
# Set-StrictMode -Version Latest themselves before dot-sourcing this
# (kept out of here so sourcing this file cannot silently change a
# caller's error-handling mode).

function Write-Section {
    param([string]$Text)
    Write-Host ""
    Write-Host "==== $Text ===="
}

function Test-NotPlaceholder {
    param([string]$Value, [string]$FieldName)
    if ([string]::IsNullOrWhiteSpace($Value)) {
        throw "upstream.lock: field '$FieldName' is empty."
    }
    if ($Value -match "REPLACE_WITH|RESOLVE_AT_PIN_TIME|PLACEHOLDER|TODO|FIXME|<.*>") {
        throw "upstream.lock: field '$FieldName' looks like a placeholder value: '$Value'. Refusing to proceed from an unpinned lock."
    }
}

function Assert-CommitShape {
    param([string]$Value, [string]$FieldName)
    if ($Value -notmatch '^[0-9a-f]{40}$') {
        throw "upstream.lock: field '$FieldName' ('$Value') is not a 40-hex-char git commit SHA."
    }
}

# Refresh $env:PATH from the machine+user registry values. A tool
# installed by winget/choco moments ago (make, for the upstream
# test-suite layer) may not be visible to an already-running shell; this
# is a harmless no-op otherwise.
function Update-PathFromRegistry {
    $machine = [System.Environment]::GetEnvironmentVariable("Path", "Machine")
    $user = [System.Environment]::GetEnvironmentVariable("Path", "User")
    $env:PATH = "$machine;$user"
}

# Runs a native executable with an explicit argument ARRAY (never a
# shell string - see architecture.md §16.2 and decisions ledger V-6,
# which found PowerShell mangles attached-form flags like `-oPATH` when
# they're built via string interpolation/concatenation instead of
# passed as a literal array element).
function Invoke-Native {
    param(
        [Parameter(Mandatory)][string]$Exe,
        [Parameter(Mandatory)][string[]]$Arguments,
        [string]$WorkingDirectory,
        [string]$FailureContext = $Exe
    )
    $prevLocation = Get-Location
    if ($WorkingDirectory) { Set-Location -LiteralPath $WorkingDirectory }
    try {
        & $Exe @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "$FailureContext failed (exit $LASTEXITCODE): $Exe $($Arguments -join ' ')"
        }
    }
    finally {
        Set-Location $prevLocation
    }
}

# Clones (or fetches an existing checkout of) a pinned repository, with
# mirror fallback, and hard-verifies HEAD + HEAD^{tree} against the lock.
# Used identically by the build script (to get sources to compile) and
# the verify script (to get the upstream test/ suite to run against the
# already-built binary) so both always test/build the exact same pinned
# commit.
function Get-PinnedCheckout {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$PrimaryUrl,
        [string]$MirrorUrl,
        [Parameter(Mandatory)][string]$Commit,
        [string]$ExpectedTree,
        [Parameter(Mandatory)][string]$Dest,
        [switch]$ForceRefresh
    )
    Write-Section "Fetching $Name @ $Commit"

    $haveUsableCheckout = $false
    if ((Test-Path (Join-Path $Dest ".git")) -and (-not $ForceRefresh)) {
        Write-Host "  existing checkout at $Dest - fetching and checking out pinned commit"
        try {
            Invoke-Native -Exe "git" -Arguments @("-C", $Dest, "fetch", "--quiet", "--all", "--tags") -FailureContext "git fetch ($Name)"
            Invoke-Native -Exe "git" -Arguments @("-C", $Dest, "checkout", "--quiet", "--force", $Commit) -FailureContext "git checkout ($Name)"
            $head = (& git -C $Dest rev-parse HEAD).Trim()
            $haveUsableCheckout = ($head -eq $Commit)
            if (-not $haveUsableCheckout) {
                Write-Host "  HEAD ($head) still does not match pin after fetch - discarding and re-cloning"
            }
        }
        catch {
            Write-Host "  refresh of existing checkout failed ($($_.Exception.Message)) - discarding and re-cloning"
        }
        if (-not $haveUsableCheckout) {
            Remove-Item -LiteralPath $Dest -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    elseif (Test-Path $Dest) {
        Remove-Item -LiteralPath $Dest -Recurse -Force
    }

    if (-not $haveUsableCheckout) {
        $candidates = @($PrimaryUrl)
        if ($MirrorUrl) { $candidates += $MirrorUrl }
        $cloned = $false
        foreach ($url in $candidates) {
            Write-Host "  cloning $url"
            try {
                & git clone --quiet $url $Dest
                if ($LASTEXITCODE -ne 0) { throw "git clone exited $LASTEXITCODE" }
                $cloned = $true
                if ($url -ne $PrimaryUrl) {
                    Write-Host "  NOTE: cloned from MIRROR ($url) - primary upstream ($PrimaryUrl) was unreachable or failed"
                }
                break
            }
            catch {
                Write-Host "  clone from $url failed: $($_.Exception.Message)"
                if (Test-Path $Dest) { Remove-Item -LiteralPath $Dest -Recurse -Force -ErrorAction SilentlyContinue }
            }
        }
        if (-not $cloned) {
            throw "Could not clone $Name from primary ($PrimaryUrl)$(if ($MirrorUrl) { " or mirror ($MirrorUrl)" })."
        }
        Invoke-Native -Exe "git" -Arguments @("-C", $Dest, "checkout", "--quiet", "--force", $Commit) -FailureContext "git checkout ($Name)"
    }

    $head = (& git -C $Dest rev-parse HEAD).Trim()
    $tree = (& git -C $Dest rev-parse "HEAD^{tree}").Trim()
    if ($head -ne $Commit) {
        throw "$Name checkout HEAD ($head) does not match upstream.lock pinned commit ($Commit). Refusing to proceed from an unverified checkout."
    }
    if ($ExpectedTree -and ($tree -ne $ExpectedTree)) {
        throw "$Name checkout tree ($tree) does not match upstream.lock pinned tree ($ExpectedTree). Refusing to proceed from an unverified checkout."
    }
    Write-Host "  verified: HEAD=$head tree=$tree"
    return [PSCustomObject]@{ Head = $head; Tree = $tree; Path = $Dest }
}

# Ordinal (byte-value), locale-independent sort of a file list. Used
# wherever a set of files discovered via Get-ChildItem glob is about to
# be fed to cl.exe/lib.exe/link.exe as positional arguments - reproducible
# builds (engine-src/README.md "Reproducibility") need compile/archive/
# link input ORDER to be a pure function of the file names, never of
# filesystem enumeration order (an NTFS/OS/driver implementation detail,
# not a documented contract) or the host's locale/collation settings
# (default Sort-Object string comparison is culture-aware). MSVC's
# /Brepro (see the same README section) makes a *given* input order
# produce byte-identical output; this function is what pins the order
# itself, independent of the machine it runs on.
function Get-OrdinalSorted {
    param([AllowEmptyCollection()][string[]]$Items)
    $arr = [string[]]@($Items)
    [Array]::Sort($arr, [StringComparer]::Ordinal)
    return , $arr
}

function Get-HostTriple {
    param([string]$Override)
    if ($Override) { return $Override }
    $rustc = Get-Command rustc -ErrorAction SilentlyContinue
    if ($rustc) {
        $triple = (& rustc --print host-tuple 2>$null | Select-Object -First 1)
        if ($LASTEXITCODE -eq 0 -and $triple) {
            return $triple.Trim()
        }
    }
    Write-Warning "rustc not found (or failed); defaulting target triple to x86_64-pc-windows-msvc. Pass -Triple to override."
    return "x86_64-pc-windows-msvc"
}
