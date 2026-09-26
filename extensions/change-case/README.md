# Change Case

Changes the case of the selection, or of the whole document when nothing
is selected.

For prose:

- **Upper Case**: EVERY LETTER UP, with the full Unicode rules (straße
  becomes STRASSE).
- **Lower Case**: every letter down.
- **Title Case**: The First Letter Of Each Word Up, the rest down.

For names, line by line, keeping each line's indentation, so a column of
names converts together:

- **snake_case**, **kebab-case**, **camelCase**, **PascalCase** and
  **CONSTANT_CASE**.

Names are split where people mean: at spaces, underscores and dashes, and
where the case changes, so `parseHTTPResponse` is parse, HTTP, Response.

Reads and replaces the text you give it. Nothing else: no files, no
network.
