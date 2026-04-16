I want you to create a CLI that prettifies a structured input.
It takes an input from stdin, prettifies the input, and dumps it to stdout.

The input is structured:

1. Every curly braces, square brackets and parenthesis are opened and closed correctly.
2. Inside curly braces, square brackets and parenthesis, there are elements. Elements are separated by commas. There can be a trailing comma.
3. Curly braces, square brackets and parenthesis can be nested.
4. There can be 3 kinds of comments: python-style line comment (#), c-style line comment (//) and c-style block comment (/* */). The prettified result must preserve the comments. The contents of the comments are kept, and the beginning of comments are indented properly.
5. There are string literals inside quotations. You have to handle them properly. For example, if there's a square bracket inside a string literal, you should not indent that.
6. Some lists will be very long. For example, let's say there's a list with 2000 integers. If you put them in a single line, that'd be really difficult to read. If you put an integer per line, that's also difficult to read. The prettifier has to find a balance.

So, it can prettify json files, debug dump of rust (`format("{p:?}")`) and many other files.
