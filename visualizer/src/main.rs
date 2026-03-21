mod app;
mod hex_view;
mod permalink;
mod state;
mod structure_view;
mod syntax;
mod templates;
mod view;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title(format!(
                "FlatBuffers Binary Visualizer v{}",
                env!("CARGO_PKG_VERSION")
            )),
        ..Default::default()
    };
    eframe::run_native(
        &format!(
            "FlatBuffers Binary Visualizer v{}",
            env!("CARGO_PKG_VERSION")
        ),
        options,
        Box::new(|cc| Ok(Box::new(app::VisualizerApp::new(cc)))),
    )
}

/// Read the `data` query parameter from the URL.
///
/// Supports `?data=...` for permalink payloads. Designed for extensibility --
/// future formats (e.g. protobuf) can use additional params like `?type=protobuf&data=...`.
#[cfg(target_arch = "wasm32")]
fn read_url_data_param() -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    // Parse query string manually (no URL API needed)
    let query = search.strip_prefix('?').unwrap_or(&search);
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("data=") {
            let decoded = js_sys::decode_uri_component(value)
                .ok()
                .and_then(|s| s.as_string());
            if let Some(v) = decoded {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        let web_options = eframe::WebOptions::default();
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("the_canvas_id"))
            .and_then(|e| {
                use wasm_bindgen::JsCast;
                e.dyn_into::<web_sys::HtmlCanvasElement>().ok()
            })
            .expect("failed to find canvas element 'the_canvas_id'");

        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                document.set_title(&format!(
                    "FlatBuffers Binary Visualizer v{}",
                    env!("CARGO_PKG_VERSION")
                ));
            }
        }

        let permalink_data = read_url_data_param();

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(move |cc| {
                    Ok(Box::new(app::VisualizerApp::new_with_permalink(
                        cc,
                        permalink_data,
                    )))
                }),
            )
            .await
            .expect("failed to start eframe");
    });
}
