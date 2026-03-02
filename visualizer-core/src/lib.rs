mod binary_walker;
mod hex_parse;
pub mod json_decoder;
mod json_encoder;
mod region;
mod schema_loader;

// Re-export public API
pub use binary_walker::{is_scalar, scalar_byte_size, BinaryWalker};
pub use hex_parse::{parse_hex_bytes, HexParseError};
pub use json_decoder::annotations_to_json;
pub use json_encoder::{encode_json, JsonEncodeError};
pub use region::{AnnotatedRegion, RegionType, WalkError};
pub use schema_loader::{load_schema_from_json, SchemaLoadError, SchemaLoadResult};

// Re-export Schema so UI crates don't need a direct dependency on flatc-rs-schema
pub use flatc_rs_schema::Schema;

/// Walk a FlatBuffers binary using a Schema and root type name.
///
/// This is a convenience wrapper around [`BinaryWalker`].
pub fn walk_binary(
    binary: &[u8],
    schema: &Schema,
    root_type_name: &str,
) -> Result<Vec<AnnotatedRegion>, WalkError> {
    let walker = BinaryWalker::new(binary, schema);
    walker.walk(root_type_name)
}
