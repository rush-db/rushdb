#!/bin/sh
# RushDB installer.
#
#   curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
#
# Installs the `rushdb` binary (server and CLI) from a GitHub release
# after checking the archive's SHA-256 checksum (and its Sigstore signature
# when `cosign` is installed). Options, as flags after `sh -s --` or as
# environment variables:
#
#   --version <tag>     RUSHDB_VERSION        release tag, e.g. v3.0.0 (default: latest)
#   --dir <path>        RUSHDB_INSTALL_DIR    install directory (default: ~/.rushdb/bin)
#   --no-modify-path    RUSHDB_NO_MODIFY_PATH=1   do not edit shell startup files
#   --verify-signature  RUSHDB_VERIFY_SIGNATURE=1 require a cosign signature check
#
# RUSHDB_DOWNLOAD_URL replaces the release download base, for mirrors.
#
# Uninstall: delete the install directory and the line this script added to
# your shell startup file (marked "# rushdb").
set -eu

REPO="rush-db/rushdb"

say() { printf '%s\n' "$*"; }
err() {
  printf 'rushdb install: %s\n' "$*" >&2
  exit 1
}
need() { command -v "$1" >/dev/null 2>&1 || err "needs \`$1\`; install it and retry"; }

download() {
  if command -v curl >/dev/null 2>&1; then
    case "$1" in
      https://*) curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$2" "$1" ;;
      *) curl -fsSL --retry 3 -o "$2" "$1" ;;
    esac
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$2" "$1"
  else
    err "needs curl or wget"
  fi
}

# Linux builds link glibc 2.28 (RHEL 8, Debian 10, Ubuntu 18.10 and later).
check_glibc() {
  if ldd --version 2>&1 | grep -qi musl || ls /lib/ld-musl-* >/dev/null 2>&1; then
    err "musl-based systems (e.g. Alpine) are not supported by the prebuilt binaries; use the Docker image rushdb/rushdb"
  fi
  version="$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}')" || version=""
  [ -n "$version" ] || return 0
  major="${version%%.*}"
  minor="${version#*.}"
  minor="${minor%%.*}"
  if [ "$major" -lt 2 ] || { [ "$major" -eq 2 ] && [ "$minor" -lt 28 ]; }; then
    err "glibc $version is older than the required 2.28; use the Docker image or build from source"
  fi
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux)
      check_glibc
      case "$arch" in
        x86_64 | amd64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64 | arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) err "no prebuilt binary for Linux $arch; build from source: cargo install rushdb-server" ;;
      esac
      ;;
    Darwin)
      # A shell running under Rosetta reports x86_64 on Apple silicon.
      if [ "$arch" = "x86_64" ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = "1" ]; then
        arch="arm64"
      fi
      case "$arch" in
        arm64 | aarch64) echo "aarch64-apple-darwin" ;;
        x86_64) echo "x86_64-apple-darwin" ;;
        *) err "no prebuilt binary for macOS $arch" ;;
      esac
      ;;
    MINGW* | MSYS* | CYGWIN* | Windows_NT)
      err "Windows is not supported natively; use WSL 2 or the Docker image"
      ;;
    *) err "no prebuilt binary for $os; build from source: cargo install rushdb-server" ;;
  esac
}

# The newest engine release: the first stable vX.Y.Z tag, newest first. The
# repository also publishes package releases (e.g. SDKs), so GitHub's
# "latest" release is not necessarily the engine.
latest_tag() {
  tags="$(fetch_text "https://api.github.com/repos/$REPO/releases?per_page=100" 2>/dev/null |
    sed -n 's/.*"tag_name": *"\(v[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)".*/\1/p')" || tags=""
  tag="$(printf '%s\n' "$tags" | head -n 1)"
  if [ -z "$tag" ]; then
    # API unavailable (e.g. rate limited): follow the "latest" redirect.
    tag="$(fetch_location "https://github.com/$REPO/releases/latest" | sed -n 's|.*/releases/tag/\(v[0-9][0-9.]*\)$|\1|p')"
  fi
  [ -n "$tag" ] || err "cannot determine the latest release; set RUSHDB_VERSION (e.g. v3.0.0)"
  echo "$tag"
}

fetch_text() {
  if command -v curl >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -fsSL "$1"
  else
    wget -q -O - "$1"
  fi
}

fetch_location() {
  if command -v curl >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -fsSLI -o /dev/null -w '%{url_effective}' "$1" 2>/dev/null || true
  else
    wget -q --max-redirect=5 -S -O /dev/null "$1" 2>&1 | sed -n 's/^ *Location: *//p' | tail -n 1 | tr -d '\r'
  fi
}

verify_checksum() {
  (
    cd "$1"
    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum -c "$2.sha256" >/dev/null
    elif command -v shasum >/dev/null 2>&1; then
      shasum -a 256 -c "$2.sha256" >/dev/null
    else
      err "needs sha256sum or shasum to verify the download"
    fi
  ) || err "checksum mismatch for $2; the download is corrupt or was tampered with"
}

