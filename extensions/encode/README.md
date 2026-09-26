# Encode and Decode

Encodes or decodes the selection, or the whole document when nothing is
selected.

- **Base64 Encode** and **Base64 Decode**: the standard alphabet with
  padding. Decoding ignores line breaks, accepts missing padding and the
  URL alphabet (`-` and `_`).
- **URL Encode** and **URL Decode**: percent-encoding for a URL component;
  everything but letters, digits and `-_.~` is escaped. Decoding also turns
  `+` into a space, as forms send it.
- **HTML Escape** and **HTML Unescape**: `& < > " '`, the common named
  entities and every numeric one.

Text that does not decode is left as it is, with a message saying why,
and binary results are refused rather than pasted in as garbage.

Reads and replaces the text you give it. Nothing else: no files, no
network.
