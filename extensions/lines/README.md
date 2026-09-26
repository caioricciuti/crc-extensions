# Lines

Align, join, number, wrap, sort naturally and tidy lines, and count words.
Works on the selection, or on the whole document when nothing is selected.
Every command keeps each line's own ending, so `\r\n` files stay `\r\n`.

- **Align by =**: lines up `=` across consecutive lines with the same
  indentation. Compound tokens (`==`, `+=`, `=>`, `:=` and the like) count
  as one.

      let a = 1;            let a     = 1;
      let total += 2;   ->  let total += 2;

- **Align by :**: lines up the values after `:`, one space after the
  colon, for YAML, JSON, CSS and the like. `::` is left alone.

      name: crc             name:    crc
      version:   0.1    ->  version: 0.1

- **Join Lines**: each paragraph becomes one line, its lines trimmed and
  joined with a space. Blank lines keep paragraphs apart.
- **Number Lines**: right-aligned numbers, then a space, then the line.
- **Reverse Lines**: last line first.
- **Sort Lines Naturally**: without regard to case, and with numbers
  compared by value, so `file2` comes before `file10`.
- **Sort Lines by Length**: shortest first; equal lengths keep their order.
- **Trim Trailing Whitespace**: spaces and tabs at the end of every line.
- **Squeeze Blank Lines**: a run of blank lines becomes one.
- **Remove Blank Lines**: all of them.
- **Hard Wrap at 80**: re-flows each paragraph to 80 columns. A paragraph
  keeps its indentation and comment marker (`//`, `#`, `--`, `>` and so
  on) on every line, and a list item gets a hanging indent. In code, only
  comments are wrapped; in prose, everything except headings, tables,
  fenced and indented code.

      // alpha beta ... omicron pi       // alpha beta ... nu xi
      // rho                         ->  // omicron pi rho

- **Unwrap Paragraphs**: the reverse, each paragraph on one line.
- **Word Count**: words, characters, lines and reading time, in the status
  line. Changes nothing.

Reads and replaces the text you give it. Nothing else: no files, no
network.
