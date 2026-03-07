//! Pure application state, commands, and effects for the FlatBuffers visualizer.
//!
//! This module has **zero dependency on `egui`**. All state transitions go through
//! [`AppState::dispatch`], which returns [`Effect`] values for the runtime to execute.
//! Effect results are fed back as [`Command`] variants, creating a closed loop that
//! is fully testable without a GUI.

use std::collections::VecDeque;

use flatbuf_visualizer_core::{AnnotatedRegion, Schema};

use crate::permalink;
use crate::templates;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const DEMO_SCHEMA: &str = r#"namespace MyGame;

enum Color : byte { Red = 1, Green, Blue }

struct Vec3 {
  x: float;
  y: float;
  z: float;
}

table Monster {
  pos: Vec3;
  mana: short = 150;
  hp: short = 100;
  name: string;
  color: Color = Blue;
  inventory: [ubyte];
}

root_type Monster;
"#;

pub const DEMO_DATA: &str = r#"{
  "pos": { "x": 1.0, "y": 2.0, "z": 3.0 },
  "mana": 200,
  "hp": 300,
  "name": "Orc",
  "color": "Red",
  "inventory": [0, 1, 2, 3, 4]
}"#;

// ---------------------------------------------------------------------------
// DataFormat
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    Json,
    Binary,
}

// ---------------------------------------------------------------------------
// Command -- every possible input to the state machine
// ---------------------------------------------------------------------------

/// Every user action or effect result that can modify application state.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Some variants are only used in tests
pub enum Command {
    // -- User interactions --
    SelectTemplate(usize),
    CompileAndEncode,
    SwitchDataFormat(DataFormat),
    EditSchema(String),
    EditData(String),
    EditRootType(String),
    ToggleSchemaJson,
    ToggleDecodedJson,
    HoverRegion(Option<usize>),
    ClickRegion(usize),
    UnlockRegion,

    // -- File / data loading (platform-agnostic payloads) --
    LoadSchemaText(String),
    SetBinaryData(Vec<u8>),

    // -- Sharing --
    CopyShareLink,
    LoadFromPermalink(String),

    // -- Random generation --
    GenerateRandom {
        seed: u64,
    },

    // -- Side effect results (fed back by the runtime) --
    SchemaCompiled {
        schema: Box<Schema>,
        schema_json: String,
        root_type_name: Option<String>,
    },
    SchemaCompileError(String),
    JsonEncoded(Vec<u8>),
    EncodeError(String),
    BinaryWalked {
        annotations: Vec<AnnotatedRegion>,
        decoded_json: String,
    },
    WalkError(String),
    RandomGenerated {
        schema_text: String,
        data_text: String,
    },
    RandomGenerateError(String),
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::SelectTemplate(idx) => write!(f, "SelectTemplate({idx})"),
            Command::CompileAndEncode => write!(f, "CompileAndEncode"),
            Command::SwitchDataFormat(fmt) => write!(f, "SwitchDataFormat({fmt:?})"),
            Command::EditSchema(_) => write!(f, "EditSchema(...)"),
            Command::EditData(_) => write!(f, "EditData(...)"),
            Command::EditRootType(name) => write!(f, "EditRootType({name:?})"),
            Command::ToggleSchemaJson => write!(f, "ToggleSchemaJson"),
            Command::ToggleDecodedJson => write!(f, "ToggleDecodedJson"),
            Command::HoverRegion(r) => write!(f, "HoverRegion({r:?})"),
            Command::ClickRegion(idx) => write!(f, "ClickRegion({idx})"),
            Command::UnlockRegion => write!(f, "UnlockRegion"),
            Command::CopyShareLink => write!(f, "CopyShareLink"),
            Command::LoadFromPermalink(_) => write!(f, "LoadFromPermalink(...)"),
            Command::LoadSchemaText(_) => write!(f, "LoadSchemaText(...)"),
            Command::SetBinaryData(d) => write!(f, "SetBinaryData({} bytes)", d.len()),
            Command::SchemaCompiled { root_type_name, .. } => {
                write!(f, "SchemaCompiled(root={root_type_name:?})")
            }
            Command::SchemaCompileError(e) => write!(f, "SchemaCompileError({e})"),
            Command::JsonEncoded(d) => write!(f, "JsonEncoded({} bytes)", d.len()),
            Command::EncodeError(e) => write!(f, "EncodeError({e})"),
            Command::BinaryWalked { annotations, .. } => {
                write!(f, "BinaryWalked({} regions)", annotations.len())
            }
            Command::WalkError(e) => write!(f, "WalkError({e})"),
            Command::GenerateRandom { seed } => write!(f, "GenerateRandom(seed={seed})"),
            Command::RandomGenerated { .. } => write!(f, "RandomGenerated(...)"),
            Command::RandomGenerateError(e) => write!(f, "RandomGenerateError({e})"),
        }
    }
}

