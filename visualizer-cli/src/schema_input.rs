use std::fs;
use std::path::{Path, PathBuf};

use flatbuf_visualizer_core::Schema;

/// Load a schema from either a `.fbs` file (compiled via flatc-rs-compiler)
/// or a `.json` file (pre-compiled schema JSON from `flatc-rs --dump-schema`).
pub fn load_schema(
    schema_path: &Path,
    include_paths: &[PathBuf],
) -> Result<(Schema, Option<String>), String> {
    let ext = schema_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "fbs" => load_from_fbs(schema_path, include_paths),
        "json" => load_from_json(schema_path),
        other => Err(format!(
            "unsupported schema file extension '.{other}': expected .fbs or .json"
        )),
    }
}

fn load_from_fbs(
    path: &Path,
    include_paths: &[PathBuf],
) -> Result<(Schema, Option<String>), String> {
    let options = flatc_rs_compiler::CompilerOptions {
        include_paths: include_paths.to_vec(),
    };
    let result = flatc_rs_compiler::compile(&[path.to_path_buf()], &options)
        .map_err(|e| format!("compilation failed: {e}"))?;

    let root_type_name = result
        .schema
        .root_table
        .as_ref()
        .and_then(|rt| rt.name.clone());

    Ok((result.schema, root_type_name))
}

fn load_from_json(path: &Path) -> Result<(Schema, Option<String>), String> {
    let json = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let result = flatbuf_visualizer_core::load_schema_from_json(&json)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((result.schema, result.root_type_name))
}
