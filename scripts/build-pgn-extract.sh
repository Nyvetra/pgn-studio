#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# *** UNTESTED FROM THIS MACHINE ***
# This script was authored on a Windows-only development machine (no Mac
# is available - decisions ledger D-006). It mirrors
# scripts/build-pgn-extract.ps1's contract and has been reviewed for
# correctness against Apple clang / macOS documentation, but it has
# never actually been run. Treat its first real run (a macOS CI leg, or
# a contributor's Mac) as a verification step, not a formality - report
# any deviation from this file's assumptions rather than silently
# patching around it.
#
# Builds the pgn-extract sidecar for aarch64-apple-darwin or
# x86_64-apple-darwin from the pinned source in engine-src/upstream.lock:
#
#   1. Validate engine-src/upstream.lock (refuse placeholders).
#   2. Fetch pgn-extract into a gitignored cache dir (engine-src/.build/),
#      verifying `git rev-parse HEAD` AND `HEAD^{tree}` against the lock;
#      try upstream first, fall back to the Nyvetra mirror; hard-fail on
#      any mismatch. (No TRE on macOS - system libc provides POSIX
#      <regex.h>, exactly as upstream's own unix Makefile assumes.)
#   3. Apply engine-src/patches/*.patch (lexical order). There are
#      currently none.
#   4. Compile with Apple clang: `-std=c99 -O3`, link `-lm`. No manifest/
#      code-page handling is needed on macOS - Apple filesystem APIs are
#      natively UTF-8, so plain fopen()/access() already handle
#      non-ASCII paths correctly (unlike Windows; see
#      engine-src/manifest/pgn-extract.manifest for why Windows needs
#      one).
#   5. Smoke-check `--version` (argument array via exec, never a shell)
#      against upstream.lock's pinned version string.
#   6. Install to src-tauri/binaries/pgn-extract-<triple>.
#   7. Write src-tauri/binaries/build-info-<triple>.json and merge into
#      src-tauri/binaries/checksums.json.
#
# Requires: bash, git, Xcode Command Line Tools (clang), jq (preinstalled
# on GitHub-hosted macos-latest/macos-13/macos-14 runners; `brew install
# jq` for local use).
#
# Usage:
#   ./scripts/build-pgn-extract.sh                # build for the host's own arch
#   ./scripts/build-pgn-extract.sh --arch x86_64   # cross-build via clang -arch
#   ./scripts/build-pgn-extract.sh --arch arm64
#
# Exit code 0 = binary built, smoke-checked, and installed. Non-zero =
# see stderr; nothing partially-built is left installed (install only
# happens after the smoke check passes).

set -euo pipefail

# --------------------------------------------------------------------
# Args
# --------------------------------------------------------------------
ARCH=""
CACHE_DIR=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch) ARCH="$2"; shift 2 ;;
        --cache-dir) CACHE_DIR="$2"; shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
: "${CACHE_DIR:=$REPO_ROOT/engine-src/.build}"
LOCK_PATH="$REPO_ROOT/engine-src/upstream.lock"
PATCHES_DIR="$REPO_ROOT/engine-src/patches"
BINARIES_DIR="$REPO_ROOT/src-tauri/binaries"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: this script builds the macOS sidecar and must run on macOS (uname -s == Darwin). Detected: $(uname -s)." >&2
    exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
    echo "ERROR: jq is required to parse/write JSON in this script. Install it (brew install jq) and retry." >&2
    exit 1
fi
if ! xcrun --find clang >/dev/null 2>&1; then
    echo "ERROR: clang not found via xcrun. Install Xcode Command Line Tools: xcode-select --install" >&2
    exit 1
fi

if [[ -z "$ARCH" ]]; then
    case "$(uname -m)" in
        arm64) ARCH="arm64" ;;
        x86_64) ARCH="x86_64" ;;
        *) echo "ERROR: unrecognized host arch $(uname -m); pass --arch explicitly." >&2; exit 1 ;;
    esac
fi
case "$ARCH" in
    arm64) TRIPLE="aarch64-apple-darwin" ;;
    x86_64) TRIPLE="x86_64-apple-darwin" ;;
    *) echo "ERROR: --arch must be 'arm64' or 'x86_64', got '$ARCH'." >&2; exit 1 ;;
