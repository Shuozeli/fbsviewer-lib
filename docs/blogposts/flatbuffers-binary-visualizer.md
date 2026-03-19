# FlatBuffers Binary Visualizer -- see what every byte means

FlatBuffers binary encoding is opaque. There's no way to look at a buffer and tell which bytes are vtable entries, which are scalar fields, and which are alignment padding without manually walking the format spec.

This tool takes a `.fbs` schema and a FlatBuffer (JSON or raw hex), and shows you the role of every byte: **[fbsviewer.shuozeli.com](https://fbsviewer.shuozeli.com/)**

## What It Does

Paste a `.fbs` schema and some JSON data. The tool compiles the schema, encodes the JSON into a FlatBuffer, and shows you exactly what every byte means.

The hex view color-codes each byte by its role: vtable entries in blue, scalar fields in green, strings in yellow, vectors in purple, structs in orange, padding in gray. Hover over a byte and the structure tree highlights what field it belongs to. Click a field in the tree and the corresponding bytes light up.

It comes with 12 built-in examples covering common patterns: the classic Monster schema, nested tables, unions, vectors of structs, file identifiers. You can also paste your own schema and data to inspect whatever you're working on.

![FlatBuffers Binary Visualizer showing the Monster schema with color-coded hex view and structure tree](visualizer-monster-hex-view.png)

## Why This Exists

FlatBuffers has a lot of virtues -- zero-copy access, cross-platform, compact encoding. But the binary format is genuinely hard to reason about by hand.

Unlike Protocol Buffers (which are length-prefixed and relatively straightforward to decode), FlatBuffers uses a system of vtables and signed/unsigned offsets that make the byte layout non-obvious. Tables don't store their fields sequentially -- they store a vtable pointer, then data at offsets determined by the vtable. Structs are inline and aligned. Strings have a length prefix and null terminator. Vectors have a count prefix, and whether the elements are inline or offset-based depends on the element type.

When something goes wrong -- a field reads as zero when it shouldn't, or a string comes back garbled -- the only way to diagnose it is to understand the binary layout. This tool makes that layout visible.

## How It Works Under the Hood

The tool has three layers, each a separate Rust crate:

**Schema compilation.** The `.fbs` schema is parsed using a [tree-sitter](https://tree-sitter.github.io/) grammar, then run through semantic analysis (type resolution, enum value assignment, struct layout computation). This is the same compiler pipeline I use elsewhere, not a special-purpose mini-parser. One honest caveat: tree-sitter's parser is written in C, so this isn't "pure Rust" end-to-end. The tree-sitter C code is compiled to WASM alongside the Rust code, with a small libc shim to bridge the gap. It works, but it adds complexity to the WASM build.

**JSON-to-binary encoding.** A Rust encoder walks the JSON input and the compiled schema simultaneously, writing FlatBuffer-compatible binary. This handles all the format details: vtable deduplication, alignment padding, offset computation, union discriminants, nested table serialization. It also generates a parallel set of byte-range annotations -- "bytes 16-23 are the `hp` field of the `Monster` table" -- which is what the UI uses for highlighting.

**Binary walking.** Given a schema and a binary blob, the walker reads the raw bytes and reconstructs the structure: where each vtable lives, which fields are present, what the offset chains look like. This works on any valid FlatBuffer, not just ones the tool encoded. So you can feed it a binary that was produced by `flatc` or any other implementation and inspect it.

The web UI is built with [egui](https://github.com/emilk/egui), an immediate-mode Rust GUI framework that compiles to both native desktop and WASM.

## Built with Claude Code

Most of the code in this project was written with [Claude Code](https://claude.ai/claude-code). I directed the architecture, but the bulk of the Rust -- the encoder, the binary walker, the egui UI, the tests -- was AI-generated. I have not reviewed every single line.

This means bugs are likely. AI-generated code handles the common cases well but misses edge cases. The `force_align` struct alignment was a recent example: vectors of structs with `force_align: 16` were being padded incorrectly because the encoder aligned the wrong offset relative to the length prefix. It compiled, it passed the tests that existed at the time, and it produced wrong binary output for a specific class of schemas.

## Status & Limitations

What I have tested:

- **148 tests** covering the encoder, binary walker, CLI, and UI state machine
- **Roundtrip encoding/decoding** for all 12 built-in templates (scalars, strings, nested tables, unions, vectors of tables/structs/strings, enums, file identifiers, default values, `force_align` structs)
- **Byte-level verification** against binaries produced by the official C++ `flatc` compiler
- **Full byte coverage checks** confirming every byte in the output is accounted for (no unexplained gaps)
- **Random schema and data generation** that produces arbitrary `.fbs` schemas and matching JSON data for fuzz-style testing of the encoder and walker
- **Error cases**: truncated binaries, missing root types, type mismatches, malformed JSON

What might break: deeply nested unions-inside-vectors-inside-unions, schemas with hundreds of fields, large binary files, or every combination of field types. FlatBuffers has a large surface area.

Current constraints:

- **The WASM binary is 3.8 MB.** That's the cost of shipping a full schema compiler + encoder + binary walker in the browser. Not yet optimized for size.
- **No FlexBuffers support.** Only classic FlatBuffers.
- **No binary-to-JSON without a schema.** FlatBuffers are not self-describing. You need the `.fbs` schema to interpret the bytes.

**Update (since initial publication):** The tool now also supports Protocol Buffers binary visualization. Paste a `.proto` schema, provide hex-encoded protobuf binary data, and the tool will annotate tags, varints, length-delimited fields, and fixed-width fields. Schema format is auto-detected.

## Try It

**Live demo**: [fbsviewer.shuozeli.com](https://fbsviewer.shuozeli.com/)

Pick a template from the dropdown to see the encoding of common patterns, or paste your own schema and data.

**Source**: [github.com/Shuozeli/fbsviewer-lib](https://github.com/Shuozeli/fbsviewer-lib)

If the tool gives you wrong output for your schema, file a bug using the [encoding/decoding issue template](https://github.com/Shuozeli/fbsviewer-lib/issues/new/choose) -- it asks for your `.fbs` and JSON so I can reproduce the problem.
