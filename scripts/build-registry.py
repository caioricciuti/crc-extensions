#!/usr/bin/env python3
"""Builds the registry crc reads: dist/index.json and one .wasm per extension.

Run after `cargo build --release --target wasm32-unknown-unknown`. Checks
every manifest against the rules crc enforces, so a bad extension fails
here, in CI, and never reaches anyone. Standard library only.

Usage: scripts/build-registry.py [--out dist] [--previous index.json]
       scripts/build-registry.py --check-contract scripts/contract/manifests.json

--previous is the published index to compare with: an extension at the same
version as there must be built from the same source. A changed module under
an old version never reaches the people who installed that version.
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
# skipped by every crc. scripts/contract/manifests.json, crc's own cases,
# holds this file and crc to the same verdicts.
CAPABILITIES = {
    "selection.read",
    "selection.replace",
    "document.read",
    "document.edit",
    "preview.show",
}
# crc's `MAX_ID_LEN`: the id names a folder on the user's disk.
MAX_ID_LEN = 100
# The host functions crc links, as crc's `HOST_FUNCTIONS`: name, then the
# number of parameters and results.
HOST_FUNCTIONS = {"log": (2, 0)}
# Whole-string matches only (fullmatch), ASCII only: `$` alone accepts a
# trailing newline and `\d` accepts any script's digits, which crc does not.
ID = re.compile(r"[a-z0-9]+(-[a-z0-9]+)*(\.[a-z0-9]+(-[a-z0-9]+)*)+", re.ASCII)
VERSION = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+", re.ASCII)
COMMAND = re.compile(r"[a-z][a-z0-9_]*", re.ASCII)
# The exports crc calls besides the commands, and their shapes; every
# command is called with (ptr, len) and answers one value.
REQUIRED = {"crc_alloc": (1, 1), "crc_free": (2, 0)}
COMMAND_SHAPE = (2, 1)
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


def wasm_interface(data):
    """What crc checks of a module: the exports by name, as (kind, shape),
    and every import as (module, name, shape). A shape is the number of
    parameters and results of a function, None for anything else."""
    if data[:8] != b"\0asm\x01\0\0\0":
        raise ValueError("not a WebAssembly 1 module")
    types, funcs, imports, raw_exports, at = [], [], [], [], 8
    while at < len(data):
        section = data[at]
        size, at = leb(data, at + 1)
        body, end = at, at + size
        if end > len(data):
            raise ValueError("a section runs past the end of the module")
        if section == 1:
            count, body = leb(data, body)
            for _ in range(count):
                if data[body] != 0x60:
                    raise ValueError("a type that is not a function")
                params, body = leb(data, body + 1)
                body += params
                results, body = leb(data, body)
                body += results
                types.append((params, results))
        elif section == 2:
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
                if kind != 0:
                    raise ValueError(f"imports {module}.{name}, which is not a function")
                index, body = leb(data, body)
                funcs.append(index)
                imports.append((module, name, index))
        elif section == 3:
            count, body = leb(data, body)
            for _ in range(count):
                index, body = leb(data, body)
                funcs.append(index)
        elif section == 7:
            count, body = leb(data, body)
            for _ in range(count):
                n, body = leb(data, body)
                name = data[body : body + n].decode()
                kind = data[body + n]
                index, body = leb(data, body + n + 1)
                raw_exports.append((name, kind, index))
        at = end

    def shape(func):
        return types[funcs[func]] if func < len(funcs) and funcs[func] < len(types) else None

    exports = {
        name: (kind, shape(index) if kind == 0 else None) for name, kind, index in raw_exports
    }
    imports = [
        (module, name, types[index] if index < len(types) else None)
        for module, name, index in imports
    ]
    return exports, imports


# Everything outside an extension's folder that shapes its module's bytes.
SHARED_SOURCES = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "crc-extension"]


def source_digest(folder):
    """SHA-256 over every file that goes into `folder`'s module: its own
    files and the shared ones, by path and content. The same on any machine,
    where the module's bytes are not."""
    h = hashlib.sha256()
    paths = []
    for base in [folder] + [ROOT / s for s in SHARED_SOURCES]:
        if base.is_file():
            paths.append(base)
        else:
            paths.extend(p for p in base.rglob("*") if p.is_file() and "target" not in p.parts)
    for path in sorted(paths, key=lambda p: p.relative_to(ROOT).as_posix()):
        h.update(path.relative_to(ROOT).as_posix().encode() + b"\0")
        h.update(hashlib.sha256(path.read_bytes()).digest())
    return h.hexdigest()


