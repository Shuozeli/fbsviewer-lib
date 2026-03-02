mod app;
mod hex_view;
mod state;
mod structure_view;
mod syntax;
mod templates;
mod view;
#[cfg(target_arch = "wasm32")]
mod wasm_libc_shim;

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

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(app::VisualizerApp::new(cc)))),
            )
            .await
            .expect("failed to start eframe");
    });
}
