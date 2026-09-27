#!/usr/bin/env python3
"""Builds the registry crc reads: dist/index.json and one .wasm per extension.

Run after `cargo build --release --target wasm32-unknown-unknown`. Checks
every manifest against the rules crc enforces, so a bad extension fails
here, in CI, and never reaches anyone. Standard library only.

Usage: scripts/build-registry.py [--out dist]
"""

import hashlib
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
WASM_DIR = ROOT / "target" / "wasm32-unknown-unknown" / "release"
API = 1
# Exactly what crc's `Capability::parse` accepts (src/ext/manifest.rs in
# the crc repository). A name crc does not know would pass here and then be
# skipped by every crc, silently.
CAPABILITIES = {
    "selection.read",
    "selection.replace",
    "document.read",
    "document.edit",
    "preview.show",
}
# crc's `MAX_ID_LEN`: the id names a folder on the user's disk.
MAX_ID_LEN = 100
# The host functions crc links, as crc's `HOST_FUNCTIONS`.
HOST_FUNCTIONS = {"log"}
ID = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*(\.[a-z0-9]+(-[a-z0-9]+)*)+$")
VERSION = re.compile(r"^\d+\.\d+\.\d+$")
COMMAND = re.compile(r"^[a-z][a-z0-9_]*$")
REQUIRED = ["memory", "crc_alloc", "crc_free"]
ICONS = {
    line.strip()
    for line in (ROOT / "scripts" / "icons.txt").read_text().splitlines()
    if line.strip() and not line.startswith("#")
}


def fail(where, why):
    sys.exit(f"{where}: {why}")


def leb(data, at):
    value, shift = 0, 0
    while True:
        byte = data[at]
        at += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, at
        shift += 7


def wasm_exports_and_imports(data):
    """The export names, and the (module, name) of every import."""
    if data[:8] != b"\0asm\x01\0\0\0":
        raise ValueError("not a WebAssembly 1 module")
    exports, imports, at = [], [], 8
    while at < len(data):
        section = data[at]
        size, at = leb(data, at + 1)
        body, end = at, at + size
        if section == 2:
            count, body = leb(data, body)
            for _ in range(count):
                n, body = leb(data, body)
                module = data[body : body + n].decode()
                body += n
                n, body = leb(data, body)
                name = data[body : body + n].decode()
                body += n
                kind = data[body]
                body += 1
                if kind == 0:
                    _, body = leb(data, body)
                else:
                    raise ValueError(f"imports {module}.{name}, which is not a function")
                imports.append((module, name))
        elif section == 7:
            count, body = leb(data, body)
            for _ in range(count):
                n, body = leb(data, body)
                exports.append(data[body : body + n].decode())
                body += n + 1
                _, body = leb(data, body)
        at = end
    return exports, imports


def main():
    out = ROOT / (sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else "dist")
    out.mkdir(parents=True, exist_ok=True)
    entries, seen = [], set()
    for folder in sorted((ROOT / "extensions").iterdir()):
        manifest_path = folder / "manifest.json"
        if not manifest_path.is_file():
            continue
        where = folder.relative_to(ROOT)
        m = json.loads(manifest_path.read_text())
        for key in ["id", "name", "version", "description", "license", "api", "entry", "capabilities", "commands"]:
            if key not in m:
                fail(where, f"manifest has no {key!r}")
        if not ID.match(m["id"]) or len(m["id"]) > MAX_ID_LEN:
            fail(where, f"id {m['id']!r} is not like 'author.name' (at most {MAX_ID_LEN} characters)")
        if m["id"] in seen:
            fail(where, f"id {m['id']!r} is used twice")
        seen.add(m["id"])
        if not VERSION.match(m["version"]):
            fail(where, f"version {m['version']!r} is not x.y.z")
        if m["api"] != API:
            fail(where, f"api {m['api']} is not {API}")
        unknown = set(m["capabilities"]) - CAPABILITIES
        if unknown:
            fail(where, f"unknown capabilities {sorted(unknown)}")
        if not m["commands"]:
            fail(where, "no commands")
        # crc's Cmd-E runs the command named `preview`, on the whole document.
        if "preview.show" in m["capabilities"]:
            if not any(c.get("id") == "preview" for c in m["commands"]):
                fail(where, "preview.show needs a command with the id 'preview'")
            if "document.read" not in m["capabilities"]:
                fail(where, "preview.show needs document.read: a preview is of the whole document")
        if m.get("icon", "extensions") not in ICONS:
            fail(where, f"unknown icon {m.get('icon')!r}; see scripts/icons.txt")
        if "homepage" in m and not str(m["homepage"]).startswith("https://"):
            fail(where, "homepage must be an https address")
        readme = folder / "README.md"
        if not readme.is_file():
            fail(where, "no README.md (it is shown before install)")
        wasm_path = WASM_DIR / m["entry"]
        if not wasm_path.is_file():
            fail(where, f"{m['entry']} was not built")
        data = wasm_path.read_bytes()
        try:
            exports, imports = wasm_exports_and_imports(data)
        except (ValueError, IndexError, UnicodeDecodeError) as e:
            fail(where, f"{m['entry']}: {e}")
        for name in REQUIRED:
            if name not in exports:
                fail(where, f"{m['entry']} does not export {name}")
        command_ids = [c.get("id", "") for c in m["commands"]]
        if len(set(command_ids)) != len(command_ids):
            fail(where, "a command id is listed twice")
        for command in m["commands"]:
            if not COMMAND.match(command.get("id", "")) or not command.get("title"):
                fail(where, f"bad command {command}")
            if command["id"] not in exports:
                fail(where, f"command {command['id']!r} is not exported by {m['entry']}")
        # Only crc's own host functions; which of them get linked is up to
        # the capabilities, at load time.
        foreign = [
            f"{mod}.{name}" for mod, name in imports if mod != "crc" or name not in HOST_FUNCTIONS
        ]
        if foreign:
            fail(where, f"imports from outside crc: {foreign}")
        name = f"{m['id']}-{m['version']}.wasm"
        (out / name).write_bytes(data)
        entry = dict(m)
        entry["wasm"] = name
        entry["sha256"] = hashlib.sha256(data).hexdigest()
        entry["size"] = len(data)
        entry["readme"] = readme.read_text()
        entries.append(entry)
        print(f"{m['id']} {m['version']}: {len(data)} bytes, {len(m['commands'])} commands")
    # Rises with every publish, so crc can refuse an older signed list served
    # again as the latest. The commit time of what is built: reproducible,
    # and later for every later commit.
    serial = int(
        subprocess.run(
            ["git", "-C", str(ROOT), "log", "-1", "--format=%ct"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )
    index = {"api": API, "serial": serial, "extensions": entries}
    (out / "index.json").write_text(json.dumps(index, indent=2, ensure_ascii=False) + "\n")
    shown = out.relative_to(ROOT) if out.is_relative_to(ROOT) else out
    print(f"{shown}/index.json: {len(entries)} extensions")


if __name__ == "__main__":
    main()