def check_manifest(m):
    """The first rule `m` breaks, or None: the same rules as crc's
    manifest::parse, which the shared contract in scripts/contract/ holds
    both to."""
    if not isinstance(m, dict):
        return "the manifest is not an object"
    for key in ["id", "name", "version", "description", "license", "entry"]:
        if not isinstance(m.get(key), str):
            return f"manifest has no {key!r} string"
    if not ID.fullmatch(m["id"]) or len(m["id"]) > MAX_ID_LEN:
        return f"id {m['id']!r} is not like 'author.name' (at most {MAX_ID_LEN} characters)"
    if not VERSION.fullmatch(m["version"]):
        return f"version {m['version']!r} is not x.y.z"
    # bool is an int in Python; `true` is not an api.
    if type(m.get("api")) is not int or m["api"] != API:
        return f"api {m.get('api')!r} is not {API}"
    entry = m["entry"]
    if "/" in entry or "\\" in entry or not entry.endswith(".wasm"):
        return f"entry {entry!r} is not a .wasm file name"
    capabilities = m.get("capabilities")
    if not isinstance(capabilities, list):
        return "manifest has no capabilities list"
    unknown = [c for c in capabilities if not isinstance(c, str) or c not in CAPABILITIES]
    if unknown:
        return f"unknown capabilities {unknown}"
    commands = m.get("commands")
    if not isinstance(commands, list) or not commands:
        return "no commands"
    ids = []
    for command in commands:
        if not isinstance(command, dict):
            return f"bad command {command!r}"
        cid, title = command.get("id"), command.get("title")
        if not isinstance(cid, str) or not COMMAND.fullmatch(cid):
            return f"command id {cid!r} is not like sort_lines"
        if not isinstance(title, str) or not title.strip():
            return f"command {cid!r} has no title"
        if cid in ids:
            return f"the command {cid} is listed twice"
        ids.append(cid)
    # crc's Cmd-E runs the command named `preview`, on the whole document.
    if "preview.show" in capabilities:
        if "preview" not in ids:
            return "preview.show needs a command with the id 'preview'"
        if "document.read" not in capabilities:
            return "preview.show needs document.read: a preview is of the whole document"
    if m.get("icon", "extensions") not in ICONS:
        return f"unknown icon {m.get('icon')!r}; see scripts/icons.txt"
    if "homepage" in m and not (
        isinstance(m["homepage"], str) and m["homepage"].startswith("https://")
    ):
        return "homepage must be an https address"
    return None


def check_contract(path):
    """Runs every case of the shared contract; exits non-zero on any whose
    verdict differs from crc's."""
    corpus = json.loads(pathlib.Path(path).read_text())
    wrong = []
    for case in corpus["cases"]:
        m = dict(corpus["base"])
        for key, value in case["set"].items():
            if value is None:
                m.pop(key, None)
            else:
                m[key] = value
        error = check_manifest(m)
        if (error is None) != case["ok"]:
            wrong.append(f"{case['name']}: crc says {'ok' if case['ok'] else 'refused'}, "
                         f"this says {error or 'ok'}")
    if wrong:
        sys.exit("\n".join(wrong))
    print(f"{path}: {len(corpus['cases'])} cases agree with crc")


def check_versions(entries, previous_path):
    """Fails when an extension kept its published version but not its
    source: crc keeps an installed version as it is, so a changed module
    under an old number would never reach anyone who has it."""
    previous = {
        (e.get("id"), e.get("version")): e.get("source_sha256")
        for e in json.loads(pathlib.Path(previous_path).read_text()).get("extensions", [])
    }
    stale = [
        f"{e['id']} {e['version']}"
        for e in entries
        # Entries published before sources were recorded cannot be compared.
        if previous.get((e["id"], e["version"])) not in (None, e["source_sha256"])
    ]
    if stale:
        sys.exit(
            "changed since they were published at the same version; bump each version:\n  "
            + "\n  ".join(stale)
        )
    print(f"versions: every extension at a published version has its published source")


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
        error = check_manifest(m)
        if error:
            fail(where, error)
        if m["id"] in seen:
            fail(where, f"id {m['id']!r} is used twice")
        seen.add(m["id"])
        readme = folder / "README.md"
        if not readme.is_file():
            fail(where, "no README.md (it is shown before install)")
        wasm_path = WASM_DIR / m["entry"]
        if not wasm_path.is_file():
            fail(where, f"{m['entry']} was not built")
        data = wasm_path.read_bytes()
        try:
            exports, imports = wasm_interface(data)
        except (ValueError, IndexError, UnicodeDecodeError) as e:
            fail(where, f"{m['entry']}: {e}")
        if "memory" not in exports:
            fail(where, f"{m['entry']} does not export memory")
        # The shapes crc calls them with: a module that declares another
        # would be handed the wrong number of values, and crc refuses it.
        wanted = dict(REQUIRED)
        wanted.update({c["id"]: COMMAND_SHAPE for c in m["commands"]})
        for name, shape in wanted.items():
            if name not in exports:
                fail(where, f"{name} is not exported by {m['entry']}")
            if exports[name] != (0, shape):
                fail(where, f"{name} in {m['entry']} is not a function taking "
                            f"{shape[0]} and returning {shape[1]}")
        # Only crc's own host functions, with their shapes; which of them
        # get linked is up to the capabilities, at load time.
        foreign = [
            f"{mod}.{name}"
            for mod, name, shape in imports
            if mod != "crc" or HOST_FUNCTIONS.get(name) != shape
        ]
        if foreign:
            fail(where, f"imports crc does not provide, or with the wrong shape: {foreign}")
        name = f"{m['id']}-{m['version']}.wasm"
        (out / name).write_bytes(data)
        entry = dict(m)
        entry["wasm"] = name
        entry["sha256"] = hashlib.sha256(data).hexdigest()
        entry["source_sha256"] = source_digest(folder)
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
    if "--previous" in sys.argv:
        check_versions(entries, sys.argv[sys.argv.index("--previous") + 1])
    index = {"api": API, "serial": serial, "extensions": entries}
    (out / "index.json").write_text(json.dumps(index, indent=2, ensure_ascii=False) + "\n")
    shown = out.relative_to(ROOT) if out.is_relative_to(ROOT) else out
    print(f"{shown}/index.json: {len(entries)} extensions")


if __name__ == "__main__":
    if "--check-contract" in sys.argv:
        check_contract(sys.argv[sys.argv.index("--check-contract") + 1])
    else:
        main()