// ---------------------------------------------------------------------------
// Effect -- side effects returned by dispatch()
// ---------------------------------------------------------------------------

/// Side effects that [`AppState::dispatch`] requests. The runtime executes
/// these and feeds results back as [`Command`] variants.
#[derive(Debug)]
pub enum Effect {
    /// Compile schema text. Result: `SchemaCompiled` or `SchemaCompileError`.
    CompileSchema { source: String },
    /// Encode JSON data to FlatBuffers binary. Result: `JsonEncoded` or `EncodeError`.
    EncodeJson {
        json_text: String,
        schema: Schema,
        root_type_name: String,
    },
    /// Parse hex text to binary bytes. Result: `SetBinaryData` or `EncodeError`.
    ParseHexData { hex_text: String },
    /// Walk binary data to produce annotations. Result: `BinaryWalked` or `WalkError`.
    WalkBinary {
        binary: Vec<u8>,
        schema: Schema,
        root_type_name: String,
    },
    /// Generate a random schema and matching JSON data. Result: `RandomGenerated` or `RandomGenerateError`.
    GenerateRandomSchemaAndData { seed: u64 },
    /// Copy text to the system clipboard (URL for sharing).
    CopyToClipboard { text: String },
    /// Update the browser URL hash (WASM only, no-op on native).
    SetUrlHash { hash: String },
}

impl std::fmt::Display for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::CompileSchema { .. } => write!(f, "CompileSchema"),
            Effect::EncodeJson { .. } => write!(f, "EncodeJson"),
            Effect::ParseHexData { .. } => write!(f, "ParseHexData"),
            Effect::WalkBinary { .. } => write!(f, "WalkBinary"),
            Effect::GenerateRandomSchemaAndData { seed } => {
                write!(f, "GenerateRandomSchemaAndData(seed={seed})")
            }
            Effect::CopyToClipboard { .. } => write!(f, "CopyToClipboard"),
            Effect::SetUrlHash { .. } => write!(f, "SetUrlHash"),
        }
    }
}

// ---------------------------------------------------------------------------
// EventLog -- bounded audit trail
// ---------------------------------------------------------------------------

const MAX_EVENT_LOG_ENTRIES: usize = 200;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EventLogEntry {
    pub command: String,
    pub effects: Vec<String>,
}

// ---------------------------------------------------------------------------
// AppState -- pure application state
// ---------------------------------------------------------------------------

/// Pure application state with zero `egui` dependency.
///
/// All mutations go through [`dispatch`](AppState::dispatch), which returns
/// [`Effect`] values for the runtime to execute.
#[derive(Debug, Clone)]
pub struct AppState {
    // -- User inputs --
    pub schema_text: String,
    pub data_text: String,
    pub data_format: DataFormat,
    pub root_type_name: String,

    // -- Derived / computed state --
    pub compiled_schema: Option<Schema>,
    pub compile_error: Option<String>,
    pub binary_data: Option<Vec<u8>>,
    pub encode_error: Option<String>,
    pub annotations: Option<Vec<AnnotatedRegion>>,

    // -- Interaction state --
    pub hovered_region: Option<usize>,
    pub locked_region: Option<usize>,

    // -- View state --
    pub schema_json_output: String,
    pub show_schema_json: bool,
    pub decoded_json: String,
    pub show_decoded_json: bool,

    // -- UI state --
    pub selected_template_idx: usize,
    pub status_message: String,

    // -- Structure tree view --
    /// Generation counter for resetting collapsing header state.
    pub structure_tree_gen: u64,
    /// None = default (depth < 2 open), Some(true) = all open, Some(false) = all closed.
    pub structure_all_open: Option<bool>,

    // -- Toast notification --
    /// Temporary toast message shown briefly (e.g. "Link copied!").
    pub toast_message: Option<String>,
    /// Frame counter for auto-dismissing the toast.
    pub toast_frames_remaining: u32,

    // -- Random generation --
    pub random_seed_counter: u64,

