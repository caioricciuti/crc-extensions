# crc extensions

Official extensions for [crc](https://github.com/caioricciuti/crc), the
macOS text editor.

How crc runs extensions is in the
[extensions design](https://crceditor.com/extensions/design/): WebAssembly modules
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
extensions/json-tools/  JSON Tools: format, minify, sort keys, JSON to and
                        from YAML, and JSON to TypeScript, Rust, Go or
                        Python types for the file you are in
extensions/tables/      Tables: format Markdown tables; CSV or TSV to
                        Markdown, box-drawn table, aligned columns or JSON
extensions/lines/       Lines: align by = or :, join, number, reverse,
                        natural sort, sort by length, trim, squeeze, hard
                        wrap at 80, unwrap, word count
extensions/calculate/   Calculate: evaluate arithmetic in place, sum and
                        statistics, hex, binary and decimal, Unix
                        timestamps to dates and back
extensions/hash/        Hash: MD5, SHA-1, SHA-256, SHA-512, CRC32, and
                        decode a JWT
extensions/banner/      Banner: block-letter banners, comment boxes and
                        dividers in the file's comment syntax
extensions/markdown-preview/
                        Markdown Preview: the document rendered in a pane
                        beside the editor, as you type (Cmd-E)
scripts/                new-extension.py starts a new one; build-registry.py
                        writes and checks the registry; icons.txt lists the
                        icons a manifest may name; vendor.sh and
                        check-build-scripts.sh hold dependencies to review
docs/                   writing-an-extension.md, how to make your own;
                        dependency-review.md, the one crate used and why
```

A tag `registry-<date>` builds everything from source, signs the registry
(`index.json`) and opens a draft release. crc reads the latest published
release and checks the signature against a key built into the app. The
index carries a `serial` (the commit time it was built from), and crc
refuses one older than the newest it has seen, so an old signed index
cannot be served again as the latest.

To write one: `python3 scripts/new-extension.py`, then read
[docs/writing-an-extension.md](docs/writing-an-extension.md).

## Rules for every extension in this repository

- Built from source in CI, never checked in as a binary.
- Signed in a protected release environment. crc verifies the signature
  with a key built into the app.
- Asks only for the capabilities it uses. No network, files or processes
  unless the design has a capability for it, and it is needed.
- No dependencies beyond the standard library and `crc-extension` without a
  written review, the same bar as crc itself. The one there is (Markdown
  Preview's pulldown-cmark) is pinned, vendored in CI, and built offline.

## Licence

Everything here is dual-licensed under [MIT](LICENSE-MIT) or
[Apache 2.0](LICENSE-APACHE), at your option. That includes the
`crc-extension` crate, so an extension built on it can use any licence its
author likes, open or not. crc itself is GPL-3.0; extensions talk to it
through a defined interface and are not part of it.

Unless you say otherwise, a contribution you submit is dual-licensed the
same way, without additional terms.
