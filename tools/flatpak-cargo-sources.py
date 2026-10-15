#!/usr/bin/env python3
"""Write Flatpak sources that let `cargo build --offline` run inside the sandbox.

Reads Cargo.lock and emits the same JSON shape that flatpak-cargo-generator produces:
one archive per crates.io crate, each with its .cargo-checksum.json, plus a
.cargo/config.toml that replaces crates.io with the vendored directory.

Only crates.io (registry) packages are supported. Git dependencies are rejected so the
build cannot silently reach the network. Nothing is downloaded: the checksums come
from Cargo.lock.

Usage:
    python3 tools/flatpak-cargo-sources.py [Cargo.lock] [-o cargo-sources.json]
    python3 tools/flatpak-cargo-sources.py --check   # exit 1 if the committed file is stale
"""

import argparse
import json
import sys
import tomllib
from pathlib import Path

CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"


def build_sources(lock_path: Path) -> list:
    lock = tomllib.loads(lock_path.read_text())
    sources = []
    seen = set()

    for pkg in lock.get("package", []):
        source = pkg.get("source")
        if source is None:
            continue  # workspace member
        if source != CRATES_IO:
            raise SystemExit(
                f"unsupported source for {pkg['name']} {pkg['version']}: {source}\n"
                "Only crates.io dependencies can be vendored offline."
            )
        name, version, checksum = pkg["name"], pkg["version"], pkg["checksum"]
        key = f"{name}-{version}"
        if key in seen:
            continue
        seen.add(key)
        dest = f"cargo/vendor/{key}"
        sources.append({
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
            "sha256": checksum,
            "dest": dest,
        })
        # Cargo checks vendored crates against this file. The package checksum is the
        # one from the lock; per-file checksums are left empty, as flatpak-cargo-generator does.
        sources.append({
            "type": "inline",
            "contents": json.dumps({"files": {}, "package": checksum}),
            "dest": dest,
            "dest-filename": ".cargo-checksum.json",
        })

    sources.append({
        "type": "inline",
        "contents": (
            '[source.crates-io]\n'
            'replace-with = "vendored-sources"\n\n'
            '[source.vendored-sources]\n'
            'directory = "cargo/vendor"\n'
        ),
        "dest": "cargo",
        "dest-filename": "config.toml",
    })
    return sources


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("lock", nargs="?", default="Cargo.lock")
    parser.add_argument("-o", "--output", default="spacepods-ui/cargo-sources.json")
    parser.add_argument("--check", action="store_true", help="fail if the output file is out of date")
    args = parser.parse_args()

    sources = build_sources(Path(args.lock))
    text = json.dumps(sources, indent=4) + "\n"
    if args.check:
        current = Path(args.output).read_text() if Path(args.output).exists() else ""
        if current != text:
            print(f"{args.output} is out of date with {args.lock}; regenerate it", file=sys.stderr)
            return 1
        print(f"{args.output} is up to date", file=sys.stderr)
        return 0
    Path(args.output).write_text(text)
    crates = sum(1 for s in sources if s.get("type") == "archive")
    print(f"wrote {args.output}: {crates} crates", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
