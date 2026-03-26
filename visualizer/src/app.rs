//! Thin egui shell: owns [`AppState`], dispatches commands, executes effects.
//!
//! This is the runtime layer. It bridges between:
//! - [`view::render_view`] (produces commands from UI interaction)
//! - [`state::AppState::dispatch`] (pure state transitions)
//! - Side effects (schema compilation, JSON encoding, binary walking)
//! - Platform-specific code (file dialogs, WASM uploads, CJK font loading)

use flatbuf_visualizer_core::{
    annotations_to_json, collect_proto_message_names, encode_json, extract_root_type_name,
    parse_hex_bytes, walk_binary, walk_protobuf,
};

use crate::state::{AppState, Command, Effect};
use crate::view;

// ---------------------------------------------------------------------------
// VisualizerApp -- the egui application shell
// ---------------------------------------------------------------------------

pub struct VisualizerApp {
    state: AppState,

    /// Recursion depth guard for dispatch -> execute_effect -> dispatch chains.
    dispatch_depth: usize,

    // CJK font loading (WASM only -- on native the font is loaded eagerly in the constructor)
    #[cfg(target_arch = "wasm32")]
    cjk_font_loaded: bool,
    #[cfg(target_arch = "wasm32")]
    pending_cjk_font: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>,

    // WASM file upload state
    #[cfg(target_arch = "wasm32")]
    pending_binary_upload: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>,
    #[cfg(target_arch = "wasm32")]
    pending_schema_upload: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>,
}

const MAX_DISPATCH_DEPTH: usize = 8;

/// Shared GenConfig used for random schema generation in both production and tests.
pub(crate) fn default_gen_config() -> flatc_rs_fbs_gen::GenConfig {
    flatc_rs_fbs_gen::GenConfig {
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
    }
}

impl VisualizerApp {
    pub fn new(#[allow(unused_variables)] cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_permalink(cc, None)
    }

