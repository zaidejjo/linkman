#!/usr/bin/env bash
# Build the distributable archives for one Rust target.
#
#   usage: packaging/package.sh <rust-target> [output-dir]
#
# Environment:
#   GITHUB_REF_NAME / LINKMAN_VERSION   version to stamp (defaults to Cargo.toml)
#   LINKMAN_DEB=1                       also build the Debian package (Linux x86_64 only)
#   LINKMAN_COMPLETIONS_DIR=<dir>       pre-generated completions; needed when the
#                                       built binary cannot run on this host
#                                       (cross builds such as aarch64 on x86_64)
set -euo pipefail

die() {
  echo "$*" >&2
  exit 1
}

target="${1:?usage: package.sh <rust-target> [output-dir]}"
out="${2:-dist}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$(dirname "$script_dir")"

version="${LINKMAN_VERSION:-${GITHUB_REF_NAME:-}}"
version="${version#v}"
if [[ -z "$version" ]]; then
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
fi

bin="target/${target}/release/linkman"
[[ -f "$bin" ]] || die "missing ${bin} — run: cargo build --release --locked --target ${target}"

# Shell completions come from the CLI definition, so they are identical for
# every target. Generate them by running the binary when we can, otherwise
# take them from a pre-generated directory (cross-built binaries cannot run).
completions_tmp=""
if [[ -n "${LINKMAN_COMPLETIONS_DIR:-}" && -d "${LINKMAN_COMPLETIONS_DIR}" ]]; then
  completions="${LINKMAN_COMPLETIONS_DIR}"
  for file in linkman.bash _linkman linkman.fish; do
    [[ -f "${completions}/${file}" ]] || die "missing ${completions}/${file}"
  done
elif "$bin" --version >/dev/null 2>&1; then
  completions_tmp="$(mktemp -d)"
  completions="$completions_tmp"
  "$bin" completions bash > "$completions/linkman.bash"
  "$bin" completions zsh > "$completions/_linkman"
  "$bin" completions fish > "$completions/linkman.fish"
else
  die "cannot run ${bin} on this host — set LINKMAN_COMPLETIONS_DIR to pre-generated completions"
fi
trap '[[ -n "$completions_tmp" ]] && rm -rf "$completions_tmp"' EXIT

name="linkman-${version}-${target}"
stage="${out}/${name}"
rm -rf "$stage"
mkdir -p "$stage/completions"

cp "$bin" "$stage/linkman"
cp README.md LICENSE "$stage/"
cp "$completions/linkman.bash" "$completions/_linkman" "$completions/linkman.fish" "$stage/completions/"

tar -C "$out" -czf "${out}/${name}.tar.gz" "$name"
rm -rf "$stage"
echo "wrote ${out}/${name}.tar.gz"

if [[ "${LINKMAN_DEB:-0}" == "1" ]]; then
  command -v dpkg-deb >/dev/null || die "dpkg-deb not available"
  deb="${out}/deb"
  rm -rf "$deb"
  install -Dm755 "$bin" "$deb/usr/bin/linkman"
  mkdir -p "$deb/DEBIAN" \
    "$deb/usr/share/bash-completion/completions" \
    "$deb/usr/share/zsh/site-functions" \
    "$deb/usr/share/fish/vendor_completions.d" \
    "$deb/usr/share/doc/linkman"
  cp "$completions/linkman.bash" "$deb/usr/share/bash-completion/completions/linkman"
  cp "$completions/_linkman" "$deb/usr/share/zsh/site-functions/_linkman"
  cp "$completions/linkman.fish" "$deb/usr/share/fish/vendor_completions.d/linkman.fish"
  install -Dm644 README.md "$deb/usr/share/doc/linkman/README.md"
  install -Dm644 LICENSE "$deb/usr/share/doc/linkman/copyright"

  cat > "$deb/DEBIAN/control" <<EOF
Package: linkman
Version: ${version}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: linkman developers <zaidejjo@users.noreply.github.com>
Homepage: https://github.com/zaidejjo/linkman
Depends: libc6
Description: symlink studio — TUI and CLI for managing symlinks
 linkman is a fast, beautiful TUI and CLI for creating, retargeting,
 backing up and removing symlinks, with fuzzy search, undo/redo and
 selectable colour themes.
EOF

  dpkg-deb --build --root-owner-group "$deb" "${out}/linkman_${version}_amd64.deb"
  rm -rf "$deb"
  echo "wrote ${out}/linkman_${version}_amd64.deb"
fi
