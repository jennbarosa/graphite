#!/usr/bin/env python3
"""Installs the pinned tools the test suite runs, into tools/ (ignored by git):

  tools/bin/bun   Bun 1.4.4, for the TypeScript runtime tests
  tools/bin/purs  PureScript 0.15.16, the compiler for the generated pure modules
  tools/node_modules/.bin/{tsc,spago}  TypeScript 6.0.3 and spago 1.0.5, from tools/package-lock.json

Every download is checked against its SHA-256 digest. Needs npm and Node 22.5+ on the PATH.
Standard library only, so it runs unchanged on Linux and macOS (x86_64 and arm64).
"""

import hashlib
import os
import platform
import shutil
import stat
import subprocess
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path

BUN_VERSION = "1.4.4"
PURS_VERSION = "0.15.16"
BUN_BASE = f"https://github.com/oven-sh/bun/releases/download/bun-v{BUN_VERSION}"
PURS_BASE = f"https://github.com/purescript/purescript/releases/download/v{PURS_VERSION}"

# (system, machine) -> (bun asset, bun sha256, purs asset, purs sha256)
PLATFORMS = {
    ("Linux", "x86_64"): (
        "bun-linux-x64.zip",
        "bf8ccbfb2b0b80159e0f9a6d15c55317add8077534dbc6130a3edb52fccb395c",
        "linux64.tar.gz",
        "44da9efb8a4e14519e8fd5350acc377a4b981e42351a78c53e9f84045bf38e22",
    ),
    ("Linux", "aarch64"): (
        "bun-linux-aarch64.zip",
        "baa6bb36f51d6e58561e02bbea825038a9df454f673d6d776e8bbf86f8c7850e",
        "linux-arm64.tar.gz",
        "b8d153c5c6e0d8c9618a90824eb342b5818e14a92b0b6cdff753d9b353a927f3",
    ),
    ("Darwin", "x86_64"): (
        "bun-darwin-x64.zip",
        "50a3491f73abc2fe81673171a85762d3bc2cc1680ccecadd983de99adf77977e",
        "macos.tar.gz",
        "9f5bfdb7a468241c7106660bc6bd0209ce3f19ad197516ccdcd5c17df81958b6",
    ),
    ("Darwin", "arm64"): (
        "bun-darwin-aarch64.zip",
        "c240d7f0ccb90c263a849fcdb09b7ee62630d921348ed6aafda86b9d577bc510",
        "macos-arm64.tar.gz",
        "63f15a13e226efc260fb025c4a4ad7f66429b6685da607040b7bcc209332fcc4",
    ),
}

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / "tools"
DOWNLOADS = TOOLS / "downloads"
BIN = TOOLS / "bin"


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fetch(base: str, asset: str, sha: str) -> Path:
    """Downloads the asset unless a verified copy is already there, then checks its digest."""
    file = DOWNLOADS / asset
    if not file.is_file() or sha256_of(file) != sha:
        print(f"downloading {asset}")
        with urllib.request.urlopen(f"{base}/{asset}", timeout=120) as response:
            file.write_bytes(response.read())
    if sha256_of(file) != sha:
        sys.exit(f"checksum mismatch for {asset}; delete {file} and try again")
    return file


def find_binary(directory: Path, name: str) -> Path:
    for path in directory.rglob(name):
        if path.is_file():
            return path
    sys.exit(f"{name} not found in {directory}")


def install_binary(source: Path, target: Path) -> None:
    shutil.copyfile(source, target)
    target.chmod(target.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def unpack_zip(archive: Path, into: Path) -> None:
    shutil.rmtree(into, ignore_errors=True)
    with zipfile.ZipFile(archive) as zip_file:
        zip_file.extractall(into)


def unpack_tar(archive: Path, into: Path) -> None:
    shutil.rmtree(into, ignore_errors=True)
    into.mkdir(parents=True)
    with tarfile.open(archive, "r:gz") as tar_file:
        # The "data" filter refuses absolute paths and links that leave the target folder.
        tar_file.extractall(into, filter="data")


def main() -> None:
    key = (platform.system(), platform.machine())
    if key not in PLATFORMS:
        sys.exit(
            f"no pinned Bun or PureScript for {key[0]} {key[1]}; "
            "install them by hand (see CONTRIBUTING.md)"
        )
    bun_asset, bun_sha, purs_asset, purs_sha = PLATFORMS[key]

    DOWNLOADS.mkdir(parents=True, exist_ok=True)
    BIN.mkdir(parents=True, exist_ok=True)

    bun_archive = fetch(BUN_BASE, bun_asset, bun_sha)
    bun_dir = DOWNLOADS / "bun"
    unpack_zip(bun_archive, bun_dir)
    install_binary(find_binary(bun_dir, "bun"), BIN / "bun")

    purs_archive = fetch(PURS_BASE, purs_asset, purs_sha)
    purs_dir = DOWNLOADS / "purs"
    unpack_tar(purs_archive, purs_dir)
    install_binary(find_binary(purs_dir, "purs"), BIN / "purs")

    npm = shutil.which("npm")
    if npm is None:
        sys.exit("npm is not on the PATH; install Node 22.5+")
    subprocess.run(
        [npm, "ci", "--prefix", str(TOOLS), "--no-audit", "--no-fund"], check=True
    )

    print(f"\ninstalled into {TOOLS}:")
    versions = [
        [str(BIN / "bun"), "--version"],
        [str(BIN / "purs"), "--version"],
        [str(TOOLS / "node_modules" / ".bin" / "tsc"), "--version"],
        [str(TOOLS / "node_modules" / ".bin" / "spago"), "--version"],
    ]
    for command in versions:
        subprocess.run(command, check=True)


if __name__ == "__main__":
    main()
