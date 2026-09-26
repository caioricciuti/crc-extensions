# Tables

Formats Markdown tables, and turns CSV or TSV into Markdown tables, box
tables, aligned columns or JSON. Works on the selection, or on the whole
document when nothing is selected.

**Format Markdown Table** aligns every pipe table in the text and keeps
the alignment marks of the separator row. Everything around the tables
stays as it is.

```
name|qty|  note                | name | qty | note |
:--|--:|:-:            ->      | :--- | --: | :--: |
Pear | 10 | ripe               | Pear |  10 | ripe |
|Kiwi|3|                       | Kiwi |   3 |      |
```

**CSV to Markdown Table** reads the first row as the header. Columns
holding only numbers are right-aligned.

```
name,qty              | name | qty |
Pear,10       ->      | ---- | --: |
Kiwi,3                | Pear |  10 |
                      | Kiwi |   3 |
```

**Markdown Table to CSV** is the way back: one comma-separated line per
row, quoted where a field holds a comma, a quote or a line break.

**CSV to Box Table** draws the table with box characters, the header set
apart, ready for a README, a comment or a chat message.

```
Name,Qty              ┌──────┬─────┐
Pear,10       ->      │ Name │ Qty │
Kiwi,3                ├──────┼─────┤
                      │ Pear │  10 │
                      │ Kiwi │   3 │
                      └──────┴─────┘
```

**Align CSV Columns** pads the fields so the delimiters line up. The
result is still CSV with the same delimiter; quoted fields keep their
quotes.

```
name,qty,note                 name  , qty, note
Pear,10,"a, b"       ->       Pear  ,  10, "a, b"
Kiwi,3,x                      Kiwi  ,   3, x
```

**CSV to JSON** gives an array of objects keyed by the header row.
Numbers and `true`/`false` are typed, empty cells become `null`.

```
id,name              [
1,Pear       ->        { "id": 1, "name": "Pear" }
                     ]
```

The delimiter is guessed from the first line: a tab if there is one, a
semicolon if there are more semicolons than commas, else a comma. Quoted
fields, doubled quotes and line breaks inside quotes follow RFC 4180.
Columns are measured for a monospace font: East Asian wide characters and
emoji count double, combining marks count nothing. The ranges are
approximate, so an unusual script may be a column off.

Reads and replaces the text you give it. Nothing else: no files, no
network.
