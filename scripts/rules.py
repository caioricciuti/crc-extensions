"""What crc accepts in an extension's manifest and module, for the scripts
that build and scaffold extensions. The same rules as crc's
src/ext/manifest.rs; scripts/contract/manifests.json, crc's own cases,
holds this file and crc to the same verdicts.
"""

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
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
