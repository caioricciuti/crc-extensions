# crc extensions

Official extensions for [crc](https://github.com/caioricciuti/crc), the
macOS text editor.

How crc runs extensions is in the
[extensions design](https://crceditor.com/extensions/): WebAssembly modules
with a manifest that declares what each one may read, write and reach, run
by crc's own interpreter with a budget, off the main thread. crc's side is
shipping; the official registry is published from this repository's
releases.

## What is here

```
crc-extension/          the crate: an extension is a plain Rust function
                        from Input to Output
extensions/sort-lines/  Sort Lines: sort, sort descending, remove duplicates
extensions/change-case/ Change Case: upper, lower, title, snake_case,
                        kebab-case, camelCase, PascalCase, CONSTANT_CASE
extensions/encode/      Encode and Decode: Base64, URL and HTML, both ways
scripts/                new-extension.py starts a new one; build-registry.py
                        writes and checks the registry; icons.txt lists the
                        icons a manifest may name
docs/                   writing-an-extension.md, how to make your own
```

A tag `registry-<date>` builds everything from source, signs the registry
(`index.json`) and opens a draft release. crc reads the latest published
release and checks the signature against a key built into the app.

To write one: `python3 scripts/new-extension.py`, then read
[docs/writing-an-extension.md](docs/writing-an-extension.md).

## Rules for every extension in this repository

- Built from source in CI, never checked in as a binary.
- Signed in a protected release environment. crc verifies the signature
  with a key built into the app.
- Asks only for the capabilities it uses. No network, files or processes
  unless the design has a capability for it, and it is needed.
- No dependencies beyond the standard library and `crc-extension` without a
  written review, the same bar as crc itself.

## Licence

Everything here is dual-licensed under [MIT](LICENSE-MIT) or
[Apache 2.0](LICENSE-APACHE), at your option. That includes the
`crc-extension` crate, so an extension built on it can use any licence its
author likes, open or not. crc itself is GPL-3.0; extensions talk to it
through a defined interface and are not part of it.

Unless you say otherwise, a contribution you submit is dual-licensed the
same way, without additional terms.
