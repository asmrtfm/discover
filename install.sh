#!/usr/bin/env bash
set -euo pipefail

# install.sh — idempotent installer for discover + static ast-grep binaries.
#
# Downloads the latest GitHub Release (no local build, no Rust toolchain) and
# installs to user scope. Safe to re-run: exits 0 without changes when the
# installed version already matches the latest release.
#
#   curl -fsSL https://github.com/asmrtfm/discover/releases/latest/download/install.sh | bash
#
# Requirements: bash 3.2+, curl, tar. Runs on Linux, macOS, and WSL.
# On native Windows use install.ps1 instead.
#
# Environment overrides:
#   DISCOVER_REPO    GitHub repo to install from   (default: asmrtfm/discover)
#   DISCOVER_PREFIX  Install prefix                (default: ~/.local)

REPO="${DISCOVER_REPO:-asmrtfm/discover}"
PREFIX="${DISCOVER_PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
SHARE_DIR="$PREFIX/share/discover"

GITHUB_URL="https://github.com"
RELEASES_URL="$GITHUB_URL/$REPO/releases"
JQ_LATEST_URL="$GITHUB_URL/jqlang/jq/releases/latest/download"

CHECKSUM_FILE="SHA256SUMS"
VERSION_STAMP="$SHARE_DIR/VERSION"

die() { echo "error: $*" >&2; exit 1; }
info() { echo "==> $*"; }

usage() {
  cat <<EOF
Usage: install.sh [OPTIONS]

Install discover and statically-linked ast-grep/sg from GitHub Releases.

Options:
  --version TAG  Install a specific release tag (default: latest)
  -f, --force    Reinstall even if the installed version is current
  -h, --help     Show this help

Files:
  $BIN_DIR/{discover,ast-grep,sg}   commands (jq added if absent)
  $SHARE_DIR/                       framework tree + VERSION stamp
EOF
}

FORCE=0
REQUESTED_TAG=""
while [ $# -gt 0 ]; do
  case "$1" in
    --version) REQUESTED_TAG="$2"; shift 2 ;;
    -f|--force) FORCE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown option: $1 (see --help)" ;;
  esac
done

# ── Platform detection ──────────────────────────────────────────────
os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)  platform_os="linux" ;;
  Darwin) platform_os="macos" ;;
  MINGW*|MSYS*|CYGWIN*)
    die "native Windows detected — use install.ps1:
  irm $RELEASES_URL/latest/download/install.ps1 | iex" ;;
  *) die "unsupported OS: $os" ;;
esac

case "$arch" in
  x86_64|amd64)  platform_arch="x86_64";  jq_arch="amd64" ;;
  aarch64|arm64) platform_arch="aarch64"; jq_arch="arm64" ;;
  *) die "unsupported architecture: $arch" ;;
esac

platform="${platform_os}-${platform_arch}"

# ── Resolve release tag ─────────────────────────────────────────────
# The /releases/latest redirect avoids api.github.com's anonymous rate limit.
if [ -n "$REQUESTED_TAG" ]; then
  tag="$REQUESTED_TAG"
else
  redirect="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$RELEASES_URL/latest")" \
    || die "could not reach $RELEASES_URL/latest"
  tag="${redirect##*/}"
fi
case "$tag" in
  v[0-9]*) ;;
  *) die "could not resolve a release tag from $RELEASES_URL/latest (got '$tag')" ;;
esac
version="${tag#v}"

# ── Idempotency check ───────────────────────────────────────────────
if [ "$FORCE" -eq 0 ] && [ -f "$VERSION_STAMP" ] && [ "$(cat "$VERSION_STAMP")" = "$tag" ]; then
  info "discover $tag already installed — up to date"
  exit 0
fi

# ── Download and verify ─────────────────────────────────────────────
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

binaries_archive="ast-grep-${version}-${platform}.tar.gz"
framework_archive="discover-${version}.tar.gz"
download_url="$RELEASES_URL/download/$tag"

info "Downloading discover $tag for $platform"
for asset in "$binaries_archive" "$framework_archive" "$CHECKSUM_FILE"; do
  curl -fsSL -o "$WORK_DIR/$asset" "$download_url/$asset" \
    || die "download failed: $download_url/$asset"
done

checksum_check() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$1"
  else
    shasum -a 256 -c "$1"
  fi
}

info "Verifying checksums"
: > "$WORK_DIR/$CHECKSUM_FILE.local"
for asset in "$binaries_archive" "$framework_archive"; do
  grep "  ${asset}\$" "$WORK_DIR/$CHECKSUM_FILE" >> "$WORK_DIR/$CHECKSUM_FILE.local" \
    || die "$asset not listed in $CHECKSUM_FILE"
done
(cd "$WORK_DIR" && checksum_check "$CHECKSUM_FILE.local" >/dev/null) \
  || die "checksum verification failed"

# ── Install ─────────────────────────────────────────────────────────
mkdir -p "$BIN_DIR"

tar -xzf "$WORK_DIR/$binaries_archive" -C "$WORK_DIR"
for bin_name in ast-grep sg; do
  install -m 755 "$WORK_DIR/ast-grep-${version}-${platform}/$bin_name" "$BIN_DIR/$bin_name"
done

tar -xzf "$WORK_DIR/$framework_archive" -C "$WORK_DIR"
rm -rf "$SHARE_DIR"
mkdir -p "$(dirname "$SHARE_DIR")"
mv "$WORK_DIR/discover-${version}" "$SHARE_DIR"

# Wrapper rather than symlink: pre-sets the root so bin/discover works even
# where realpath is unavailable (macOS < 13)
cat > "$BIN_DIR/discover" <<EOF
#!/usr/bin/env bash
# Installed by discover's install.sh — re-run it to update.
DISCOVER_ROOT="$SHARE_DIR" SCRIPT_DIR="$SHARE_DIR/bin" exec bash "$SHARE_DIR/bin/discover" "\$@"
EOF
chmod +x "$BIN_DIR/discover"

# discover requires jq at runtime; provide a static build if the system lacks it
if ! command -v jq >/dev/null 2>&1 && [ ! -x "$BIN_DIR/jq" ]; then
  info "jq not found — installing static jq to $BIN_DIR/jq"
  curl -fsSL -o "$BIN_DIR/jq" "$JQ_LATEST_URL/jq-${platform_os}-${jq_arch}" \
    || die "jq download failed"
  chmod +x "$BIN_DIR/jq"
fi

info "Installed discover $tag"
echo "    $BIN_DIR/discover"
echo "    $BIN_DIR/ast-grep"
echo "    $BIN_DIR/sg"
echo "    $SHARE_DIR/"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    echo ""
    echo "NOTE: $BIN_DIR is not on your PATH. Add it, e.g.:"
    echo "  export PATH=\"$BIN_DIR:\$PATH\""
    ;;
esac
