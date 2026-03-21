mod hex_parse;
pub mod json_decoder;
mod json_encoder;
mod proto_walker;
mod schema_loader;

// Re-export annotator types (previously local binary_walker + region modules)
pub use flatc_rs_annotator::{walk_binary, AnnotatedRegion, BinaryWalker, RegionType, WalkError};

pub use hex_parse::{parse_hex_bytes, HexParseError};
pub use json_decoder::annotations_to_json;
pub use json_encoder::{encode_json, JsonEncodeError};
pub use proto_walker::{walk_protobuf, ProtoWalkError};
pub use schema_loader::{load_schema_from_json, SchemaLoadError, SchemaLoadResult};

// Re-export ResolvedSchema (the new canonical schema type) so UI crates don't
// need a direct dependency on flatc-rs-schema.  The legacy `Schema` is kept for
// internal backward-compat (json_encoder, schema_loader deserialization).
pub use flatc_rs_schema::resolved::ResolvedSchema;

// Re-export protobuf schema types so UI crates don't need direct deps
pub use protoc_rs_schema::FileDescriptorSet as ProtoSchema;

/// Collect fully-qualified message names from a `FileDescriptorSet`.
///
/// Returns names like `".package.MessageName"` for each top-level message.
pub fn collect_proto_message_names(fds: &ProtoSchema) -> Vec<String> {
    let mut names = Vec::new();
    for file in &fds.file {
        let pkg = file.package.as_deref().unwrap_or("");
        for msg in &file.message_type {
            if let Some(ref name) = msg.name {
                if pkg.is_empty() {
                    names.push(format!(".{name}"));
                } else {
                    names.push(format!(".{pkg}.{name}"));
                }
            }
        }
    }
    names
}
