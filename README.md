# crc extensions

Official extensions for [crc](https://github.com/caioricciuti/crc), the
macOS text editor.

**Nothing here yet.** crc does not load extensions today. How it will is in
the [extensions design](https://crceditor.com/extensions/): WebAssembly
modules with a manifest that declares what each one may read, write and
reach, run by crc's own interpreter with a budget, off the main thread.

## What will live here

```
extensions/<name>/     one folder per extension: Rust source, manifest.json,
                       README.md shown before install
crc-extension/         the small crate that turns an extension into a plain
                       Rust function over &str
index.json             the registry crc reads, signed
```

## Rules for every extension in this repository

- Built from source in CI, never checked in as a binary.
- Signed in a protected release environment. crc verifies the signature
  with a key built into the app.
- Asks only for the capabilities it uses. No network, files or processes
  unless the design has a capability for it, and it is needed.
- No dependencies beyond the standard library and `crc-extension` without a
  written review, the same bar as crc itself.
