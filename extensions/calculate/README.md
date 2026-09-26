# Calculate

Arithmetic and number tools for the selection, or the whole document when
nothing is selected. Nothing asks for input: each command reads the text
and decides from it.

## Evaluate

One line: the expression becomes its result. Write `= something` after it
and the answer goes there instead, so a line can be recomputed as often as
you like.

```
12 * (3 + 4)            ->  84
2 ^ 10 = 999            ->  2 ^ 10 = 1024
```

Several lines: every line that is an expression becomes `expression =
result`; prose, code and bare numbers stay as they are.

```
rent 1200                       rent 1200
100 * 12                  ->    100 * 12 = 1200
sqrt(2) = 1.4                   sqrt(2) = 1.41421356237
```

| Piece | Written as |
|---|---|
| Operators | `+ - * / %`, `^` or `**` for powers, parentheses |
| Numbers | `42`, `3.14`, `1e6`, `1_000_000`, `1,000` (groups of three), `0xff`, `0b101`, `0o17` |
| Percent | `20% * 50` is 10, `200 * 15%` is 30. `7 % 4` is the remainder |
| Functions | `sqrt abs floor ceil round min max sin cos tan asin acos atan log ln log2 exp pow` |
| Constants | `pi`, `e` |

Integers stay exact (`2 ** 62` is `4611686018427387904`); everything else
is shown with up to twelve significant digits. Division by zero and
results that are not a number are reported in the status line, and the
text is left alone.

## Sum Numbers and Number Statistics

Both leave the text as it is and answer in the status line. Every number
in the text counts: a column in a table, figures in a sentence, literals
in code. `1,200.50` is one number; `abc123` and `v2` are names, not
numbers; `-3` is negative; `10px` is 10.

```
sum 2,000 of 2 numbers
5 numbers: sum 70, min 1, max 40, mean 14, median 10
```

## To Hexadecimal, To Binary, To Decimal

Rewrites every whole number in the text.

```
255 and -16 and 0b11    ->  To Hexadecimal  ->  0xff and -0x10 and 0x3
5 0x0a                  ->  To Binary       ->  0b101 0b1010
0xff 0b101 0o17 1f      ->  To Decimal      ->  255 5 15 31
```

To Decimal also reads a bare word as hexadecimal when it can be nothing
else: only hex digits, with at least one letter and one digit (`1f`,
`c0ffee1`). `ff`, `add` and `deadbeef` are left alone, since they are also
words.

## Unix Timestamp to Date, Date to Unix Timestamp

```
1727361600             ->  2024-09-26T14:40:00Z
1700000000123          ->  2023-11-14T22:13:20.123Z
2024-09-26T16:40+02:00 ->  1727361600
2024-09-26             ->  1727308800
```

A run of 9 or 10 digits is seconds, 12 or 13 is milliseconds, 15 or 16
is microseconds. Dates are ISO 8601: `YYYY-MM-DD`, optionally followed by
a time with `T` or a space, seconds, a fraction, and `Z` or an offset. A
time without an offset is UTC. Everything is in UTC: the sandbox has no
clock and no time zones.

Reads and replaces the text you give it. Nothing else: no files, no
network.
