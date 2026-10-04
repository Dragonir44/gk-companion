#!/usr/bin/env bash
#
# scripts/fix-appimage-wayland.sh
#
# Post-process the Tauri-built AppImage to remove the bundled libwayland-*
# libraries, then re-sign it.
#
# Why: linuxdeploy bundles libwayland-client/cursor/egl/server from the BUILD
# host into the AppImage. At runtime the user's host Mesa libEGL loads that
# bundled libwayland-client (it shadows the host's), and when the build-host
# wayland version differs from the user's, the wl_display handed to EGL is
# incompatible -> "Could not create default EGL display: EGL_BAD_PARAMETER.
# Aborting..." and a blank white window (seen on AMD/Wayland). libwayland is a
# host-provided library and must NOT be bundled. Removing it lets the host's
# own libwayland be used, which matches its Mesa/EGL stack.
#
# Confirmed empirically: dropping these 4 libs from a pipeline AppImage makes
# the EGL crash disappear on the AMD/Wayland host that reproduced it. See
# the dragal-launcher project, docs/linux-amd-egl-diagnosis.md.
#
# Re-signing: the original .AppImage.sig (produced by `tauri build`) no longer
# matches after repack, so we regenerate it with `tauri signer sign`, which
# reads the key + password from the TAURI_SIGNING_PRIVATE_KEY[_PASSWORD] env
# vars (already exported by the CI job). If the build produced no .sig (e.g. an
# unsigned local build), the re-sign step is skipped.
#
# Safe to run unconditionally after a build: a no-op when no AppImage exists.

set -euo pipefail

# AppImages self-extract without FUSE when this is set — needed in the CI
# Docker runner where /dev/fuse is unavailable.
export APPIMAGE_EXTRACT_AND_RUN=1

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

APPIMAGE_DIR="src-tauri/target/release/bundle/appimage"

# --- Locate the AppImage ----------------------------------------------------
shopt -s nullglob
appimages=("$APPIMAGE_DIR"/*.AppImage)
shopt -u nullglob
if [ ${#appimages[@]} -eq 0 ]; then
    echo "fix-appimage-wayland: no AppImage in $APPIMAGE_DIR — nothing to do."
    exit 0
fi
if [ ${#appimages[@]} -gt 1 ]; then
    echo "fix-appimage-wayland: WARNING — multiple AppImages found, processing all:"
    printf '  %s\n' "${appimages[@]}"
fi

# --- Resolve tauri CLI (for re-signing) ------------------------------------
# Invoke through `node` on the resolved JS entry rather than node_modules/.bin/
# tauri: the latter is a shell wrapper that needs its exec bit, which can be
# lost on some checkouts/filesystems. node-resolving only needs node + the
# installed package, both guaranteed here. Override with TAURI_BIN if needed.
TAURI_ENTRY=""
if [ -z "${TAURI_BIN:-}" ]; then
    TAURI_ENTRY="$(node -e "process.stdout.write(require.resolve('@tauri-apps/cli/tauri.js'))" 2>/dev/null || true)"
fi

run_tauri() {
    if [ -n "${TAURI_BIN:-}" ]; then
        "$TAURI_BIN" "$@"
    elif [ -n "$TAURI_ENTRY" ]; then
        node "$TAURI_ENTRY" "$@"
    else
        return 127
    fi
}

tauri_available() {
    [ -n "${TAURI_BIN:-}" ] || [ -n "$TAURI_ENTRY" ]
}

# --- Resolve appimagetool (cache + download) --------------------------------
APPIMAGETOOL="${APPIMAGETOOL:-}"
if [ -z "$APPIMAGETOOL" ]; then
    cache_dir="${XDG_CACHE_HOME:-$HOME/.cache}/gk-companion"
    APPIMAGETOOL="$cache_dir/appimagetool-x86_64.AppImage"
    if [ ! -x "$APPIMAGETOOL" ]; then
        mkdir -p "$cache_dir"
        echo "fix-appimage-wayland: downloading appimagetool..."
        curl --proto '=https' --tlsv1.2 -fsSL \
            -o "$APPIMAGETOOL" \
            "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
        chmod +x "$APPIMAGETOOL"
    fi
fi

# Libraries that must come from the host, never the bundle.
WAYLAND_LIBS=(
    libwayland-client.so.0
    libwayland-cursor.so.0
    libwayland-egl.so.1
    libwayland-server.so.0
)

process_one() {
    local appimage_rel="$1"
    local appimage; appimage="$(cd "$(dirname "$appimage_rel")" && pwd)/$(basename "$appimage_rel")"
    local sig="${appimage}.sig"
    echo ""
    echo "fix-appimage-wayland: processing $(basename "$appimage")"

    local work; work="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$work'" RETURN

    # Extract (--appimage-extract is built into the AppImage, no FUSE needed).
    ( cd "$work" && "$appimage" --appimage-extract >/dev/null )

    # Drop the host-provided wayland libs from the bundle.
    local removed=0 lib
    for lib in "${WAYLAND_LIBS[@]}"; do
        if [ -e "$work/squashfs-root/usr/lib/$lib" ]; then
            rm -f "$work/squashfs-root/usr/lib/$lib"
            echo "  removed usr/lib/$lib"
            removed=$((removed + 1))
        fi
    done
    if [ "$removed" -eq 0 ]; then
        echo "  no bundled libwayland found — leaving AppImage unchanged."
        return 0
    fi

    # Repack over the original path.
    ARCH=x86_64 "$APPIMAGETOOL" "$work/squashfs-root" "$work/out.AppImage" >/dev/null
    mv "$work/out.AppImage" "$appimage"
    chmod +x "$appimage"
    echo "  repacked $(basename "$appimage")"

    # Re-sign if the build had produced a signature.
    if [ -f "$sig" ]; then
        if ! tauri_available; then
            echo "  ERROR: a .sig exists but the tauri CLI was not found to re-sign." >&2
            echo "         Run 'npm ci' first, or set TAURI_BIN." >&2
            return 1
        fi
        if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]; then
            echo "  ERROR: a .sig exists but no signing key in env (TAURI_SIGNING_PRIVATE_KEY[_PATH])." >&2
            return 1
        fi
        # Key + password are read from the TAURI_SIGNING_PRIVATE_KEY[_PASSWORD]
        # env vars. `tauri signer sign` writes "<file>.sig" next to the input.
        run_tauri signer sign "$appimage" >/dev/null
        echo "  re-signed -> $(basename "$sig")"
    else
        echo "  no .sig alongside (unsigned build) — skipping re-sign."
    fi
}

rc=0
for img in "${appimages[@]}"; do
    process_one "$img" || rc=1
done

echo ""
if [ "$rc" -eq 0 ]; then
    echo "fix-appimage-wayland: done."
else
    echo "fix-appimage-wayland: completed with errors." >&2
fi
exit "$rc"
