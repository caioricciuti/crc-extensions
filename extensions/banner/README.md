# Banner

Turns the selection into big block letters, a boxed comment or a divider,
written as a comment in the file's own syntax: `//` in Rust, JavaScript,
TypeScript, C, C++ and Go, `#` in Python, TOML, YAML and shell, `/* */` in
CSS, `<!-- -->` in HTML, and plain text everywhere else. The selection's
indentation is kept.

**ASCII Banner** draws the text in a five-row block font with `#`:

```rust
//  #### ####   ####
// #     #   # #
// #     ####  #
// #     #  #  #
//  #### #   #  ####
```

**Block Banner** draws the same letters with `█`:

```python
# █   █ ███ █
# █   █  █  █
# █████  █  █
# █   █  █
# █   █ ███ █
```

The font has letters, digits and `! ? . , : ; - _ + = / ( ) ' " # @ & * %`.
Anything else comes out as a solid block so you notice. Each line of the
selection becomes its own banner.

**Comment Box** puts the selection's lines in a box:

```rust
// +------------------------------+
// |  Public API                  |
// |  Everything below is stable  |
// +------------------------------+
```

**Comment Divider** centres each line in dashes reaching column 80:

```python
# ---------------------------------- Helpers -----------------------------------
```

Text that is already a comment is not commented twice: select a `// note`
and box it, and the box holds `note`.

Reads and replaces the text you give it. Nothing else: no files, no
network.