    pub fn new_with_permalink(
        #[allow(unused_variables)] cc: &eframe::CreationContext<'_>,
        permalink_data: Option<String>,
    ) -> Self {
        // Try to load CJK font eagerly on native (result not stored -- font is
        // installed into the egui context as a side effect).
        #[cfg(not(target_arch = "wasm32"))]
        let _cjk_font_loaded = try_load_system_cjk_font(&cc.egui_ctx);

        #[cfg(target_arch = "wasm32")]
        let pending_cjk_font = {
            let font_data: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>> =
                std::sync::Arc::new(std::sync::Mutex::new(None));
            let sink = font_data.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match fetch_cjk_font_from_cdn().await {
                    Ok(bytes) => {
                        log::info!("CJK font loaded: {} bytes", bytes.len());
                        *sink.lock().unwrap() = Some(bytes);
                    }
                    Err(e) => {
                        log::warn!("Failed to load CJK font: {e}");
                    }
                }
            });
            font_data
        };

        let mut app = Self {
            state: AppState::default(),
            dispatch_depth: 0,
            #[cfg(target_arch = "wasm32")]
            cjk_font_loaded: false,
            #[cfg(target_arch = "wasm32")]
            pending_cjk_font,
            #[cfg(target_arch = "wasm32")]
            pending_binary_upload: std::sync::Arc::new(std::sync::Mutex::new(None)),
            #[cfg(target_arch = "wasm32")]
            pending_schema_upload: std::sync::Arc::new(std::sync::Mutex::new(None::<Vec<u8>>)),
        };

        // Bootstrap: load from permalink query param if present, otherwise compile demo data
        if let Some(data) = permalink_data.filter(|d| !d.is_empty()) {
            app.dispatch(Command::LoadFromPermalink(data));
        } else {
            app.dispatch(Command::CompileAndEncode);
        }

        app
    }

    // -----------------------------------------------------------------------
    // Command dispatch and effect execution
    // -----------------------------------------------------------------------

    /// Dispatch a command: update state, execute returned effects.
    fn dispatch(&mut self, cmd: Command) {
        if self.dispatch_depth >= MAX_DISPATCH_DEPTH {
            debug_log(&format!("DEPTH LIMIT REACHED, dropping: {cmd}"));
            return;
        }
        self.dispatch_depth += 1;
        let effects = self.state.dispatch(cmd);
        for effect in effects {
            self.execute_effect(effect);
        }
        self.dispatch_depth -= 1;
    }

    /// Execute a single effect and dispatch the result command.
    fn execute_effect(&mut self, effect: Effect) {
        match effect {
            // Platform effects are handled here (not in execute_effect_pure)
            Effect::CopyToClipboard { text } => {
                #[cfg(target_arch = "wasm32")]
                {
                    if let Some(window) = web_sys::window() {
                        let origin = window.location().origin().unwrap_or_default();
                        let pathname = window.location().pathname().unwrap_or_default();
                        // URI-encode the data param for safe embedding in URL
                        let encoded = js_sys::encode_uri_component(&text)
                            .as_string()
                            .unwrap_or(text);
                        let full_url = format!("{origin}{pathname}?data={encoded}");
                        let clipboard = window.navigator().clipboard();
                        let _ = clipboard.write_text(&full_url);
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    debug_log(&format!("Share link ({}B encoded): {text}", text.len()));
                }
            }

            Effect::SetUrlQueryParam { key, value } => {
                #[cfg(target_arch = "wasm32")]
                {
                    if let Some(window) = web_sys::window() {
                        let encoded = js_sys::encode_uri_component(&value)
                            .as_string()
                            .unwrap_or(value);
                        let pathname = window.location().pathname().unwrap_or_default();
                        let new_url = format!("{pathname}?{key}={encoded}");
                        let _ = window.history().ok().and_then(|h| {
                            h.replace_state_with_url(
                                &wasm_bindgen::JsValue::NULL,
                                "",
                                Some(&new_url),
                            )
                            .ok()
                        });
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let _ = (key, value);
                }
            }

            // All non-platform effects are handled by the shared pure function
            other => {
                if let Some(cmd) = execute_effect_pure(other) {
                    self.dispatch(cmd);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Platform bridge: poll async events
    // -----------------------------------------------------------------------

    fn poll_platform_events(&mut self, #[allow(unused_variables)] ctx: &egui::Context) {
        // WASM: check async CJK font load
        #[cfg(target_arch = "wasm32")]
        if !self.cjk_font_loaded {
            if let Some(font_bytes) = self
                .pending_cjk_font
                .try_lock()
                .ok()
                .and_then(|mut g| g.take())
            {
                install_cjk_font(ctx, font_bytes);
                self.cjk_font_loaded = true;
            }
        }

        // WASM: check async file uploads
        #[cfg(target_arch = "wasm32")]
        {
            let pending_binary = self
                .pending_binary_upload
                .try_lock()
                .ok()
                .and_then(|mut g| g.take());
            let pending_schema = self
                .pending_schema_upload
                .try_lock()
                .ok()
                .and_then(|mut g| g.take());

            if let Some(data) = pending_binary {
                self.dispatch(Command::SetBinaryData(data));
            }
            if let Some(bytes) = pending_schema {
                match String::from_utf8(bytes) {
                    Ok(text) => {
                        self.dispatch(Command::LoadSchemaText(text));
                    }
                    Err(e) => {
                        self.dispatch(Command::SchemaCompileError(format!(
                            "Invalid UTF-8 in schema file: {e}"
                        )));
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Platform-specific file loading
    // -----------------------------------------------------------------------

    #[cfg(not(target_arch = "wasm32"))]
    fn handle_load_schema_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("FlatBuffers Schema", &["fbs"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    self.dispatch(Command::LoadSchemaText(text));
                }
                Err(e) => {
                    self.dispatch(Command::SchemaCompileError(format!(
                        "failed to read file: {e}"
                    )));
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn handle_load_binary_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Binary", &["bin", "mon", "bfbs"])
            .pick_file()
        {
            match std::fs::read(&path) {
                Ok(data) => {
                    self.dispatch(Command::SetBinaryData(data));
                }
                Err(e) => {
                    self.dispatch(Command::EncodeError(format!("failed to read file: {e}")));
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn handle_load_schema_file(&self) {
        self.trigger_file_upload(".fbs", false);
    }

    #[cfg(target_arch = "wasm32")]
    fn handle_load_binary_file(&self) {
        self.trigger_file_upload(".bin,.mon,.bfbs", true);
    }

    // -----------------------------------------------------------------------
    // WASM file upload trigger
    // -----------------------------------------------------------------------

    #[cfg(target_arch = "wasm32")]
    fn trigger_file_upload(&self, accept: &str, is_binary: bool) {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;
        use web_sys::HtmlInputElement;

        let window = web_sys::window().expect("WASM: no global window");
        let document = window.document().expect("WASM: no document on window");

        let input: HtmlInputElement = document
            .create_element("input")
            .expect("WASM: failed to create <input> element")
            .dyn_into()
            .expect("WASM: created element is not an HtmlInputElement");
        input.set_type("file");
        input
            .set_attribute("accept", accept)
            .expect("WASM: failed to set accept attribute");

        let sink = if is_binary {
            self.pending_binary_upload.clone()
        } else {
            self.pending_schema_upload.clone()
        };

        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let target: HtmlInputElement = event
                .target()
                .expect("WASM: change event has no target")
                .dyn_into()
                .expect("WASM: event target is not an HtmlInputElement");
            let files = target.files().expect("WASM: input element has no files");
            if let Some(file) = files.get(0) {
                let reader =
                    web_sys::FileReader::new().expect("WASM: failed to create FileReader");
                let reader_clone = reader.clone();
                let sink = sink.clone();

                let onload = Closure::wrap(Box::new(move |_: web_sys::Event| {
                    let result = reader_clone
                        .result()
                        .expect("WASM: FileReader result() failed");
                    let array_buffer = result
                        .dyn_into::<js_sys::ArrayBuffer>()
                        .expect("WASM: FileReader result is not an ArrayBuffer");
                    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                    let mut data = vec![0u8; uint8_array.length() as usize];
                    uint8_array.copy_to(&mut data);
                    *sink.lock().expect("WASM: upload sink mutex poisoned") = Some(data);
                }) as Box<dyn FnMut(web_sys::Event)>);

                reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                onload.forget();
                reader
                    .read_as_array_buffer(&file)
                    .expect("WASM: read_as_array_buffer failed");
            }
        }) as Box<dyn FnMut(web_sys::Event)>);

        input
            .add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())
            .expect("WASM: failed to add change event listener");
        closure.forget();
        input.click();
    }
}

// ---------------------------------------------------------------------------
// Pure (non-platform) effect execution -- shared by production and tests
// ---------------------------------------------------------------------------

/// Execute a non-platform effect and return the resulting command, if any.
///
/// Platform effects ([`Effect::CopyToClipboard`], [`Effect::SetUrlQueryParam`])
/// must be handled separately by the caller.
pub(crate) fn execute_effect_pure(effect: Effect) -> Option<Command> {
    match effect {
        Effect::CompileSchema { source } => {
            let result =
                std::panic::catch_unwind(|| flatc_rs_compiler::compile_single(&source));
            match result {
                Ok(Ok(result)) => {
                    let root_name = extract_root_type_name(&result.schema);
                    let legacy = result.schema.as_legacy();
                    let schema_json =
                        serde_json::to_string_pretty(&legacy).unwrap_or_default();
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

        Effect::CompileProtoSchema { source } => match protoc_rs_analyzer::analyze(&source) {
            Ok(fds) => {
                let msg_names = collect_proto_message_names(&fds);
                let schema_json =
                    format!("{} messages in {} files", msg_names.len(), fds.file.len());
                Some(Command::ProtoSchemaCompiled {
                    schema: Box::new(fds),
                    schema_json,
                    root_message_names: msg_names,
                })
            }
            Err(e) => Some(Command::SchemaCompileError(e.to_string())),
        },

        Effect::WalkProtobuf {
            binary,
            schema,
            root_message,
        } => match walk_protobuf(&binary, &schema, &root_message) {
            Ok(annotations) => {
                let decoded_json = String::new(); // No JSON decode for protobuf yet
                Some(Command::BinaryWalked {
                    annotations,
                    decoded_json,
                })
            }
            Err(e) => Some(Command::WalkError(e.to_string())),
        },

        Effect::GenerateRandomSchemaAndData { seed } => {
            let gen_config = default_gen_config();
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

            let root_type =
                extract_root_type_name(&compile_result.schema).unwrap_or_default();

            let legacy = compile_result.schema.as_legacy();
            let data_config = flatc_rs_data_gen::DataGenConfig::default();
            match flatc_rs_data_gen::generate_json(&legacy, &root_type, seed, data_config) {
                Ok(json_text) => Some(Command::RandomGenerated {
                    schema_text: fbs_text,
                    data_text: json_text,
                }),
                Err(e) => Some(Command::RandomGenerateError(e.to_string())),
            }
        }

        // Platform effects return None -- they are handled by the app runtime
        Effect::CopyToClipboard { .. } | Effect::SetUrlQueryParam { .. } => None,
    }
}

// ---------------------------------------------------------------------------
// eframe::App implementation
// ---------------------------------------------------------------------------

impl eframe::App for VisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Poll platform async events (WASM uploads, CJK font)
        self.poll_platform_events(ctx);

        // 2. Render view, collecting commands
        let output = view::render_view(ctx, &mut self.state);

        // 3. Handle platform actions (file dialogs / uploads)
        if output.load_schema_file {
            self.handle_load_schema_file();
        }
        if output.load_binary_file {
            self.handle_load_binary_file();
        }

        // 4. Dispatch all state commands
        for cmd in output.commands {
            self.dispatch(cmd);
        }
    }
}

// ---------------------------------------------------------------------------
// Debug logging (works on both native and WASM)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
fn debug_log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}

#[cfg(not(target_arch = "wasm32"))]
fn debug_log(msg: &str) {
    eprintln!("{msg}");
}

// ---------------------------------------------------------------------------
// CJK font support
// ---------------------------------------------------------------------------

fn install_cjk_font(ctx: &egui::Context, font_bytes: Vec<u8>) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "cjk".to_owned(),
        egui::FontData::from_owned(font_bytes).into(),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.push("cjk".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.push("cjk".to_owned());
    }
    ctx.set_fonts(fonts);
}

#[cfg(not(target_arch = "wasm32"))]
fn try_load_system_cjk_font(ctx: &egui::Context) -> bool {
    let candidates = [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/OTF/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];

    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path) {
            install_cjk_font(ctx, bytes);
            return true;
        }
    }
    false
}

#[cfg(target_arch = "wasm32")]
async fn fetch_cjk_font_from_cdn() -> Result<Vec<u8>, String> {
    use wasm_bindgen::JsCast;

    let url = "https://fonts.gstatic.com/s/notosanssc/v40/k3kCo84MPvpLmixcA63oeAL7Iqp5IZJF9bmaG9_FnYw.ttf";

    let window = web_sys::window().ok_or("no window")?;
    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|e| format!("fetch failed: {e:?}"))?;
    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "response cast failed".to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let array_buffer = wasm_bindgen_futures::JsFuture::from(
        resp.array_buffer()
            .map_err(|_| "array_buffer() failed".to_string())?,
    )
    .await
    .map_err(|e| format!("array_buffer await failed: {e:?}"))?;
    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    let mut bytes = vec![0u8; uint8_array.length() as usize];
    uint8_array.copy_to(&mut bytes);
    Ok(bytes)
}
