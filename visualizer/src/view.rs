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

/// Width threshold below which the layout switches to vertically stacked panels.
const NARROW_BREAKPOINT: f32 = 700.0;

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

    let is_narrow = ctx.screen_rect().width() < NARROW_BREAKPOINT;

    // -- Top toolbar --
    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            render_toolbar(
                ui,
                state,
                &mut commands,
                &mut load_schema_file,
                &mut load_binary_file,
            );
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

    if is_narrow {
        render_narrow_layout(ctx, state, &mut commands);
    } else {
        render_wide_layout(ctx, state, &mut commands);
    }

    // -- Toast notification (auto-dismissing) --
    if let Some(msg) = &state.toast_message {
        let screen = ctx.screen_rect();
        let toast_width = 250.0;
        let toast_pos = egui::pos2(screen.center().x - toast_width / 2.0, screen.top() + 50.0);
        egui::Area::new(egui::Id::new("toast_notification"))
            .fixed_pos(toast_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(50, 160, 90))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(msg)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                    });
            });
        ctx.request_repaint();
    }
    state.tick_toast();

    ViewOutput {
        commands,
        load_schema_file,
        load_binary_file,
    }
}

// ---------------------------------------------------------------------------
// Layout variants
// ---------------------------------------------------------------------------

/// Narrow (< 700px): everything stacked in a single scrollable panel.
fn render_narrow_layout(ctx: &egui::Context, state: &mut AppState, commands: &mut Vec<Command>) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let mut hover = None;
        let mut click = None;

        egui::ScrollArea::vertical()
            .id_salt("narrow_scroll")
            .show(ui, |ui| {
                egui::CollapsingHeader::new(egui::RichText::new("Schema (.fbs)").heading())
                    .default_open(true)
                    .show(ui, |ui| {
                        render_schema_editor(ui, state, commands, None);
                    });

                ui.separator();

                let format_label = data_format_label(state.data_format);
                egui::CollapsingHeader::new(egui::RichText::new(format_label).heading())
                    .default_open(true)
                    .show(ui, |ui| {
                        render_data_editor(ui, state, commands, None);
                    });

                ui.separator();

                egui::CollapsingHeader::new(egui::RichText::new("Structure").heading())
                    .default_open(true)
                    .show(ui, |ui| {
                        render_structure_header(ui, state, commands);
                        egui::ScrollArea::vertical()
                            .id_salt("narrow_structure")
                            .max_height(300.0)
                            .show(ui, |ui| {
                                render_structure_tree(ui, state, &mut hover, &mut click);
                            });
                    });

                ui.separator();

                egui::CollapsingHeader::new(egui::RichText::new("Hex View").heading())
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("narrow_hex")
                            .max_height(400.0)
                            .show(ui, |ui| {
                                render_hex_view(ui, state, &mut hover, &mut click);
                            });
                    });
            });

        emit_interaction_commands(commands, state, hover, click);
    });
}

/// Wide (>= 700px): side-by-side left editor panel + central viewer panel.
fn render_wide_layout(ctx: &egui::Context, state: &mut AppState, commands: &mut Vec<Command>) {
    egui::SidePanel::left("editors")
        .resizable(true)
        .default_width(450.0)
        .min_width(200.0)
        .max_width(600.0)
        .show(ctx, |ui| {
            let available = ui.available_height();

            ui.horizontal(|ui| {
                ui.heading("Schema (.fbs)");
                if state.compiled_schema.is_some() {
                    let label = if state.show_schema_json {
                        "Hide JSON"
                    } else {
                        "View JSON"
                    };
                    if ui.button(label).clicked() {
                        commands.push(Command::ToggleSchemaJson);
                    }
                }
            });
            let schema_h = if state.show_schema_json {
                available * 0.25
            } else {
                available * 0.45
            };
            render_schema_editor(ui, state, commands, Some(schema_h));

            ui.separator();

            let format_label = data_format_label(state.data_format);
            ui.horizontal(|ui| {
                ui.heading(format_label);
                if state.annotations.is_some() && !state.decoded_json.is_empty() {
                    let label = if state.show_decoded_json {
                        "Hide Decoded"
                    } else {
                        "View Decoded"
                    };
                    if ui.button(label).clicked() {
                        commands.push(Command::ToggleDecodedJson);
                    }
                }
            });
            let data_h = if state.show_decoded_json {
                Some(ui.available_height() * 0.4)
            } else {
                None
            };
            render_data_editor(ui, state, commands, data_h);
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        let mut hover = None;
        let mut click = None;
        let available = ui.available_height();

        ui.allocate_ui(
            egui::Vec2::new(ui.available_width(), available * 0.45),
            |ui| {
                render_structure_header(ui, state, commands);
                render_structure_tree(ui, state, &mut hover, &mut click);
            },
        );

        ui.separator();
        ui.heading("Hex View");
        render_hex_view(ui, state, &mut hover, &mut click);

        emit_interaction_commands(commands, state, hover, click);
    });
}

