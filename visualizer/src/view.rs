//! View layer: reads [`AppState`] and emits [`Command`] values.
//!
//! The rendering code is extracted from the old `app.rs` monolith. All egui
//! panel rendering lives here. State mutations go through commands; text edits
//! are the one pragmatic exception (direct `&mut` to avoid per-keystroke cloning).

use crate::hex_view;
use crate::state::{AppState, Command, DataFormat};
use crate::structure_view;
use crate::syntax;
use crate::templates;

// ---------------------------------------------------------------------------
// ViewOutput -- what the view produces each frame
// ---------------------------------------------------------------------------

/// Output of [`render_view`]. Contains commands for state dispatch and flags
/// for platform actions (file dialogs, uploads).
pub struct ViewOutput {
    pub commands: Vec<Command>,
    /// Request to open a schema file (native dialog or WASM upload).
    pub load_schema_file: bool,
    /// Request to open a binary file (native dialog or WASM upload).
    pub load_binary_file: bool,
}

// ---------------------------------------------------------------------------
// render_view -- the main rendering function
// ---------------------------------------------------------------------------

/// Render all UI panels. Reads state, emits commands.
///
/// Text edits (`schema_text`, `data_text`, `root_type_name`) are done
/// directly via `&mut` on `AppState` for efficiency -- these are trivial
/// mutations that don't need command overhead per keystroke. All other
/// interactions (buttons, combos, clicks) go through commands.
pub fn render_view(ctx: &egui::Context, state: &mut AppState) -> ViewOutput {
    let mut commands = Vec::new();
    let mut load_schema_file = false;
    let mut load_binary_file = false;

    // -- Top toolbar --
    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Template examples dropdown
            let all_templates = templates::all();
            let prev_idx = state.selected_template_idx;
            let desc = all_templates[state.selected_template_idx].description;
            let mut selected = state.selected_template_idx;
            egui::ComboBox::from_label("Examples")
                .width(130.0)
                .show_index(ui, &mut selected, all_templates.len(), |i| {
                    all_templates[i].name.to_string()
                })
                .on_hover_text(desc);
            if selected != prev_idx {
                commands.push(Command::SelectTemplate(selected));
            }

            ui.separator();

            // Native: file dialog buttons
            #[cfg(not(target_arch = "wasm32"))]
            {
                if ui.button("Load .fbs").clicked() {
                    load_schema_file = true;
                }
                if ui.button("Load .bin").clicked() {
                    load_binary_file = true;
                }
                ui.separator();
            }

            // Web: file upload buttons
            #[cfg(target_arch = "wasm32")]
            {
                if ui.button("Upload .bin").clicked() {
                    load_binary_file = true;
                }
                if ui.button("Upload .fbs").clicked() {
                    load_schema_file = true;
                }
                ui.separator();
            }

            {
                let btn = egui::Button::new(
                    egui::RichText::new("Compile & Encode")
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(egui::Color32::from_rgb(30, 100, 210));
                if ui
                    .add(btn)
                    .on_hover_text("Compile schema and encode data")
                    .clicked()
                {
                    commands.push(Command::CompileAndEncode);
                }
            }

            {
                let btn = egui::Button::new(
                    egui::RichText::new("Random")
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(egui::Color32::from_rgb(30, 160, 80));
                if ui
                    .add(btn)
                    .on_hover_text("Generate random schema and data")
                    .clicked()
                {
                    commands.push(Command::GenerateRandom {
                        seed: state.random_seed_counter,
                    });
                }
            }

            ui.separator();
            ui.label("Root type:");
            ui.text_edit_singleline(&mut state.root_type_name)
                .on_hover_text("Override root_type (leave empty to auto-detect)");

            ui.separator();
            let prev_format = state.data_format;
            let mut current_format = state.data_format;
            egui::ComboBox::from_label("Data format")
                .width(60.0)
                .selected_text(match current_format {
                    DataFormat::Json => "JSON",
                    DataFormat::Binary => "Hex",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut current_format, DataFormat::Json, "JSON");
                    ui.selectable_value(&mut current_format, DataFormat::Binary, "Hex");
                });
            if current_format != prev_format {
                commands.push(Command::SwitchDataFormat(current_format));
            }
        });
    });

    // -- Bottom status bar --
    egui::TopBottomPanel::bottom("status_bar")
        .min_height(24.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&state.status_message);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.hyperlink_to("GitHub", "https://github.com/Shuozeli/fbsviewer-lib");
                });
            });
        });

    // -- Left panel: editors --
    egui::SidePanel::left("editors")
        .resizable(true)
        .default_width(450.0)
        .min_width(200.0)
        .max_width(600.0)
        .show(ctx, |ui| {
            let available_height = ui.available_height();

            // Schema editor (top portion)
            ui.horizontal(|ui| {
                ui.heading("Schema (.fbs)");
                if state.compiled_schema.is_some() {
                    let btn_label = if state.show_schema_json {
                        "Hide JSON"
                    } else {
                        "View JSON"
                    };
                    if ui.button(btn_label).clicked() {
                        commands.push(Command::ToggleSchemaJson);
                    }
                }
            });
            if let Some(ref err) = state.compile_error {
                ui.colored_label(egui::Color32::RED, err);
            }

            let schema_height = if state.show_schema_json {
                available_height * 0.25
            } else {
                available_height * 0.45
            };
            egui::ScrollArea::vertical()
                .id_salt("schema_scroll")
                .max_height(schema_height)
                .show(ui, |ui| {
                    let font = egui::FontId::monospace(12.0);
                    ui.add(
                        egui::TextEdit::multiline(&mut state.schema_text)
                            .font(font.clone())
                            .desired_width(ui.available_width())
                            .desired_rows(15)
                            .layouter(&mut |ui, text, wrap_width| {
                                let job = syntax::highlight_fbs(text, &font, wrap_width);
                                ui.fonts(|f| f.layout_job(job))
                            }),
                    );
                });

            // Schema JSON view (collapsible)
            if state.show_schema_json && !state.schema_json_output.is_empty() {
                ui.separator();
                ui.label("Compiled Schema (JSON):");
                egui::ScrollArea::vertical()
                    .id_salt("schema_json_scroll")
                    .max_height(available_height * 0.20)
                    .show(ui, |ui| {
                        let mut json = state.schema_json_output.as_str();
                        let font = egui::FontId::monospace(11.0);
                        ui.add(
                            egui::TextEdit::multiline(&mut json)
                                .font(font.clone())
                                .desired_width(ui.available_width())
                                .layouter(&mut |ui, text, wrap_width| {
                                    let job = syntax::highlight_json(text, &font, wrap_width);
                                    ui.fonts(|f| f.layout_job(job))
                                }),
                        );
                    });
            }

            ui.separator();

            // Data editor (bottom half)
            let format_label = match state.data_format {
                DataFormat::Json => "Data (JSON)",
                DataFormat::Binary => "Data (Hex bytes)",
            };
            ui.horizontal(|ui| {
                ui.heading(format_label);
                if state.annotations.is_some() && !state.decoded_json.is_empty() {
                    let btn_label = if state.show_decoded_json {
                        "Hide Decoded"
                    } else {
                        "View Decoded"
                    };
                    if ui.button(btn_label).clicked() {
                        commands.push(Command::ToggleDecodedJson);
                    }
                }
            });
            if let Some(ref err) = state.encode_error {
                ui.colored_label(egui::Color32::RED, err);
            }
            egui::ScrollArea::vertical()
                .id_salt("data_scroll")
                .max_height(if state.show_decoded_json {
                    ui.available_height() * 0.4
                } else {
                    ui.available_height()
                })
                .show(ui, |ui| {
                    let font = egui::FontId::monospace(12.0);
                    let is_json = state.data_format == DataFormat::Json;
                    ui.add(
                        egui::TextEdit::multiline(&mut state.data_text)
                            .font(font.clone())
                            .desired_width(ui.available_width())
                            .desired_rows(10)
                            .layouter(&mut |ui, text, wrap_width| {
                                let job = if is_json {
                                    syntax::highlight_json(text, &font, wrap_width)
                                } else {
                                    let mut job = egui::text::LayoutJob::default();
                                    job.wrap.max_width = wrap_width;
                                    job.append(
                                        text,
                                        0.0,
                                        egui::TextFormat {
                                            font_id: font.clone(),
                                            ..Default::default()
                                        },
                                    );
                                    job
                                };
                                ui.fonts(|f| f.layout_job(job))
                            }),
                    );
                });

            // Decoded JSON view (collapsible)
            if state.show_decoded_json && !state.decoded_json.is_empty() {
                ui.separator();
                ui.label("Decoded Data (JSON):");
                egui::ScrollArea::vertical()
                    .id_salt("decoded_json_scroll")
                    .show(ui, |ui| {
                        let mut json = state.decoded_json.as_str();
                        let font = egui::FontId::monospace(11.0);
                        ui.add(
                            egui::TextEdit::multiline(&mut json)
                                .font(font.clone())
                                .desired_width(ui.available_width())
                                .layouter(&mut |ui, text, wrap_width| {
                                    let job = syntax::highlight_json(text, &font, wrap_width);
                                    ui.fonts(|f| f.layout_job(job))
                                }),
                        );
                    });
            }
        });

    // -- Central panel: hex view + structure tree --
    egui::CentralPanel::default().show(ctx, |ui| {
        let mut new_hovered_region = None;
        let mut new_clicked_region = None;

        let available_height = ui.available_height();

        // Top half: structure tree
        ui.allocate_ui(
            egui::Vec2::new(ui.available_width(), available_height * 0.45),
            |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Structure");
                    if state.annotations.is_some() {
                        if ui.small_button("Expand All").clicked() {
                            state.structure_tree_gen += 1;
                            state.structure_all_open = Some(true);
                        }
                        if ui.small_button("Collapse All").clicked() {
                            state.structure_tree_gen += 1;
                            state.structure_all_open = Some(false);
                        }
                    }
                    if state.locked_region.is_some() && ui.small_button("Unlock").clicked() {
                        commands.push(Command::UnlockRegion);
                    }
                });
                if let Some(ref annotations) = state.annotations {
                    let tree_output = structure_view::show(
                        ui,
                        annotations,
                        state.locked_region,
                        state.hovered_region,
                        state.structure_tree_gen,
                        state.structure_all_open,
                    );
                    if tree_output.hovered_region.is_some() {
                        new_hovered_region = tree_output.hovered_region;
                    }
                    if tree_output.clicked_region.is_some() {
                        new_clicked_region = tree_output.clicked_region;
                    }
                } else {
                    ui.label("No data loaded. Click 'Compile & Encode' to start.");
                }
            },
        );

        ui.separator();

        // Bottom half: hex view
        ui.heading("Hex View");
        if let Some(ref data) = state.binary_data {
            if let Some(ref annotations) = state.annotations {
                let hex_output = hex_view::show(
                    ui,
                    data,
                    annotations,
                    state.locked_region,
                    state.hovered_region,
                );
                if hex_output.hovered_region.is_some() {
                    new_hovered_region = hex_output.hovered_region;
                }
                if hex_output.clicked_region.is_some() {
                    new_clicked_region = hex_output.clicked_region;
                }
            } else {
                ui.label("Binary data loaded but no annotations.");
            }
        } else {
            ui.label("No binary data.");
        }

        // Emit commands for interaction state changes
        if let Some(clicked) = new_clicked_region {
            commands.push(Command::ClickRegion(clicked));
        }
        if new_hovered_region != state.hovered_region {
            commands.push(Command::HoverRegion(new_hovered_region));
        }
    });

    ViewOutput {
        commands,
        load_schema_file,
        load_binary_file,
    }
}