verify_signature() {
  if ! command -v cosign >/dev/null 2>&1; then
    [ "$require_signature" = "1" ] && err "--verify-signature needs cosign (https://docs.sigstore.dev)"
    return 0
  fi
  download "$base/$1.sigstore.json" "$2/$1.sigstore.json" ||
    err "cannot download the signature bundle for $1"
  cosign verify-blob \
    --bundle "$2/$1.sigstore.json" \
    --certificate-identity-regexp "^https://github.com/$REPO/" \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com \
    "$2/$1" >/dev/null 2>&1 || err "signature verification failed for $1"
  say "Verified the Sigstore signature."
}

add_to_path() {
  case ":$PATH:" in *":$1:"*) return 0 ;; esac
  line="export PATH=\"$1:\$PATH\" # rushdb"
  if [ "$modify_path" != "1" ]; then
    say ""
    say "Add RushDB to your PATH:"
    say "  $line"
    return 0
  fi
  case "$(basename "${SHELL:-sh}")" in
    zsh) profile="${ZDOTDIR:-$HOME}/.zshrc" ;;
    bash)
      if [ "$(uname -s)" = "Darwin" ]; then profile="$HOME/.bash_profile"; else profile="$HOME/.bashrc"; fi
      ;;
    fish)
      profile="$HOME/.config/fish/conf.d/rushdb.fish"
      line="fish_add_path \"$1\" # rushdb"
      mkdir -p "$(dirname "$profile")"
      ;;
    *) profile="$HOME/.profile" ;;
  esac
  if ! grep -qs "# rushdb" "$profile"; then
    printf '\n%s\n' "$line" >>"$profile"
    say "Added $1 to PATH in $profile."
  fi
  say "Open a new shell, or run:"
  say "  $line"
}

main() {
  version="${RUSHDB_VERSION:-latest}"
  install_dir="${RUSHDB_INSTALL_DIR:-${HOME:?HOME is not set}/.rushdb/bin}"
  modify_path=1
  [ "${RUSHDB_NO_MODIFY_PATH:-0}" = "1" ] && modify_path=0
  require_signature="${RUSHDB_VERIFY_SIGNATURE:-0}"

  while [ $# -gt 0 ]; do
    case "$1" in
      --version) [ $# -ge 2 ] || err "--version needs a tag"; version="$2"; shift 2 ;;
      --dir) [ $# -ge 2 ] || err "--dir needs a path"; install_dir="$2"; shift 2 ;;
      --no-modify-path) modify_path=0; shift ;;
      --verify-signature) require_signature=1; shift ;;
      -h | --help) sed -n '2,/^set -eu$/{/^#/s/^# \{0,1\}//p;}' "$0" 2>/dev/null || true; exit 0 ;;
      *) err "unknown option $1" ;;
    esac
  done

  need uname
  need tar
  target="$(detect_target)"
  archive="rushdb-$target.tar.gz"
  if [ -n "${RUSHDB_DOWNLOAD_URL:-}" ]; then
    base="$RUSHDB_DOWNLOAD_URL"
  else
    case "$version" in
      latest) version="$(latest_tag)" ;;
      v*) ;;
      *) version="v$version" ;;
    esac
    base="https://github.com/$REPO/releases/download/$version"
  fi

  tmp="$(mktemp -d 2>/dev/null || mktemp -d -t rushdb)"
  trap 'rm -rf "$tmp"' EXIT INT TERM

  say "Downloading RushDB ($version) for $target..."
  download "$base/$archive" "$tmp/$archive" || err "cannot download $base/$archive"
  download "$base/$archive.sha256" "$tmp/$archive.sha256" || err "cannot download the checksum for $archive"
  verify_checksum "$tmp" "$archive"
  verify_signature "$archive" "$tmp"

  mkdir -p "$tmp/unpacked" "$install_dir"
  tar -xzf "$tmp/$archive" -C "$tmp/unpacked"
  [ -f "$tmp/unpacked/rushdb" ] || err "the archive has no rushdb"
  # Replace through a rename so a running server keeps its old binary.
  cp "$tmp/unpacked/rushdb" "$install_dir/.rushdb.new"
  chmod 755 "$install_dir/.rushdb.new"
  mv -f "$install_dir/.rushdb.new" "$install_dir/rushdb"

  installed="$("$install_dir/rushdb" --version 2>/dev/null)" || err "the installed rushdb does not run on this system"
  say "Installed $installed to $install_dir."
  add_to_path "$install_dir"
  say ""
  say "Start a local database:"
  say "  rushdb up"
  say "Docs: https://github.com/$REPO/blob/main/docs/manual/getting-started/README.md"
}

main "$@"
