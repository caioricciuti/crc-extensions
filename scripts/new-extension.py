#!/usr/bin/env python3
"""Starts a new extension: a folder under extensions/ that builds, passes CI
and installs in crc as it is, for you to fill in.

    python3 scripts/new-extension.py                 asks for everything
    python3 scripts/new-extension.py --id you.shout --name Shout \\
        --description "Upper-case the text." --author "Your Name" \\
        --icon text-size --command "shout:Shout"
    python3 scripts/new-extension.py --list-icons

Every field crc shows before install is asked for here: the name, a one-line
description, who wrote it, the licence, an icon from crc's set, and the
commands with their palette titles. Standard library only.
"""

import argparse
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ID = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*(\.[a-z0-9]+(-[a-z0-9]+)*)+$")
COMMAND = re.compile(r"^[a-z][a-z0-9_]*$")
ICONS = [
    line.strip()
    for line in (ROOT / "scripts" / "icons.txt").read_text().splitlines()
    if line.strip() and not line.startswith("#")
]


def ask(prompt, default=None, check=None, why=""):
    """Asks until the answer passes `check`."""
    while True:
        shown = f" [{default}]" if default else ""
        try:
            answer = input(f"{prompt}{shown}: ").strip() or (default or "")
        except EOFError:
            sys.exit("\nstopped")
        if answer and (check is None or check(answer)):
            return answer
        print(f"  {why or 'required'}")


def main():
    parser = argparse.ArgumentParser(description="Start a new crc extension.")
    parser.add_argument("--id", help="author.name, lower case, never changes")
    parser.add_argument("--name", help="what people see, e.g. Change Case")
    parser.add_argument("--description", help="one line")
    parser.add_argument("--author")
    parser.add_argument("--icon", help="one of --list-icons")
    parser.add_argument("--license", default="MIT OR Apache-2.0")
    parser.add_argument(
        "--command",
        action="append",
        help="id:Title, repeatable, e.g. shout:Shout",
    )
    parser.add_argument(
        "--selection-only",
        action="store_true",
        help="only the selection, never the whole document",
    )
    parser.add_argument("--list-icons", action="store_true")
    args = parser.parse_args()

    if args.list_icons:
        print("\n".join(ICONS))
        return

    interactive = args.id is None
    ext_id = args.id or ask(
        "Id (author.name)", check=ID.match, why="like yourname.shout: lower case, dots between parts"
    )
    if not ID.match(ext_id):
        sys.exit(f"--id {ext_id!r} is not like author.name")
    slug = ext_id.split(".", 1)[1].replace(".", "-")
    folder = ROOT / "extensions" / slug
    if folder.exists():
        sys.exit(f"{folder.relative_to(ROOT)} exists already")
    default_name = " ".join(w.capitalize() for w in slug.split("-"))
    name = args.name or (ask("Name", default_name) if interactive else default_name)
    # With flags, a missing field is an error rather than a prompt, so a
    # script calling this never hangs waiting for an answer.
    def need(value, flag, prompt, why=""):
        if value:
            return value
        if not interactive:
            sys.exit(f"{flag} is needed when --id is given")
        return ask(prompt, why=why)

    description = need(args.description, "--description", "One-line description", "crc shows it in the list")
    author = need(args.author, "--author", "Author")
    icon = args.icon or (
        ask(
            "Icon (--list-icons shows them)",
            "extensions",
            check=lambda i: i in ICONS,
            why="not one of crc's icons; run with --list-icons",
        )
        if interactive
        else "extensions"
    )
    if icon not in ICONS:
        sys.exit(f"--icon {icon!r} is not one of crc's icons; see --list-icons")

    commands = []
    for spec in args.command or []:
        cid, _, title = spec.partition(":")
        if not COMMAND.match(cid) or not title.strip():
            sys.exit(f"--command {spec!r} is not id:Title with an id like shout_loudly")
        commands.append((cid, title.strip()))
    if not commands and interactive:
        print("Commands: an id (lower case and underscores) and the title the palette shows.")
        while True:
            try:
                cid = input("Command id" + (" (empty when done)" if commands else "") + ": ").strip()
            except EOFError:
                sys.exit("\nstopped")
            if not cid and commands:
                break
            if not COMMAND.match(cid):
                print("  lower case, digits and underscores, like shout_loudly")
                continue
            default_title = " ".join(w.capitalize() for w in cid.split("_"))
            commands.append((cid, ask("  Palette title", default_title)))
    if not commands:
        commands = [("run", name)]

    selection_only = args.selection_only
    if interactive and not selection_only:
        whole = ask("Work on the whole document when nothing is selected? (y/n)", "y", check=lambda a: a in "yn")
        selection_only = whole == "n"
    capabilities = ["selection.read", "selection.replace"]
    if not selection_only:
        capabilities += ["document.read", "document.edit"]

    crate = slug
    entry = crate.replace("-", "_") + ".wasm"
    folder.mkdir(parents=True)
    (folder / "src").mkdir()

    (folder / "Cargo.toml").write_text(
        f"""[package]
name = "{crate}"
version = "0.1.0"
publish = false
edition.workspace = true
license.workspace = true
repository.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
crc-extension = {{ path = "../../crc-extension" }}
"""
    )

    functions = "\n\n".join(
        f"""/// {title}.
pub fn {cid}(input: Input) -> Output {{
    // Replace this with what the command does to the text.
    Output::replace(input.text)
}}"""
        for cid, title in commands
    )
    table = "\n".join(f'    "{cid}" => {cid},' for cid, _ in commands)
    first = commands[0][0]
    (folder / "src" / "lib.rs").write_text(
        f"""//! {name}: {description}

use crc_extension::{{Input, Output}};

{functions}

crc_extension::commands! {{
{table}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn {first}_answers() {{
        let out = {first}(Input {{
            text: "hello\\n".into(),
            ..Input::default()
        }});
        assert_eq!(out.replace.as_deref(), Some("hello\\n"));
    }}
}}
"""
    )

    manifest = {
        "id": ext_id,
        "name": name,
        "version": "0.1.0",
        "description": description,
        "authors": [author],
        "license": args.license,
        "icon": icon,
        "api": 1,
        "entry": entry,
        "capabilities": capabilities,
        "commands": [{"id": cid, "title": title} for cid, title in commands],
    }
    (folder / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")

    scope = "the selection" if selection_only else "the selection, or on the whole document when nothing is selected"
    listed = "\n".join(f"- **{title}**: what it does." for _, title in commands)
    (folder / "README.md").write_text(
        f"""# {name}

{description}

Works on {scope}:

{listed}

Reads and replaces the text you give it. Nothing else: no files, no
network.
"""
    )

    rel = folder.relative_to(ROOT)
    print(
        f"""
Created {rel}/: Cargo.toml, src/lib.rs, manifest.json, README.md.

Next:
  1. Write the commands in {rel}/src/lib.rs, and say what they do in README.md.
  2. cargo test -p {crate}
  3. cargo build --release --target wasm32-unknown-unknown
  4. python3 scripts/build-registry.py   (checks it the way CI does)
  5. Try it: copy manifest.json, README.md and
     target/wasm32-unknown-unknown/release/{entry} into one folder, then in
     crc: Extensions > Extensions..., Install from Folder...
"""
    )


if __name__ == "__main__":
    main()
