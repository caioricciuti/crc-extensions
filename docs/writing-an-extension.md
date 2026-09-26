# Writing an extension

A crc extension is a small Rust library compiled to WebAssembly. crc runs it
in a sandbox: it gets the text you give it, returns what should replace it,
and cannot touch your files, your network or anything else unless its
manifest asks and you agree. The design behind this is at
[crceditor.com/extensions/design](https://crceditor.com/extensions/design/).

## What you need

- Rust, through [rustup](https://rustup.rs).
- The WebAssembly target: `rustup target add wasm32-unknown-unknown`.
  Inside this repository `rust-toolchain.toml` does that for you.

## Start one

```sh
python3 scripts/new-extension.py
```

It asks for everything crc shows before install: the id (`author.name`),
the name, a one-line description, the author, an icon, and each command
with the title the palette shows. It writes `extensions/<name>/` with a
`Cargo.toml`, a `src/lib.rs` with a function per command and a test, the
`manifest.json` and a `README.md`, and prints the next steps. It builds and
passes CI as generated; then you fill it in. With flags it asks nothing:
`python3 scripts/new-extension.py --help`.

The files it writes:

**`Cargo.toml`**: the package name. The `.wasm` file is named after it, with
dashes turned into underscores.

**`src/lib.rs`**: your commands. Each is a function from `Input` to
`Output`:

```rust
use crc_extension::{Input, Output};

fn shout(input: Input) -> Output {
    Output::replace(input.text.to_uppercase())
}

crc_extension::commands! {
    "shout" => shout,
}
```

`Input` has the `text` (the selection, or the whole document when nothing
is selected), whether it was a `selection`, and the document's `language`.
An `Output` can `replace` the text, show a `message` in the status line, or
both. `crc_extension::log` writes to crc's extension log while you debug.

**`manifest.json`**: what crc shows before install and enforces after.

```json
{
  "id": "yourname.shout",
  "name": "Shout",
  "version": "0.1.0",
  "description": "Upper-case the selection.",
  "authors": ["Your Name"],
  "license": "MIT OR Apache-2.0",
  "api": 1,
  "entry": "shout.wasm",
  "capabilities": ["selection.read", "selection.replace"],
  "commands": [{ "id": "shout", "title": "Shout" }]
}
```

- `id` is `author.name`, lower case, and never changes.
- Each command's `id` must match a name in `commands!`; its `title` is what
  the palette shows.
- `icon` is a name from `scripts/icons.txt` (`--list-icons` prints them),
  one of the icons crc already draws. Extensions do not ship images: crc
  never decodes a file from an extension, and every icon is tinted to match
  the theme.
- `homepage` is optional, an `https://` address shown on the details.
- `capabilities` lists what the extension may do. Ask for as little as you
  can. The ones that exist today:

| Capability | Lets the extension |
|---|---|
| `selection.read` | read the selection |
| `selection.replace` | replace the selection |
| `document.read` | read the whole document |
| `document.edit` | replace the whole document |

More (the project index, diagnostics, a status bar item) come as crc
implements them. There is no network, file or process access, on purpose.

Also write a `README.md`: crc shows it before anyone installs.

## Build and check

```sh
cargo test
cargo build --release --target wasm32-unknown-unknown
python3 scripts/build-registry.py
```

The last step checks your manifest against the module the same way CI does:
every command exported, nothing imported from outside crc, known
capabilities only.

## Try it in crc

In crc, **crc > Extensions**, then **Install from Folder…**, and pick a
folder holding your `manifest.json`, `README.md` and the `.wasm` from
`target/wasm32-unknown-unknown/release/`. A folder install is unsigned, so
crc says so before it installs. Your commands appear in the palette.

## Publish it

- **In this repository**: open a pull request adding your folder. CI builds
  it from source; once merged it ships in the next signed registry release,
  and everyone can install it from crc's Extensions page.
- **On your own**: publish the folder anywhere. People install it from a
  folder, unsigned, after crc shows them what it asks for.

Rules for this repository: built from source (no binaries in the pull
request), no dependencies beyond the standard library and `crc-extension`
without a written reason, and only the capabilities the extension uses.
