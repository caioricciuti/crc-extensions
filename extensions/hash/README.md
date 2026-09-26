# Hash

Digests of the selection, and a look inside a JWT. Works on the selection
only: select the text, run the command, and the selection becomes the
result. Undo brings the text back.

- **MD5**: the 32 hex digit MD5 digest.
- **SHA-1**: the 40 hex digit SHA-1 digest.
- **SHA-256**: the 64 hex digit SHA-256 digest.
- **SHA-512**: the 128 hex digit SHA-512 digest.
- **CRC32**: the 8 hex digit CRC-32, the same one zlib, PNG and `cksum -o 3`
  compute.
- **Decode JWT**: replaces a token with its header and payload as readable
  JSON, and says in the status line when it expires. The signature is not
  verified; there is no key here to verify it with. Read the claims, do not
  trust them.

Every digest is of the exact UTF-8 bytes of the selection, nothing trimmed:
a trailing newline or a leading space changes the result, the same way it
would for `shasum` on a file. The status line says how many bytes went in.

MD5 and SHA-1 are broken as security primitives and are here for checksums
and for talking to systems that still use them.

crc gives a command about two seconds. A digest of one megabyte takes half
a second; a selection of several megabytes runs out of time, and crc says
so and changes nothing.

Reads and replaces the text you give it. Nothing else: no files, no
network.
