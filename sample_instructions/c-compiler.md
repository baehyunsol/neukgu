I want you to create a C-compiler from scratch, in Rust. You can only use Rust std libraries, don't use any external libraries.

It works in 5 stages: C -> AST -> IR -> ASM -> ELF.
The compiler should include assembler and linker, so that it can generate ELF files, and everything has to be in pure, no-external-lib Rust.
The compiler will only accept valid C code. It never has to care about syntax errors and type errors.

Currently, it's suffice to support linux-x64-gnu. But later I want to add different OS/ISA, so you have to keep that in mind.
