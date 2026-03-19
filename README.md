# fbsviewer-lib

Source code for the FlatBuffers and Protocol Buffers binary visualizer. Interactive tool
for understanding binary encoding, available as a native desktop app and a web app (WASM).

[![Live Demo](https://img.shields.io/badge/Live_Demo-fbsviewer.shuozeli.com-blue?style=for-the-badge)](https://fbsviewer.shuozeli.com/)
[![Deploy Repo](https://img.shields.io/badge/Deploy_Repo-Shuozeli%2Ffbsviewer-green?style=flat-square&logo=github)](https://github.com/Shuozeli/fbsviewer)

> **Try the live demo now: [fbsviewer.shuozeli.com](https://fbsviewer.shuozeli.com/)**
>
> No install required -- runs entirely in the browser via WebAssembly.

![FlatBuffers Binary Visualizer showing the Monster schema with color-coded hex view and structure tree](docs/blogposts/visualizer-monster-hex-view.png)

## Features

- Paste a `.fbs` schema and JSON or hex data (FlatBuffers), or a `.proto` schema and hex binary (Protocol Buffers)
- Pure Rust JSON-to-FlatBuffers encoder (works on native and WASM)
- Hex view with per-byte coloring by region type (vtable, table, scalar, string, vector, etc.)
- Structure tree with collapsible hierarchy matching the binary layout
- Bidirectional hover highlighting with click-to-lock
- Data format dropdown: switch between JSON and Hex input with auto-conversion
- 12 built-in template examples (Monster, Simple Scalars, Nested Structs, String Fields, Nested Tables, Union, Vector of Tables/Strings/Structs, All Scalar Types, Default Values, File Identifier)
- Random schema and data generation for quick exploration
- Shareable permalinks (state encoded in URL, auto-synced)
- FBS and JSON syntax highlighting in editors
- Decoded JSON view for inspecting walker output
- File upload support (native: file dialogs, WASM: browser upload)
- Responsive layout (stacked on narrow screens, side-by-side on wide screens)
- Auto-detection of schema format (FlatBuffers vs Protobuf)

## Architecture

```
visualizer-core/   Portable library (no GUI deps)
  - Re-exports binary walker and region types from flatc-rs-annotator
  - JSON encoder: JSON -> FlatBuffers binary (pure Rust)
  - JSON decoder: annotated regions -> JSON
  - Protobuf walker bridge: protobuf-rs annotations -> shared region type
  - Hex parser: hex string -> bytes
  - Schema loader: JSON string -> Schema

visualizer/        egui GUI app (native + WASM)
  - Desktop app via eframe
  - Web app via trunk + wasm-bindgen
  - MVU architecture (state.rs / view.rs / app.rs)
  - Permalink sharing, syntax highlighting, random generation

visualizer-cli/    CLI binary inspection tool (flatbuf-viz)
  - Supports .fbs, .json, and .proto schema files
  - Filters: --byte-range, --field, --region-type
  - Output formats: table, json, compact
```

## Build

```bash
# Native desktop app
cargo run -p flatbuf-visualizer

# Run tests
cargo test --workspace

# WASM web app
cd visualizer && trunk build --release --public-url ./
```

## Dependencies

Depends on [flatbuffers-rs](https://github.com/Shuozeli/flatbuffers-rs) for schema
compilation (`flatc-rs-compiler`), schema types (`flatc-rs-schema`), binary annotation
(`flatc-rs-annotator`), and random generation (`flatc-rs-fbs-gen`, `flatc-rs-data-gen`).

Depends on [protobuf-rs](https://github.com/Shuozeli/protobuf-rs) for Protocol Buffers
support (`protoc-rs-annotator`, `protoc-rs-schema`, `protoc-rs-analyzer`).

## Deploy

Pre-built WASM artifacts are published to [Shuozeli/fbsviewer](https://github.com/Shuozeli/fbsviewer)
and auto-deployed to [fbsviewer.shuozeli.com](https://fbsviewer.shuozeli.com/).

## License

MIT
