use std::fs;
use std::path::{Path, PathBuf};

use flatbuf_visualizer_core::{collect_proto_message_names, ProtoSchema, ResolvedSchema};

/// What kind of schema was loaded.
pub enum LoadedSchema {
    FlatBuffers {
        schema: Box<ResolvedSchema>,
        root_type_name: Option<String>,
    },
    Protobuf {
        schema: ProtoSchema,
        /// All top-level message FQNs (e.g., ".pkg.Foo").
        message_names: Vec<String>,
    },
}

/// Load a schema from a `.fbs`, `.json`, or `.proto` file.
pub fn load_schema(schema_path: &Path, include_paths: &[PathBuf]) -> Result<LoadedSchema, String> {
    let ext = schema_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "fbs" => load_from_fbs(schema_path, include_paths),
        "json" => load_from_json(schema_path),
        "proto" => load_from_proto(schema_path),
        other => Err(format!(
            "unsupported schema file extension '.{other}': expected .fbs, .json, or .proto"
        )),
    }
}

fn load_from_fbs(path: &Path, include_paths: &[PathBuf]) -> Result<LoadedSchema, String> {
    let options = flatc_rs_compiler::CompilerOptions {
        include_paths: include_paths.to_vec(),
    };
    let result = flatc_rs_compiler::compile(&[path.to_path_buf()], &options)
        .map_err(|e| format!("compilation failed: {e}"))?;

    let root_type_name = result
        .schema
        .root_table_index
        .and_then(|idx| result.schema.objects.get(idx))
        .map(|obj| obj.name.clone());

    Ok(LoadedSchema::FlatBuffers {
        schema: Box::new(result.schema),
        root_type_name,
    })
}

fn load_from_json(path: &Path) -> Result<LoadedSchema, String> {
    let json = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let result = flatbuf_visualizer_core::load_schema_from_json(&json)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(LoadedSchema::FlatBuffers {
        schema: Box::new(result.schema),
        root_type_name: result.root_type_name,
    })
}

fn load_from_proto(path: &Path) -> Result<LoadedSchema, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let fds = protoc_rs_analyzer::analyze(&source)
        .map_err(|e| format!("proto compilation failed: {e}"))?;

    let msg_names = collect_proto_message_names(&fds);

    Ok(LoadedSchema::Protobuf {
        schema: fds,
        message_names: msg_names,
    })
}