esac

section() { printf '\n==== %s ====\n' "$1"; }

sha256_file() {
    # macOS ships shasum, not sha256sum, by default.
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        sha256sum "$1" | awk '{print $1}'
    fi
}

section "PGN Studio engine build (macOS / Apple clang, $TRIPLE)"
echo "Repo root : $REPO_ROOT"
echo "Cache dir : $CACHE_DIR"
echo "NOTE: this script is UNTESTED FROM THIS MACHINE (no Mac available in dev)."

[[ -f "$LOCK_PATH" ]] || { echo "ERROR: could not find $LOCK_PATH" >&2; exit 1; }

# ---- 1. Validate the lock ------------------------------------------------
section "Validating engine-src/upstream.lock"
assert_not_placeholder() {
    local value="$1" name="$2"
    if [[ -z "$value" ]]; then
        echo "ERROR: upstream.lock field '$name' is empty." >&2; exit 1
    fi
    if [[ "$value" =~ REPLACE_WITH|RESOLVE_AT_PIN_TIME|PLACEHOLDER|TODO|FIXME ]]; then
        echo "ERROR: upstream.lock field '$name' looks like a placeholder: '$value'" >&2; exit 1
    fi
}
ENGINE_REPO=$(jq -r '.engine.repository' "$LOCK_PATH")
ENGINE_MIRROR=$(jq -r '.engine.mirror // empty' "$LOCK_PATH")
ENGINE_COMMIT=$(jq -r '.engine.commit' "$LOCK_PATH")
ENGINE_TREE=$(jq -r '.engine.gitTree // empty' "$LOCK_PATH")
ENGINE_VERSION=$(jq -r '.engine.version' "$LOCK_PATH")
MACOS_FLAGS=$(jq -r '.toolchains."'"$TRIPLE"'".engineFlags // ["-std=c99","-O3"] | join(" ")' "$LOCK_PATH")
# linkFlags mirrors Windows's /Brepro linker determinism flag (see
# engine-src/README.md "Reproducibility"): -Wl,-no_uuid drops Mach-O's
# LC_UUID load command, which ld64 otherwise fills with a fresh random
# UUID on every link. UNVERIFIED on real Apple clang - see the note on
# this triple in upstream.lock and the banner at the top of this file.
MACOS_LINK_FLAGS=$(jq -r '.toolchains."'"$TRIPLE"'".linkFlags // [] | join(" ")' "$LOCK_PATH")

assert_not_placeholder "$ENGINE_REPO" "engine.repository"
assert_not_placeholder "$ENGINE_COMMIT" "engine.commit"
assert_not_placeholder "$ENGINE_VERSION" "engine.version"
if [[ ! "$ENGINE_COMMIT" =~ ^[0-9a-f]{40}$ ]]; then
    echo "ERROR: engine.commit ('$ENGINE_COMMIT') is not a 40-hex-char git SHA." >&2; exit 1
fi
echo "  OK: pgn-extract $ENGINE_VERSION @ $ENGINE_COMMIT"

# ---- 2. Fetch pinned pgn-extract source -----------------------------------
section "Fetching pgn-extract @ $ENGINE_COMMIT"
PGN_SRC_DIR="$CACHE_DIR/pgn-extract"

have_checkout=0
if [[ -d "$PGN_SRC_DIR/.git" ]]; then
    echo "  existing checkout at $PGN_SRC_DIR - fetching and checking out pinned commit"
    if git -C "$PGN_SRC_DIR" fetch --quiet --all --tags \
        && git -C "$PGN_SRC_DIR" checkout --quiet --force "$ENGINE_COMMIT"; then
        head="$(git -C "$PGN_SRC_DIR" rev-parse HEAD)"
        [[ "$head" == "$ENGINE_COMMIT" ]] && have_checkout=1
    fi
    if [[ "$have_checkout" -ne 1 ]]; then
        echo "  existing checkout unusable - discarding and re-cloning"
        rm -rf "$PGN_SRC_DIR"
    fi
fi

