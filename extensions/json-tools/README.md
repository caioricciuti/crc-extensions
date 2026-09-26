# JSON Tools

Format, minify and sort JSON, convert between JSON and YAML, and turn a
JSON sample into types for the file's language.

Works on the selection, or on the whole document when nothing is selected.
When the text is not valid JSON, nothing changes and the status line says
where it stopped: `line 4, column 1: unexpected '}'`.

## Commands

**Format JSON**: two-space indentation, one member per line, keys in their
original order. `{"a":[1,2]}` becomes

```json
{
  "a": [
    1,
    2
  ]
}
```

**Minify JSON**: the same text with every space and newline removed. The
status line says how many bytes it saved.

**Sort JSON Keys**: every object's keys in order, at every depth, then
formatted. `{"b":1,"a":{"d":1,"c":2}}` becomes `a` then `b`, with `c` before
`d` inside.

**JSON to YAML**: block style, two-space indentation. Strings are quoted
only when YAML would read them as something else (`"12"`, `"yes"`, `"a: b"`,
an empty string). Multi-line strings become `|` literal blocks.

```yaml
name: crc
tags:
  - a
  - b
items:
  - id: 1
    ok: true
```

**YAML to JSON**: one YAML document to formatted JSON. Reads what real
configuration files use: block and flow mappings and sequences, plain,
single and double quoted strings, `|` and `>` blocks with `-` and `+`
chomping, comments, `---`, `true`/`false`/`null`/`~`, integers (decimal,
`0x`, `0o`) and floats. Not read, on purpose: anchors (`&`), aliases (`*`),
tags (`!`), and more than one document in the text. Those stop with a
message naming the line.

**JSON to Types**: type declarations inferred from the JSON, in the language
of the file you are in. From

```json
{"id": 1, "tags": ["a"], "owner": {"name": "x", "age": null}}
```

a `.ts` file gets

```ts
export interface Owner {
  name: string;
  age: unknown | null;
}

export interface Root {
  id: number;
  tags: string[];
  owner: Owner;
}
```

A `.rs` file gets `pub struct`s with `Option<T>` for null or missing fields,
a `.go` file gets structs with `json:"..."` tags, a `.py` file gets
`@dataclass` classes with type hints. Anything else gets TypeScript. Object
shapes are merged across array items, so a field missing from some items is
optional. Nested types are named after their key (`user_address` becomes
`UserAddress`, `items` becomes `Item`). Where the sample has different
kinds of value in one place, the field is `unknown`, `any` or has a
`// mixed types` note.

Reads and replaces the text you give it. Nothing else: no files, no
network.