    // -- Event log --
    pub event_log: VecDeque<EventLogEntry>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            schema_text: DEMO_SCHEMA.to_string(),
            data_text: DEMO_DATA.to_string(),
            data_format: DataFormat::Json,
            root_type_name: String::new(),
            compiled_schema: None,
            compile_error: None,
            binary_data: None,
            encode_error: None,
            annotations: None,
            hovered_region: None,
            locked_region: None,
            schema_json_output: String::new(),
            show_schema_json: false,
            decoded_json: String::new(),
            show_decoded_json: false,
            selected_template_idx: 0,
            status_message: "Ready.".to_string(),
            structure_tree_gen: 0,
            structure_all_open: None,
            toast_message: None,
            toast_frames_remaining: 0,
            random_seed_counter: 0,
            event_log: VecDeque::new(),
        }
    }
}

impl AppState {
    /// Pure state transition. Returns effects for the runtime to execute.
    pub fn dispatch(&mut self, cmd: Command) -> Vec<Effect> {
        let cmd_str = cmd.to_string();
        let mut effects = vec![];

        match cmd {
            // ----- User interactions -----
            Command::SelectTemplate(idx) => {
                let all = templates::all();
                if idx < all.len() {
                    let t = &all[idx];
                    self.schema_text = t.schema.to_string();
                    self.data_text = t.json_data.to_string();
                    self.data_format = DataFormat::Json;
                    self.root_type_name.clear();
                    self.locked_region = None;
                    self.selected_template_idx = idx;
                    effects.push(Effect::CompileSchema {
                        source: self.schema_text.clone(),
                    });
                }
            }

            Command::CompileAndEncode => {
                effects.push(Effect::CompileSchema {
                    source: self.schema_text.clone(),
                });
            }

            Command::SwitchDataFormat(new_format) => {
                let prev = self.data_format;
                if prev == new_format {
                    return effects;
                }
                self.data_format = new_format;
                match (prev, new_format) {
                    (DataFormat::Json, DataFormat::Binary) => {
                        // Convert displayed data to hex representation
                        if let Some(ref binary) = self.binary_data {
                            self.data_text = bytes_to_hex(binary);
                            self.status_message = "Switched to Hex view.".to_string();
                        }
                    }
                    (DataFormat::Binary, DataFormat::Json) => {
                        if !self.decoded_json.is_empty() {
                            self.data_text = self.decoded_json.clone();
                        } else {
                            self.data_text = "{}".to_string();
                        }
                        self.status_message = "Switched to JSON view.".to_string();
                    }
                    _ => {}
                }
                // Re-compile and re-encode with new format
                effects.push(Effect::CompileSchema {
                    source: self.schema_text.clone(),
                });
            }

            Command::EditSchema(text) => {
                self.schema_text = text;
            }
            Command::EditData(text) => {
                self.data_text = text;
            }
            Command::EditRootType(text) => {
                self.root_type_name = text;
            }

            Command::LoadSchemaText(text) => {
                self.schema_text = text;
                self.status_message = "Schema file loaded.".to_string();
            }

            Command::SetBinaryData(data) => {
                self.status_message = format!("Loaded {} bytes. Walking binary...", data.len());
                self.binary_data = Some(data.clone());
                self.encode_error = None;
                // Auto-walk if schema is available
                if let Some(ref schema) = self.compiled_schema {
                    let root_name = self.effective_root_type_name(schema);
                    effects.push(Effect::WalkBinary {
                        binary: data,
                        schema: schema.clone(),
                        root_type_name: root_name,
                    });
                }
            }

            Command::ToggleSchemaJson => {
                self.show_schema_json = !self.show_schema_json;
            }
            Command::ToggleDecodedJson => {
                self.show_decoded_json = !self.show_decoded_json;
            }

            Command::HoverRegion(region) => {
                self.hovered_region = region;
                self.update_status_from_interaction();
            }
            Command::ClickRegion(idx) => {
                if self.locked_region == Some(idx) {
                    self.locked_region = None;
                } else {
                    self.locked_region = Some(idx);
                }
                self.update_status_from_interaction();
            }
            Command::UnlockRegion => {
                self.locked_region = None;
            }

            Command::CopyShareLink => {
                let data = permalink::PermalinkData {
                    schema_text: self.schema_text.clone(),
                    data_text: self.data_text.clone(),
                    is_hex_format: self.data_format == DataFormat::Binary,
                };
                match permalink::encode(&data) {
                    Ok(encoded) => {
                        self.status_message = "Share link copied to clipboard.".to_string();
                        self.toast_message = Some("Link copied to clipboard!".to_string());
                        self.toast_frames_remaining = 120; // ~2 seconds at 60fps
                        effects.push(Effect::SetUrlHash {
                            hash: encoded.clone(),
                        });
                        effects.push(Effect::CopyToClipboard { text: encoded });
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to create share link: {e}");
                    }
                }
            }

            Command::LoadFromPermalink(encoded) => match permalink::decode(&encoded) {
                Ok(data) => {
                    self.schema_text = data.schema_text;
                    self.data_text = data.data_text;
                    self.data_format = if data.is_hex_format {
                        DataFormat::Binary
                    } else {
                        DataFormat::Json
                    };
                    self.root_type_name.clear();
                    self.locked_region = None;
                    self.status_message = "Loaded from shared link.".to_string();
                    effects.push(Effect::CompileSchema {
                        source: self.schema_text.clone(),
                    });
                }
                Err(e) => {
                    self.status_message = format!("Failed to load shared link: {e}");
                }
            },

            // ----- Side effect results -----
            Command::SchemaCompiled {
                schema,
                schema_json,
                root_type_name,
            } => {
                if self.root_type_name.is_empty() {
                    if let Some(name) = root_type_name {
                        self.root_type_name = name;
                    }
                }
                self.schema_json_output = schema_json;
                let schema = *schema;
                self.compiled_schema = Some(schema.clone());
                self.compile_error = None;
                self.status_message = "Schema compiled successfully.".to_string();

                // Chain: encode or parse data depending on format
                let root_name = self.effective_root_type_name(&schema);
                match self.data_format {
                    DataFormat::Json => {
                        effects.push(Effect::EncodeJson {
                            json_text: self.data_text.clone(),
                            schema,
                            root_type_name: root_name,
                        });
                    }
                    DataFormat::Binary => {
                        effects.push(Effect::ParseHexData {
                            hex_text: self.data_text.clone(),
                        });
                    }
                }
            }

            Command::SchemaCompileError(err) => {
                self.compiled_schema = None;
                self.schema_json_output.clear();
                self.compile_error = Some(err.clone());
                self.status_message = format!("Compile error: {err}");
            }

            Command::JsonEncoded(binary) => {
                self.status_message = format!("Encoded {} bytes. Walking binary...", binary.len());
                self.binary_data = Some(binary.clone());
                self.encode_error = None;
                // Chain: walk the encoded binary
                if let Some(ref schema) = self.compiled_schema {
                    let root_name = self.effective_root_type_name(schema);
                    effects.push(Effect::WalkBinary {
                        binary,
                        schema: schema.clone(),
                        root_type_name: root_name,
                    });
                }
            }

            Command::EncodeError(err) => {
                self.encode_error = Some(err.clone());
                self.status_message = format!("Encode error: {err}");
            }

            Command::BinaryWalked {
                annotations,
                decoded_json,
            } => {
                let count = annotations.len();
                let data_len = self.binary_data.as_ref().map(|d| d.len()).unwrap_or(0);
                self.decoded_json = decoded_json;
                self.annotations = Some(annotations);
                self.encode_error = None;
                self.status_message =
                    format!("{data_len} bytes, {count} regions. Hover to explore.");
            }

            Command::WalkError(err) => {
                self.annotations = None;
                self.decoded_json.clear();
                self.encode_error = Some(format!("walker error: {err}"));
                self.status_message = format!("Walker error: {err}");
            }

            Command::GenerateRandom { seed } => {
                self.random_seed_counter = seed.wrapping_add(1);
                self.status_message = "Generating random schema and data...".to_string();
                effects.push(Effect::GenerateRandomSchemaAndData { seed });
            }

            Command::RandomGenerated {
                schema_text,
                data_text,
            } => {
                self.schema_text = schema_text;
                self.data_text = data_text;
                self.data_format = DataFormat::Json;
                self.root_type_name.clear();
                self.locked_region = None;
                self.status_message = "Random schema and data generated.".to_string();
                effects.push(Effect::CompileSchema {
                    source: self.schema_text.clone(),
                });
            }

            Command::RandomGenerateError(err) => {
                self.status_message = format!("Random generation error: {err}");
            }
        }

        // Log
        let effect_strs: Vec<String> = effects.iter().map(|e| e.to_string()).collect();
        self.event_log.push_back(EventLogEntry {
            command: cmd_str,
            effects: effect_strs,
        });
        if self.event_log.len() > MAX_EVENT_LOG_ENTRIES {
            self.event_log.pop_front();
        }

        effects
    }