if [[ "$have_checkout" -ne 1 ]]; then
    cloned=0
    for url in "$ENGINE_REPO" "$ENGINE_MIRROR"; do
        [[ -z "$url" ]] && continue
        echo "  cloning $url"
        if git clone --quiet "$url" "$PGN_SRC_DIR"; then
            cloned=1
            [[ "$url" != "$ENGINE_REPO" ]] && echo "  NOTE: cloned from MIRROR ($url) - primary upstream was unreachable or failed"
            break
        else
            rm -rf "$PGN_SRC_DIR"
        fi
    done
    [[ "$cloned" -eq 1 ]] || { echo "ERROR: could not clone pgn-extract from primary ($ENGINE_REPO) or mirror ($ENGINE_MIRROR)." >&2; exit 1; }
    git -C "$PGN_SRC_DIR" checkout --quiet --force "$ENGINE_COMMIT"
fi

ACTUAL_HEAD="$(git -C "$PGN_SRC_DIR" rev-parse HEAD)"
ACTUAL_TREE="$(git -C "$PGN_SRC_DIR" rev-parse "HEAD^{tree}")"
[[ "$ACTUAL_HEAD" == "$ENGINE_COMMIT" ]] || { echo "ERROR: checkout HEAD ($ACTUAL_HEAD) != pinned commit ($ENGINE_COMMIT)" >&2; exit 1; }
if [[ -n "$ENGINE_TREE" && "$ACTUAL_TREE" != "$ENGINE_TREE" ]]; then
    echo "ERROR: checkout tree ($ACTUAL_TREE) != pinned tree ($ENGINE_TREE)" >&2; exit 1
fi
echo "  verified: HEAD=$ACTUAL_HEAD tree=$ACTUAL_TREE"