// ---------------------------------------------------------------------------
// Shared rendering helpers
// ---------------------------------------------------------------------------

fn data_format_label(format: DataFormat) -> &'static str {
    match format {
        DataFormat::Json => "Data (JSON)",
        DataFormat::Binary => "Data (Hex bytes)",
    }
}

/// Toolbar controls: template picker, file buttons, compile, random, root type, format.
fn render_toolbar(
    ui: &mut egui::Ui,
    state: &mut AppState,
    commands: &mut Vec<Command>,
    load_schema_file: &mut bool,
    load_binary_file: &mut bool,
) {
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

    #[cfg(not(target_arch = "wasm32"))]
    {
        if ui.button("Load .fbs").clicked() {
            *load_schema_file = true;
        }
        if ui.button("Load .bin").clicked() {
            *load_binary_file = true;
        }
        ui.separator();
    }

    #[cfg(target_arch = "wasm32")]
    {
        if ui.button("Upload .bin").clicked() {
            *load_binary_file = true;
        }
        if ui.button("Upload .fbs").clicked() {
            *load_schema_file = true;
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
    let share_btn = egui::Button::new(egui::RichText::new("Share").color(egui::Color32::WHITE))
        .fill(egui::Color32::from_rgb(74, 130, 220));
    if ui
        .add(share_btn)
        .on_hover_text("Copy a permalink to clipboard (schema + data encoded in URL)")
        .clicked()
    {
        commands.push(Command::CopyShareLink);
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
}

/// Schema text editor with syntax highlighting and optional JSON view.
/// `max_h` caps the editor scroll height; `None` uses a default.
fn render_schema_editor(
    ui: &mut egui::Ui,
    state: &mut AppState,
    _commands: &mut Vec<Command>,
    max_h: Option<f32>,
) {
    if let Some(ref err) = state.compile_error {
        ui.colored_label(egui::Color32::RED, err);
    }

    let height = max_h.unwrap_or(250.0);
    egui::ScrollArea::vertical()
        .id_salt("schema_scroll")
        .max_height(height)
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

    if state.show_schema_json && !state.schema_json_output.is_empty() {
        ui.separator();
        ui.label("Compiled Schema (JSON):");
        egui::ScrollArea::vertical()
            .id_salt("schema_json_scroll")
            .max_height(200.0)
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
}

/// Data text editor (JSON or hex) with optional decoded JSON view.
/// `max_h` caps the editor scroll height; `None` uses all remaining space.
fn render_data_editor(
    ui: &mut egui::Ui,
    state: &mut AppState,
    _commands: &mut Vec<Command>,
    max_h: Option<f32>,
) {
    if let Some(ref err) = state.encode_error {
        ui.colored_label(egui::Color32::RED, err);
    }

    let height = max_h.unwrap_or(ui.available_height());
    egui::ScrollArea::vertical()
        .id_salt("data_scroll")
        .max_height(height)
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
}

/// Structure tree header: heading + expand/collapse/unlock buttons.
fn render_structure_header(ui: &mut egui::Ui, state: &mut AppState, commands: &mut Vec<Command>) {
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
}

/// Structure tree content.
fn render_structure_tree(
    ui: &mut egui::Ui,
    state: &AppState,
    hover: &mut Option<usize>,
    click: &mut Option<usize>,
) {
    if let Some(ref annotations) = state.annotations {
        let out = structure_view::show(
            ui,
            annotations,
            state.locked_region,
            state.hovered_region,
            state.structure_tree_gen,
            state.structure_all_open,
        );
        if out.hovered_region.is_some() {
            *hover = out.hovered_region;
        }
        if out.clicked_region.is_some() {
            *click = out.clicked_region;
        }
    } else {
        ui.label("No data loaded. Click 'Compile & Encode' to start.");
    }
}

/// Hex view content.
fn render_hex_view(
    ui: &mut egui::Ui,
    state: &AppState,
    hover: &mut Option<usize>,
    click: &mut Option<usize>,
) {
    if let Some(ref data) = state.binary_data {
        if let Some(ref annotations) = state.annotations {
            let out = hex_view::show(
                ui,
                data,
                annotations,
                state.locked_region,
                state.hovered_region,
            );
            if out.hovered_region.is_some() {
                *hover = out.hovered_region;
            }
            if out.clicked_region.is_some() {
                *click = out.clicked_region;
            }
        } else {
            ui.label("Binary data loaded but no annotations.");
        }
    } else {
        ui.label("No binary data.");
    }
}

/// Emit hover/click commands based on interaction state changes.
fn emit_interaction_commands(
    commands: &mut Vec<Command>,
    state: &AppState,
    hover: Option<usize>,
    click: Option<usize>,
) {
    if let Some(clicked) = click {
        commands.push(Command::ClickRegion(clicked));
    }
    if hover != state.hovered_region {
        commands.push(Command::HoverRegion(hover));
    }
}
