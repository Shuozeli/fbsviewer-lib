use std::fs;
use std::path::PathBuf;
use std::process;

use clap::Parser;

mod filter;
mod output;
mod schema_input;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "flatbuf-viz")]
#[command(about = "Visualize FlatBuffers and Protobuf binary encoding with schema annotations")]
#[command(version = VERSION)]
struct Cli {
    /// Schema file: .fbs, .json (FlatBuffers), or .proto (Protobuf).
    #[arg(short = 's', long = "schema")]
    schema: PathBuf,

    /// Binary data file: raw .bin file or hex-dump text file.
    #[arg(short = 'b', long = "binary")]
    binary: PathBuf,

    /// Treat binary input as hex dump text instead of raw binary.
    /// Auto-detected if file extension is .hex or .txt.
    #[arg(long)]
    hex: bool,

    /// Override the root type name from the schema.
    #[arg(long)]
    root_type: Option<String>,

    /// Search for includes in the specified path (for .fbs schema compilation).
    #[arg(short = 'I', long = "include")]
    include: Vec<PathBuf>,

    // -- Filters --
    /// Show only regions overlapping this byte range (e.g. "16..32" or "0x10..0x20").
    #[arg(long)]
    byte_range: Option<String>,

    /// Show only regions whose field path contains this substring (e.g. "Monster.name").
    #[arg(long)]
    field: Option<String>,

    /// Show only regions matching this region type short name (e.g. "vtable", "scalar").
    #[arg(long)]
    region_type: Option<String>,

    // -- Output format --
    /// Output format: table (default), json, compact.
    #[arg(long, default_value = "table")]
    format: output::OutputFormat,
}

fn main() {
    let cli = Cli::parse();

    // 1. Load schema
    let loaded = match schema_input::load_schema(&cli.schema, &cli.include) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: failed to load schema: {e}");
            process::exit(1);
        }
    };

    // 2. Load binary data
    let binary = match load_binary(&cli.binary, cli.hex) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: failed to load binary: {e}");
            process::exit(1);
        }
    };

    // 3. Walk binary (dispatch based on schema type)
    let regions = match loaded {
        schema_input::LoadedSchema::FlatBuffers {
            schema,
            root_type_name,
        } => {
            let root_type = cli.root_type.or(root_type_name).unwrap_or_else(|| {
                eprintln!("error: no root type found in schema and --root-type not specified");
                process::exit(1);
            });
            match flatbuf_visualizer_core::walk_binary(&binary, &schema, &root_type) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: walk failed: {e}");
                    process::exit(1);
                }
            }
        }
        schema_input::LoadedSchema::Protobuf {
            schema,
            message_names,
        } => {
            let root_message = cli.root_type.unwrap_or_else(|| {
                if let Some(first) = message_names.first() {
                    first.clone()
                } else {
                    eprintln!(
                        "error: no messages found in proto schema and --root-type not specified"
                    );
                    process::exit(1);
                }
            });
            match flatbuf_visualizer_core::walk_protobuf(&binary, &schema, &root_message) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: protobuf walk failed: {e}");
                    process::exit(1);
                }
            }
        }
    };

    // 4. Apply filters
    let filtered = match filter::apply_filters(
        &regions,
        cli.byte_range.as_deref(),
        cli.field.as_deref(),
        cli.region_type.as_deref(),
    ) {
        Ok(indices) => indices,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // 5. Render output
    output::render(&regions, &filtered, &cli.format);
}

fn load_binary(path: &PathBuf, force_hex: bool) -> Result<Vec<u8>, String> {
    let is_hex = force_hex
        || matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("hex" | "txt")
        );

    if is_hex {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        flatbuf_visualizer_core::parse_hex_bytes(&text)
            .map_err(|e| format!("{}: {e}", path.display()))
    } else {
        fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
    }
}
