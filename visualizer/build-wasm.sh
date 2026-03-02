#!/bin/bash
set -e

cd "$(dirname "$0")"

trunk build --release --public-url ./

# Patch: browsers cannot resolve bare "env" module specifier.
# The Rust wasm_libc_shim provides all C stdlib implementations at link time,
# so the "env" import can be replaced with an empty object.
sed -i 's/^import \* as import1 from "env"/const import1 = {};/' dist/flatbuf-visualizer.js

# Remove integrity hashes that break after patching
sed -i 's/ crossorigin="anonymous" integrity="[^"]*"//g' dist/index.html

echo "Build complete: dist/"
ls -lh dist/
