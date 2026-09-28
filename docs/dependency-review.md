# Dependency review

Every extension here builds from the standard library and `crc-extension`,
except one. This file is the written reason the rules ask for, and the
record of what was checked.

## How dependencies are held

- Exact `=` pins, `default-features = false`, and `Cargo.lock` committed.
- CI and the release workflow run `scripts/vendor.sh` first: `cargo vendor
  --locked` fetches exactly what `Cargo.lock` names, checked against its
  checksums, and every later step builds with `CARGO_NET_OFFLINE=true`.
- `scripts/check-build-scripts.sh` reads `cargo metadata` (it runs no
  dependency code, and runs before any does) and fails on: a crate or
  version not in `ALLOWED_CRATES`, a source other than crates.io, a path
  package outside this workspace, a dependency build script or proc macro
  not on its allowlist, a build script or proc macro in the workspace's own
  packages (none are allowed), or a tree past `MAX_CRATES`. crc runs the
  same script with its own lists.
- A dependency is linked into one extension's module, never into
  `crc-extension`, so every other extension stays at zero.
- Whatever a dependency does, it runs inside crc's sandbox: no files, no
  network, no processes, only what the manifest's capabilities grant.

Adding or bumping one: check the crate on crates.io (publish date, at least
seven days old, maintainers), read its source and any `build.rs`, pin it
exactly with default features off, run `scripts/vendor.sh`, and record it
here in the same pull request.

## pulldown-cmark, for Markdown Preview (reviewed 2026-09-27)

Why: correct CommonMark with the GitHub tables, task lists, strikethrough
and footnotes is thousands of lines and a spec test suite. crc's own
renderer covers what READMEs need and no more. Getting nested lists,
reference links and emphasis rules right is exactly the work this crate has
done for years.

Used as `pulldown-cmark = { version = "=0.13.4", default-features = false,
features = ["html"] }`. That leaves out `getopts` and the command-line
binary, and `simd` stays off, so the crate builds under
`#![forbid(unsafe_code)]`.

| Crate | Version | Published | Owners | Build script | Proc macro | License |
|---|---|---|---|---|---|---|
| pulldown-cmark | 0.13.4 | 2026-05-20 | raphlinus, marcusklaas, Martin1887 | yes, see below | no | MIT |
| pulldown-cmark-escape | 0.11.0 | 2024-05-15 | Martin1887 | no | no | MIT |
| memchr | 2.8.3 | 2026-07-08 | BurntSushi | no (`build = false`) | no | Unlicense OR MIT |
| unicase | 2.9.0 | 2026-01-06 | seanmonstar | no (`build = false`) | no | MIT OR Apache-2.0 |
| bitflags | 2.13.2 | 2026-09-10 | KodrAus, rust-lang-owner | no (`build = false`) | no | MIT OR Apache-2.0 |

Five crates in all, none with dependencies of its own in this configuration
(memchr's `log` and bitflags' `serde`/`arbitrary`/`bytemuck` are optional
and off). bitflags 2.13.2 is also in crc's own reviewed tree.

- pulldown-cmark's `build.rs` (249 lines): `main` calls
  `generate_tests_from_spec`, which is an empty function unless the
  `gen-tests` feature is on. With it on, it reads the spec files in the
  crate and writes test sources to `OUT_DIR`. No network, no processes, no
  environment. It is the one entry in the allowlist.
- None of the five uses `std::process`, `std::net`, `std::fs`,
  `include_bytes!` or `env!` in library code. (pulldown-cmark's `main.rs`
  reads files, and is not built without `getopts`.)
- memchr has many `unsafe` blocks, for its vectorised search. On
  `wasm32-unknown-unknown` without `simd128` it takes its portable path.
- The vendored sources were compared file by file with the `.crate`
  archives read for this review: identical, apart from the `.gitignore`
  files `cargo vendor` leaves out.

Markdown Preview adds about 244 KB of WebAssembly with all of it linked.