    /// Derive root type name from state, falling back to schema's root_table.
    fn effective_root_type_name(&self, schema: &Schema) -> String {
        if !self.root_type_name.is_empty() {
            self.root_type_name.clone()
        } else {
            schema
                .root_table
                .as_ref()
                .and_then(|t| t.name.as_deref())
                .unwrap_or("")
                .to_string()
        }
    }

    /// Tick down the toast notification timer. Call once per frame.
    pub fn tick_toast(&mut self) {
        if self.toast_frames_remaining > 0 {
            self.toast_frames_remaining -= 1;
            if self.toast_frames_remaining == 0 {
                self.toast_message = None;
            }
        }
    }

    /// Update status message based on current hovered/locked region.
    fn update_status_from_interaction(&mut self) {
        let active_region = self.locked_region.or(self.hovered_region);
        if let Some(idx) = active_region {
            if let Some(ref annotations) = self.annotations {
                if let Some(r) = annotations.get(idx) {
                    let lock_indicator = if self.locked_region == Some(idx) {
                        "[locked] "
                    } else {
                        ""
                    };
                    self.status_message = format!(
                        "{}0x{:04X}..0x{:04X} | {} | {} | {}",
                        lock_indicator,
                        r.byte_range.start,
                        r.byte_range.end,
                        r.field_path.join("."),
                        r.region_type.short_name(),
                        r.value_display,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn bytes_to_hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flatbuf_visualizer_core::{annotations_to_json, encode_json, parse_hex_bytes, walk_binary};

    /// Execute a command and recursively resolve all effects using real
    /// compiler/encoder/walker implementations.
    fn run_with_effects(state: &mut AppState, cmd: Command) {
        let effects = state.dispatch(cmd);
        for effect in effects {
            let result_cmd = execute_effect_sync(effect);
            if let Some(cmd) = result_cmd {
                run_with_effects(state, cmd);
            }
        }
    }

    fn execute_effect_sync(effect: Effect) -> Option<Command> {
        match effect {
            Effect::CompileSchema { source } => {
                let result =
                    std::panic::catch_unwind(|| flatc_rs_compiler::compile_single(&source));
                match result {
                    Ok(Ok(result)) => {
                        let root_name = result
                            .schema
                            .root_table
                            .as_ref()
                            .and_then(|t| t.name.clone());
                        let schema_json =
                            serde_json::to_string_pretty(&result.schema).unwrap_or_default();
                        Some(Command::SchemaCompiled {
                            schema: Box::new(result.schema),
                            schema_json,
                            root_type_name: root_name,
                        })
                    }
                    Ok(Err(e)) => Some(Command::SchemaCompileError(e.to_string())),
                    Err(_) => Some(Command::SchemaCompileError(
                        "internal error: schema compiler panicked".to_string(),
                    )),
                }
            }
            Effect::EncodeJson {
                json_text,
                schema,
                root_type_name,
            } => {
                let json_value: serde_json::Value = match serde_json::from_str(&json_text) {
                    Ok(v) => v,
                    Err(e) => return Some(Command::EncodeError(format!("Invalid JSON: {e}"))),
                };
                match encode_json(&json_value, &schema, &root_type_name) {
                    Ok(binary) => Some(Command::JsonEncoded(binary)),
                    Err(e) => Some(Command::EncodeError(e.to_string())),
                }
            }
            Effect::ParseHexData { hex_text } => match parse_hex_bytes(&hex_text) {
                Ok(bytes) => Some(Command::SetBinaryData(bytes)),
                Err(e) => Some(Command::EncodeError(e.to_string())),
            },
            Effect::WalkBinary {
                binary,
                schema,
                root_type_name,
            } => match walk_binary(&binary, &schema, &root_type_name) {
                Ok(annotations) => {
                    let json_value = annotations_to_json(&annotations);
                    let decoded_json =
                        serde_json::to_string_pretty(&json_value).unwrap_or_default();
                    Some(Command::BinaryWalked {
                        annotations,
                        decoded_json,
                    })
                }
                Err(e) => Some(Command::WalkError(e.to_string())),
            },
            Effect::GenerateRandomSchemaAndData { seed } => {
                let gen_config = flatc_rs_fbs_gen::GenConfig {
                    max_enums: 2,
                    max_structs: 2,
                    max_tables: 3,
                    max_unions: 1,
                    max_fields_per_type: 4,
                    use_namespace: false,
                    use_file_ident: false,
                    prob_deprecated: 0.0,
                    prob_null_default: 0.0,
                    prob_nan_inf_default: 0.0,
                    prob_rpc_service: 0.0,
                    prob_doc_comment: 0.0,
                    prob_fixed_array: 0.0,
                    ..flatc_rs_fbs_gen::GenConfig::default()
                };

                let fbs_text = flatc_rs_fbs_gen::SchemaBuilder::generate(seed, gen_config);

                let compile_result =
                    match std::panic::catch_unwind(|| flatc_rs_compiler::compile_single(&fbs_text))
                    {
                        Ok(Ok(r)) => r,
                        Ok(Err(e)) => {
                            return Some(Command::RandomGenerateError(format!(
                                "Schema compile failed: {e}"
                            )));
                        }
                        Err(_) => {
                            return Some(Command::RandomGenerateError(
                                "Schema compiler panicked".to_string(),
                            ));
                        }
                    };

                let root_type = compile_result
                    .schema
                    .root_table
                    .as_ref()
                    .and_then(|t| t.name.as_deref())
                    .unwrap_or("")
                    .to_string();

                let data_config = flatc_rs_data_gen::DataGenConfig::default();
                match flatc_rs_data_gen::generate_json(
                    &compile_result.schema,
                    &root_type,
                    seed,
                    data_config,
                ) {
                    Ok(json_text) => Some(Command::RandomGenerated {
                        schema_text: fbs_text,
                        data_text: json_text,
                    }),
                    Err(e) => Some(Command::RandomGenerateError(e.to_string())),
                }
            }
            // Platform effects are no-ops in tests
            Effect::CopyToClipboard { .. } | Effect::SetUrlHash { .. } => None,
        }
    }

    #[test]
    fn test_default_state() {
        let state = AppState::default();
        assert_eq!(state.data_format, DataFormat::Json);
        assert!(state.compiled_schema.is_none());
        assert!(state.binary_data.is_none());
        assert!(state.annotations.is_none());
        assert_eq!(state.status_message, "Ready.");
        assert!(state.root_type_name.is_empty());
    }

    #[test]
    fn test_click_region_toggles_lock() {
        let mut state = AppState::default();
        state.dispatch(Command::ClickRegion(5));
        assert_eq!(state.locked_region, Some(5));
        state.dispatch(Command::ClickRegion(5));
        assert_eq!(state.locked_region, None);
        state.dispatch(Command::ClickRegion(3));
        assert_eq!(state.locked_region, Some(3));
        state.dispatch(Command::ClickRegion(7));
        assert_eq!(state.locked_region, Some(7));
    }

    #[test]
    fn test_unlock_region() {
        let mut state = AppState::default();
        state.dispatch(Command::ClickRegion(5));
        assert_eq!(state.locked_region, Some(5));
        state.dispatch(Command::UnlockRegion);
        assert_eq!(state.locked_region, None);
    }

    #[test]
    fn test_toggle_schema_json() {
        let mut state = AppState::default();
        assert!(!state.show_schema_json);
        state.dispatch(Command::ToggleSchemaJson);
        assert!(state.show_schema_json);
        state.dispatch(Command::ToggleSchemaJson);
        assert!(!state.show_schema_json);
    }

    #[test]
    fn test_toggle_decoded_json() {
        let mut state = AppState::default();
        assert!(!state.show_decoded_json);
        state.dispatch(Command::ToggleDecodedJson);
        assert!(state.show_decoded_json);
        state.dispatch(Command::ToggleDecodedJson);
        assert!(!state.show_decoded_json);
    }

    #[test]
    fn test_select_template_resets_format_to_json() {
        let mut state = AppState::default();
        state.data_format = DataFormat::Binary;
        state.locked_region = Some(3);
        let effects = state.dispatch(Command::SelectTemplate(1));
        assert_eq!(state.data_format, DataFormat::Json);
        assert_eq!(state.selected_template_idx, 1);
        assert!(state.locked_region.is_none());
        assert!(state.root_type_name.is_empty());
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::CompileSchema { .. })));
    }

    #[test]
    fn test_schema_compiled_chains_to_encode_json() {
        let mut state = AppState::default();
        state.data_format = DataFormat::Json;
        let effects = state.dispatch(Command::SchemaCompiled {
            schema: Box::new(Schema::default()),
            schema_json: "{}".to_string(),
            root_type_name: Some("Monster".to_string()),
        });
        assert!(state.compiled_schema.is_some());
        assert!(state.compile_error.is_none());
        assert_eq!(state.root_type_name, "Monster");
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::EncodeJson { .. })));
    }

    #[test]
    fn test_schema_compiled_chains_to_parse_hex_when_binary_format() {
        let mut state = AppState::default();
        state.data_format = DataFormat::Binary;
        state.data_text = "0a 0b 0c".to_string();
        let effects = state.dispatch(Command::SchemaCompiled {
            schema: Box::new(Schema::default()),
            schema_json: "{}".to_string(),
            root_type_name: Some("Test".to_string()),
        });
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::ParseHexData { .. })));
    }

    #[test]
    fn test_schema_compile_error_clears_schema() {
        let mut state = AppState::default();
        state.compiled_schema = Some(Schema::default());
        state.dispatch(Command::SchemaCompileError("syntax error".to_string()));
        assert!(state.compiled_schema.is_none());
        assert_eq!(state.compile_error.as_deref(), Some("syntax error"));
    }

    #[test]
    fn test_walk_error_clears_annotations() {
        let mut state = AppState::default();
        state.annotations = Some(vec![]);
        state.decoded_json = "some json".to_string();
        state.dispatch(Command::WalkError("bad data".to_string()));
        assert!(state.annotations.is_none());
        assert!(state.decoded_json.is_empty());
    }

    #[test]
    fn test_format_switch_json_to_hex_preserves_annotations() {
        let mut state = AppState::default();
        state.data_format = DataFormat::Json;
        state.binary_data = Some(vec![0x14, 0x00, 0x00, 0x00]);
        state.annotations = Some(vec![]); // non-None

        // Switch format -- dispatch returns effects but don't resolve them
        let _effects = state.dispatch(Command::SwitchDataFormat(DataFormat::Binary));

        // Annotations must survive the format switch itself
        assert!(
            state.annotations.is_some(),
            "Annotations must survive JSON->Hex format switch"
        );
        assert_eq!(state.data_format, DataFormat::Binary);
        assert!(state.data_text.contains("14 00 00 00"));
    }

    #[test]
    fn test_format_switch_hex_to_json_uses_decoded_json() {
        let mut state = AppState::default();
        state.data_format = DataFormat::Binary;
        state.decoded_json = r#"{"name": "Orc"}"#.to_string();

        let _effects = state.dispatch(Command::SwitchDataFormat(DataFormat::Json));

        assert_eq!(state.data_format, DataFormat::Json);
        assert_eq!(state.data_text, r#"{"name": "Orc"}"#);
    }

    #[test]
    fn test_format_switch_hex_to_json_fallback_empty() {
        let mut state = AppState::default();
        state.data_format = DataFormat::Binary;
        state.decoded_json = String::new();

        let _effects = state.dispatch(Command::SwitchDataFormat(DataFormat::Json));

        assert_eq!(state.data_text, "{}");
    }

    #[test]
    fn test_load_schema_text() {
        let mut state = AppState::default();
        state.dispatch(Command::LoadSchemaText("table Foo {}".to_string()));
        assert_eq!(state.schema_text, "table Foo {}");
        assert_eq!(state.status_message, "Schema file loaded.");
    }

    #[test]
    fn test_event_log_records_commands() {
        let mut state = AppState::default();
        state.dispatch(Command::ToggleSchemaJson);
        state.dispatch(Command::ClickRegion(3));
        assert_eq!(state.event_log.len(), 2);
        assert_eq!(state.event_log[0].command, "ToggleSchemaJson");
        assert_eq!(state.event_log[1].command, "ClickRegion(3)");
    }

    // -----------------------------------------------------------------------
    // Integration tests with real compiler/encoder/walker
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_compile_encode_walk_pipeline() {
        let mut state = AppState::default();
        run_with_effects(&mut state, Command::CompileAndEncode);

        assert!(
            state.compiled_schema.is_some(),
            "Schema should compile successfully"
        );
        assert!(state.compile_error.is_none());
        assert!(
            state.binary_data.is_some(),
            "Binary data should be produced"
        );
        assert!(state.encode_error.is_none());
        assert!(
            state.annotations.is_some(),
            "Annotations should be produced"
        );
        assert!(!state.decoded_json.is_empty());
        let annotations = state.annotations.as_ref().unwrap();
        assert!(
            !annotations.is_empty(),
            "Should have at least one annotation region"
        );
    }

    #[test]
    fn test_format_switch_roundtrip() {
        let mut state = AppState::default();
        // Compile with JSON format
        run_with_effects(&mut state, Command::CompileAndEncode);
        let original_binary = state.binary_data.clone().unwrap();
        let original_annotations_count = state.annotations.as_ref().unwrap().len();

        // Switch to Hex
        run_with_effects(&mut state, Command::SwitchDataFormat(DataFormat::Binary));
        assert_eq!(state.data_format, DataFormat::Binary);
        assert!(
            state.annotations.is_some(),
            "Annotations must survive JSON->Hex switch"
        );
        let hex_binary = state.binary_data.clone().unwrap();
        assert_eq!(
            original_binary, hex_binary,
            "Binary data must be identical after format switch"
        );

        // Switch back to JSON
        run_with_effects(&mut state, Command::SwitchDataFormat(DataFormat::Json));
        assert_eq!(state.data_format, DataFormat::Json);
        assert!(
            state.annotations.is_some(),
            "Annotations must survive Hex->JSON switch"
        );
        assert_eq!(
            state.annotations.as_ref().unwrap().len(),
            original_annotations_count,
            "Annotation count must match after round-trip"
        );
    }

    // -----------------------------------------------------------------------
    // Random generation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_random_dispatches_effect() {
        let mut state = AppState::default();
        let effects = state.dispatch(Command::GenerateRandom { seed: 42 });
        assert_eq!(state.random_seed_counter, 43);
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::GenerateRandomSchemaAndData { seed: 42 })));
    }

    #[test]
    fn test_random_generated_chains_to_compile() {
        let mut state = AppState::default();
        state.data_format = DataFormat::Binary;
        state.locked_region = Some(3);
        let effects = state.dispatch(Command::RandomGenerated {
            schema_text: "table Foo {}".to_string(),
            data_text: "{}".to_string(),
        });
        assert_eq!(state.schema_text, "table Foo {}");
        assert_eq!(state.data_text, "{}");
        assert_eq!(state.data_format, DataFormat::Json);
        assert!(state.root_type_name.is_empty());
        assert!(state.locked_region.is_none());
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::CompileSchema { .. })));
    }

    #[test]
    fn test_random_error_updates_status() {
        let mut state = AppState::default();
        state.dispatch(Command::RandomGenerateError("test error".to_string()));
        assert!(state.status_message.contains("test error"));
    }

    #[test]
    fn test_full_random_pipeline() {
        let mut state = AppState::default();
        run_with_effects(&mut state, Command::GenerateRandom { seed: 42 });
        assert!(
            state.compiled_schema.is_some(),
            "Random schema should compile"
        );
        assert!(
            state.binary_data.is_some(),
            "Random data should encode to binary"
        );
        assert!(
            state.annotations.is_some(),
            "Random binary should produce annotations"
        );
        assert!(!state.schema_text.is_empty());
        assert!(!state.data_text.is_empty());
    }

    #[test]
    fn test_all_templates_compile_and_walk() {
        let all = templates::all();
        for (i, _t) in all.iter().enumerate() {
            let mut state = AppState::default();
            run_with_effects(&mut state, Command::SelectTemplate(i));
            assert!(
                state.compiled_schema.is_some(),
                "Template {i} should compile successfully"
            );
            assert!(
                state.annotations.is_some(),
                "Template {i} should produce annotations"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Permalink tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_copy_share_link_produces_effects() {
        let mut state = AppState::default();
        run_with_effects(&mut state, Command::CompileAndEncode);
        let effects = state.dispatch(Command::CopyShareLink);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::CopyToClipboard { .. })),
            "CopyShareLink should produce CopyToClipboard effect"
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SetUrlHash { .. })),
            "CopyShareLink should produce SetUrlHash effect"
        );
        assert!(state.status_message.contains("clipboard"));
    }

    #[test]
    fn test_permalink_round_trip_through_state() {
        let mut state = AppState::default();
        state.schema_text = "table T { x:int; } root_type T;".to_string();
        state.data_text = r#"{"x": 42}"#.to_string();
        state.data_format = DataFormat::Json;

        // Encode
        let effects = state.dispatch(Command::CopyShareLink);
        let encoded = effects
            .iter()
            .find_map(|e| match e {
                Effect::SetUrlHash { hash } => Some(hash.clone()),
                _ => None,
            })
            .expect("should have SetUrlHash effect");

        // Decode into fresh state
        let mut state2 = AppState::default();
        run_with_effects(&mut state2, Command::LoadFromPermalink(encoded));
        assert_eq!(state2.schema_text, "table T { x:int; } root_type T;");
        assert_eq!(state2.data_text, r#"{"x": 42}"#);
        assert_eq!(state2.data_format, DataFormat::Json);
        assert!(
            state2.compiled_schema.is_some(),
            "should auto-compile after loading permalink"
        );
        assert!(
            state2.annotations.is_some(),
            "should auto-walk after loading permalink"
        );
    }

    #[test]
    fn test_load_invalid_permalink() {
        let mut state = AppState::default();
        state.dispatch(Command::LoadFromPermalink("not-valid-data".to_string()));
        assert!(state.status_message.contains("Failed"));
    }
}
