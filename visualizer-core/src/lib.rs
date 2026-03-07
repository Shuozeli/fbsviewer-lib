mod hex_parse;
pub mod json_decoder;
mod json_encoder;
mod schema_loader;

// Re-export annotator types (previously local binary_walker + region modules)
pub use flatc_rs_annotator::{
    is_scalar, scalar_byte_size, walk_binary, AnnotatedRegion, BinaryWalker, RegionType, WalkError,
};

pub use hex_parse::{parse_hex_bytes, HexParseError};
pub use json_decoder::annotations_to_json;
pub use json_encoder::{encode_json, JsonEncodeError};
pub use schema_loader::{load_schema_from_json, SchemaLoadError, SchemaLoadResult};

// Re-export Schema so UI crates don't need a direct dependency on flatc-rs-schema
pub use flatc_rs_schema::Schema;
