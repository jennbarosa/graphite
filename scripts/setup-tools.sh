#!/usr/bin/env bash
# Installs the pinned tools the test suite runs, into tools/ (ignored by git):
#   tools/bin/bun   Bun 1.4.4, for the TypeScript runtime tests
#   tools/bin/purs  PureScript 0.15.16, the compiler for the generated pure modules
#   tools/node_modules/.bin/{tsc,spago}  TypeScript 6.0.3 and spago 1.0.5, from tools/package-lock.json
# Every download is checked against its SHA-256 digest. Needs curl, unzip, tar, npm and Node 22.5+.
set -euo pipefail

cd "$(dirname "$0")/.."
root="$PWD/tools"
downloads="$root/downloads"
mkdir -p "$root/bin" "$downloads"

bun_version=1.4.4
purs_version=0.15.16
bun_base="https://github.com/oven-sh/bun/releases/download/bun-v$bun_version"
purs_base="https://github.com/purescript/purescript/releases/download/v$purs_version"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)
    bun_asset=bun-linux-x64.zip
    bun_sha=bf8ccbfb2b0b80159e0f9a6d15c55317add8077534dbc6130a3edb52fccb395c
    purs_asset=linux64.tar.gz
    purs_sha=44da9efb8a4e14519e8fd5350acc377a4b981e42351a78c53e9f84045bf38e22
    ;;
  Linux-aarch64)
    bun_asset=bun-linux-aarch64.zip
    bun_sha=baa6bb36f51d6e58561e02bbea825038a9df454f673d6d776e8bbf86f8c7850e
    purs_asset=linux-arm64.tar.gz
    purs_sha=b8d153c5c6e0d8c9618a90824eb342b5818e14a92b0b6cdff753d9b353a927f3
    ;;
  Darwin-x86_64)
    bun_asset=bun-darwin-x64.zip
    bun_sha=50a3491f73abc2fe81673171a85762d3bc2cc1680ccecadd983de99adf77977e
    purs_asset=macos.tar.gz
    purs_sha=9f5bfdb7a468241c7106660bc6bd0209ce3f19ad197516ccdcd5c17df81958b6
    ;;
  Darwin-arm64)
    bun_asset=bun-darwin-aarch64.zip
    bun_sha=c240d7f0ccb90c263a849fcdb09b7ee62630d921348ed6aafda86b9d577bc510
    purs_asset=macos-arm64.tar.gz
    purs_sha=63f15a13e226efc260fb025c4a4ad7f66429b6685da607040b7bcc209332fcc4
    ;;
  *)
    echo "no pinned Bun or PureScript for $(uname -s) $(uname -m); install them by hand (see CONTRIBUTING.md)" >&2
    exit 1
    ;;
esac

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

# fetch <base url> <asset> <sha256>: downloads the asset unless a verified copy is already there.
fetch() {
  local file="$downloads/$2"
  if [ ! -f "$file" ] || [ "$(sha256_of "$file")" != "$3" ]; then
    echo "downloading $2"
    curl -fsSL -o "$file" "$1/$2"
  fi
  if [ "$(sha256_of "$file")" != "$3" ]; then
    echo "checksum mismatch for $2; delete $file and try again" >&2
    exit 1
  fi
}

fetch "$bun_base" "$bun_asset" "$bun_sha"
rm -rf "$downloads/bun"
unzip -q -o "$downloads/$bun_asset" -d "$downloads/bun"
cp "$(find "$downloads/bun" -type f -name bun | head -n 1)" "$root/bin/bun"
chmod +x "$root/bin/bun"

fetch "$purs_base" "$purs_asset" "$purs_sha"
rm -rf "$downloads/purs"
mkdir -p "$downloads/purs"
tar -xzf "$downloads/$purs_asset" -C "$downloads/purs"
cp "$(find "$downloads/purs" -type f -name purs | head -n 1)" "$root/bin/purs"
chmod +x "$root/bin/purs"

npm ci --prefix "$root" --no-audit --no-fund

echo
echo "installed into $root:"
"$root/bin/bun" --version
"$root/bin/purs" --version
"$root/node_modules/.bin/tsc" --version
"$root/node_modules/.bin/spago" --version