# ---- 3. Apply patches -------------------------------------------------------
section "Applying patches"
shopt -s nullglob
patches=("$PATCHES_DIR"/*.patch)
shopt -u nullglob
if [[ ${#patches[@]} -eq 0 ]]; then
    echo "  no *.patch files in $PATCHES_DIR (expected - see engine-src/patches/README.md)"
else
    for p in "${patches[@]}"; do
        echo "  applying $(basename "$p")"
        git -C "$PGN_SRC_DIR" apply --whitespace=nowarn "$p"
    done
fi

# ---- 4. Compile --------------------------------------------------------------
section "Compiling pgn-extract (Apple clang, arch=$ARCH)"
WORK_DIR="$CACHE_DIR/work-$TRIPLE"
rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"
BUILT_EXE="$WORK_DIR/pgn-extract"

CLANG_VERSION_BANNER="$(clang --version | head -n1)"
echo "  compiler: $CLANG_VERSION_BANNER"

# "$PGN_SRC_DIR"/*.c: bash pathname expansion is documented to return
# sorted results (glob(3), no GLOB_NOSORT), so - unlike a raw
# Get-ChildItem enumeration on Windows - this is already a fixed, sorted
# input order without needing an explicit sort step; see
# engine-src/README.md "Reproducibility" for why input order matters and
# what the Windows build does about it.
# shellcheck disable=SC2086
clang $MACOS_FLAGS $MACOS_LINK_FLAGS -arch "$ARCH" "$PGN_SRC_DIR"/*.c -lm -o "$BUILT_EXE"
echo "  built: $BUILT_EXE"

# ---- 5. Smoke check ----------------------------------------------------------
section "Smoke check: --version"
EXPECTED_VERSION="pgn-extract $ENGINE_VERSION"
set +e
VERSION_OUTPUT="$("$BUILT_EXE" --version)"
VERSION_EXIT=$?
set -e
echo "  exit code : $VERSION_EXIT"
echo "  output    : $VERSION_OUTPUT"
if [[ "$VERSION_EXIT" -ne 0 ]]; then
    echo "ERROR: smoke check FAILED: --version exited $VERSION_EXIT (expected 0). Nothing installed." >&2
    exit 1
fi
if [[ "$VERSION_OUTPUT" != "$EXPECTED_VERSION" ]]; then
    echo "ERROR: smoke check FAILED: --version printed '$VERSION_OUTPUT', expected exactly '$EXPECTED_VERSION'. Nothing installed." >&2
    exit 1
fi
echo "  OK"

DEPENDENTS=""
if command -v otool >/dev/null 2>&1; then
    DEPENDENTS="$(otool -L "$BUILT_EXE" | tail -n +2 | awk '{print $1}' | tr '\n' ',' | sed 's/,$//')"
    echo "  linked libraries: $DEPENDENTS"
fi

# ---- 6. Install ----------------------------------------------------------------
section "Installing sidecar"
INSTALLED_NAME="pgn-extract-$TRIPLE"
INSTALLED_PATH="$BINARIES_DIR/$INSTALLED_NAME"
mkdir -p "$BINARIES_DIR"
cp "$BUILT_EXE" "$INSTALLED_PATH"
echo "  installed: $INSTALLED_PATH"

# ---- 7. checksums.json + build-info-<triple>.json -------------------------------
section "Recording checksums and build info"
HASH="$(sha256_file "$INSTALLED_PATH")"
SIZE="$(wc -c < "$INSTALLED_PATH" | tr -d ' ')"
echo "  sha256 : $HASH"
echo "  size   : $SIZE bytes"

CHECKSUMS_PATH="$BINARIES_DIR/checksums.json"
if [[ ! -f "$CHECKSUMS_PATH" ]]; then
    echo '{}' > "$CHECKSUMS_PATH"
fi
ECO_SHA256=$(jq -r '.engine.resources."eco.pgn".sha256' "$LOCK_PATH")
ECO_SIZE=$(jq -r '.engine.resources."eco.pgn".sizeBytes' "$LOCK_PATH")
jq \
    --arg name "$INSTALLED_NAME" \
    --arg sha256 "$HASH" \
    --argjson size "$SIZE" \
    --arg engineVersion "$ENGINE_VERSION" \
    --arg commit "$ENGINE_COMMIT" \
    --arg ecoSha256 "$ECO_SHA256" \
    --argjson ecoSize "$ECO_SIZE" \
    '.[$name] = {sha256: $sha256, sizeBytes: $size, engineVersion: $engineVersion, commit: $commit}
     | ."eco.pgn" = {sha256: $ecoSha256, sizeBytes: $ecoSize}' \
    "$CHECKSUMS_PATH" > "$CHECKSUMS_PATH.tmp"
mv "$CHECKSUMS_PATH.tmp" "$CHECKSUMS_PATH"
echo "  wrote: $CHECKSUMS_PATH"

LOCK_DIGEST="$(sha256_file "$LOCK_PATH")"
BUILDER="local:$(whoami)@$(hostname)"
if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
    BUILDER="ci:github-actions:run=${GITHUB_RUN_ID:-unknown}:repo=${GITHUB_REPOSITORY:-unknown}"
fi
BUILT_AT_UTC="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
BUILD_INFO_PATH="$BINARIES_DIR/build-info-$TRIPLE.json"
jq -n \
    --arg triple "$TRIPLE" \
    --arg binary "$INSTALLED_NAME" \
    --arg sha256 "$HASH" \
    --argjson sizeBytes "$SIZE" \
    --arg engineVersion "$ENGINE_VERSION" \
    --arg engineCommit "$ENGINE_COMMIT" \
    --arg upstreamLockSha256 "$LOCK_DIGEST" \
    --arg compiler "$CLANG_VERSION_BANNER" \
    --arg flags "$MACOS_FLAGS $MACOS_LINK_FLAGS -arch $ARCH -lm" \
    --arg dllDependents "$DEPENDENTS" \
    --arg builtAtUtc "$BUILT_AT_UTC" \
    --arg builder "$BUILDER" \
    '{triple: $triple, binary: $binary, sha256: $sha256, sizeBytes: $sizeBytes,
      engineVersion: $engineVersion, engineCommit: $engineCommit,
      upstreamLockSha256: $upstreamLockSha256, compiler: $compiler, flags: $flags,
      dllDependents: $dllDependents, builtAtUtc: $builtAtUtc, builder: $builder}' \
    > "$BUILD_INFO_PATH"
echo "  wrote: $BUILD_INFO_PATH"

section "Build complete"
echo "  $INSTALLED_PATH"
echo "  sha256: $HASH"
echo "  Run scripts/verify-engine.sh (or the relevant verify layer) next."
echo "  REMINDER: this script has never been executed on real macOS hardware - treat its first CI/contributor run as verification, not a formality."
