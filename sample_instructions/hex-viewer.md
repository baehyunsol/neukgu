Create a hex viewer program. It reads a file, and dumps the content of the file as a hex format, to stdout.

The dump format looks like this.

```
0000 | 89 50 4e 47 0d 0a 1a 0a   00 00 00 0d 49 48 44 52 | xPNG.... 000xIHDR
0010 | 00 00 00 80 00 00 00 44   08 02 00 00 00 c6 25 aa | 000.000D ..000.%.
0020 | 3e 00 00 00 c2 49 44 41   54 78 5e ed d4 81       | >000.IDA Tx^...
```

1. Each line shows 16 bytes. The above example file is 46 bytes long, so the last line has only 14 bytes.
  - A line consists of 3 parts: byte offset, hex dump, and ascii encode.
  - Byte offset is offset of the first byte of the line. It's represented in hexadecimal.
  - The second part is hex dump of 16 bytes. Each byte is 2 characters in hex. 8 bytes comes, 3 whitespaces, then the next 8 byte comes.
  - The last part is ascii dump of the 16 bytes. If the byte is 0, the corresponding ascii dump is 0. If the byte is in ascii range (32..=126), it dumps the ascii character. Otherwise, the ascii dump is '.'.
    - You might think "0 and . are valid ascii characters, wouldn't that be confusing?". You're correct. So I want you to implement `--color` flag.
    - If `--color` flag is set, the stdout dump is colored with ansi-term coloring. In this mode, the hex dump (2nd part) and the ascii dump (3rd part) are colored. The color scheme is
      - match byte
        - 0 => gray
        - 1..=31 => green
        - 32..=126 => sky blue
        - 127..=255 => red
    - `--color` is on by default
2. I want you to implement `--start` and `--end` cli args. These are the byte offsets.
  - For example, if `--end=2048` are given, it dumps the first 2048 bytes. If `--start=1024` is given, it skips the first 1024 bytes.
3. You don't have to care about paging. I'll use `less` to page the result.
