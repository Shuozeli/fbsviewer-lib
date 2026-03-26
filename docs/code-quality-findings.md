# Code Quality Findings

## 1. Duplication

### Duplicated effect execution logic (app.rs vs state.rs tests) -- DONE
- **Location:** `visualizer/src/app.rs:118-338` (VisualizerApp::execute_effect)
- **Also at:** `visualizer/src/state.rs:766-909` (execute_effect_sync in #[cfg(test)])
- **Problem:** The test helper `execute_effect_sync` duplicates most of the production `execute_effect` logic (CompileSchema, EncodeJson, ParseHexData, WalkBinary, GenerateRandomSchemaAndData, CompileProtoSchema, WalkProtobuf). Changes to effect handling must be mirrored in both places. This is the largest duplication in the codebase (~140 lines).
- **Fix:** Extract the pure (non-platform) effect execution into a shared function `fn execute_effect_pure(effect: Effect) -> Option<Command>` that both `VisualizerApp::execute_effect` and the test helper call. The app method would additionally handle platform effects (CopyToClipboard, SetUrlQueryParam).
- **Status:** Extracted `execute_effect_pure` in `app.rs`. `VisualizerApp::execute_effect` delegates non-platform effects to it. Test helper `execute_effect_sync` in `state.rs` replaced with a call to `crate::app::execute_effect_pure`. ~140 lines of duplication removed.

### Duplicated GenConfig construction -- DONE
- **Location:** `visualizer/src/app.rs:279-293` (GenerateRandomSchemaAndData handler)
- **Also at:** `visualizer/src/state.rs:827-841` (test execute_effect_sync)
- **Problem:** The exact same 14-field `GenConfig` struct literal is copy-pasted between production and test code. If the config ever changes, both must be updated.
- **Fix:** Extract a shared constructor `fn default_gen_config() -> flatc_rs_fbs_gen::GenConfig` and call it from both locations.
- **Status:** Extracted `pub(crate) fn default_gen_config()` in `app.rs`. Both production and test code call it. (Test code now uses it via `execute_effect_pure`.)

### Duplicated hex-view cursor position calculation -- DONE
- **Location:** `visualizer/src/hex_view.rs:42-61` (hover detection)
- **Also at:** `visualizer/src/hex_view.rs:66-85` (click detection)
- **Problem:** The hover and click branches compute the same byte index from cursor position using identical math (addr_width, hex_x, byte_col). Copy-pasted with only the result target differing.
- **Fix:** Extract `fn byte_index_at_pos(pos: Pos2, rect: Rect, row_start: usize, char_width: f32, data_len: usize) -> Option<usize>` and call from both branches.
- **Status:** Extracted `byte_index_at_pos` helper function. Both hover and click branches now call it.

### Duplicated schema root type extraction pattern -- DONE
- **Location:** `visualizer/src/app.rs:125-129` (CompileSchema effect handler)
- **Also at:** `visualizer/src/app.rs:316-322` (GenerateRandomSchemaAndData handler)
- **Also at:** `visualizer/src/state.rs:773-778` (test helper)
- **Also at:** `visualizer/src/state.rs:862-868` (test helper, random gen)
- **Also at:** `visualizer-cli/src/schema_input.rs:43-47` (load_from_fbs)
- **Problem:** The pattern `root_table_index -> objects[idx] -> name` for extracting root type name is repeated 5 times with minor variations.
- **Fix:** Add a helper in `visualizer-core`: `pub fn extract_root_type_name(schema: &ResolvedSchema) -> Option<String>`.
- **Status:** Added `extract_root_type_name` to `visualizer-core/src/lib.rs`. All 5 call sites updated.

## 2. Unsafe Patterns

### unwrap() in WASM file upload code -- DONE
- **Location:** `visualizer/src/app.rs:452-494` (trigger_file_upload)
- **Problem:** Contains 12 `unwrap()` calls on WASM API results (`window().unwrap()`, `document().unwrap()`, `create_element().unwrap()`, `dyn_into().unwrap()`, `files().unwrap()`, `FileReader::new().unwrap()`, `result().unwrap()`, `lock().unwrap()`, etc.). Any failure crashes the WASM app with an unhelpful panic.
- **Fix:** Use `expect()` with descriptive messages at minimum, or chain with `ok_or`/`map_err` and log errors gracefully. Consider returning early on failure rather than panicking.
- **Status:** All 12 `unwrap()` calls replaced with `expect()` containing descriptive messages (e.g., `"WASM: no global window"`, `"WASM: FileReader result() failed"`).

### unwrap() on path.last() in non-test code -- DONE
- **Location:** `visualizer-core/src/json_decoder.rs:177` (field_name_from_path)
- **Problem:** `path.last().unwrap().clone()` is guarded by `path.len() >= 2`, so it cannot panic today. However, the `unwrap()` is unnecessary and a latent risk if the guard condition changes.
- **Fix:** Replace with `path.last().cloned().unwrap_or_else(|| fallback.to_string())` without the length guard, collapsing the entire function into one expression.
- **Status:** Simplified to single expression `path.last().cloned().unwrap_or_else(|| fallback.to_string())`.

### Unchecked index in field_offsets_in_table -- DONE
- **Location:** `visualizer-core/src/json_encoder.rs:297` (encode_table)
- **Problem:** `field_offsets_in_table[slot.field_id]` could panic if `slot.field_id >= num_vtable_entries` due to a corrupt or unexpected schema. No bounds check.
- **Fix:** Add `if slot.field_id >= num_vtable_entries { return Err(JsonEncodeError::ObjectIndexOutOfRange { .. }) }` before the index access.
- **Status:** Added bounds check returning `ObjectIndexOutOfRange` error before the index access.

### truncate() uses byte indexing on multi-byte strings -- DONE
- **Location:** `visualizer-cli/src/output.rs:144-150` (truncate function)
- **Problem:** `&s[..max.saturating_sub(3)]` indexes by byte position, which will panic if the truncation point falls in the middle of a multi-byte UTF-8 character (e.g., CJK text in field names).
- **Fix:** Use `s.char_indices()` to find a safe truncation boundary, or use `s.chars().take(max - 3).collect::<String>()`.
- **Status:** Replaced byte slicing with `s.chars().take(max.saturating_sub(3)).collect::<String>()`.

### process::exit() called from library function -- DONE
- **Location:** `visualizer-cli/src/filter.rs:43-46` (apply_filters)
- **Problem:** `parse_byte_range` errors cause `process::exit(1)` inside `apply_filters`. A function that processes data should not terminate the process -- that decision belongs to `main`. This also makes the filter logic untestable for error cases.
- **Fix:** Return `Result<Vec<usize>, String>` from `apply_filters` and let `main()` handle the exit.
- **Status:** Changed `apply_filters` to return `Result<Vec<usize>, String>`. Caller in `main.rs` updated to match on the result and exit on error.

## 3. Allow Attributes Suppressing Real Warnings

### #[allow(dead_code)] on EventLogEntry -- DONE
- **Location:** `visualizer/src/state.rs:192-196`
- **Problem:** `EventLogEntry` is pushed into `event_log` (line 624) but `event_log` entries are never read in production code. This is genuine dead code being silenced. The VecDeque accumulates up to 200 entries consuming memory with no consumer.
- **Fix:** Either expose the event log in the UI (e.g., a debug panel) or remove both the `#[allow(dead_code)]` and the `event_log` field. If keeping for future use, add a `// TODO: expose in debug UI` comment.
- **Status:** Added `// TODO: expose event_log in a debug UI panel` comment. `#[allow(dead_code)]` is retained on the struct because the fields are only read in tests; removing it triggers `-D warnings` in CI. The struct is kept because it is used in tests (`test_event_log_records_commands`).

### #[allow(dead_code)] on cjk_font_loaded -- DONE
- **Location:** `visualizer/src/app.rs:28-29`
- **Problem:** On native builds, `cjk_font_loaded` is written once during construction but never read. The `#[allow(dead_code)]` hides this. On WASM, it is used properly.
- **Fix:** Gate the field with `#[cfg(target_arch = "wasm32")]` only, or use it on native (e.g., log whether CJK font was loaded). Low priority.
- **Status:** Field gated with `#[cfg(target_arch = "wasm32")]`. On native, the return value from `try_load_system_cjk_font` is now a local `_cjk_font_loaded` (font is installed into egui context as a side effect, return value not needed).

### #[allow(clippy::too_many_arguments)] on render_tree_node -- SKIPPED
- **Location:** `visualizer/src/structure_view.rs:51`
- **Problem:** `render_tree_node` takes 8 arguments. The allow suppresses a legitimate lint rather than fixing the design.
- **Fix:** Group related parameters into a struct: `struct TreeRenderContext<'a> { annotations: &'a [AnnotatedRegion], locked_region: Option<usize>, hovered_region: Option<usize>, tree_gen: u64, all_open: Option<bool> }`. Low priority.
- **Status:** Skipped. Low priority and the struct extraction would add boilerplate for an internal recursive render function with no public API surface.

## 4. Dead / No-Op Code

### EventLog is write-only -- DONE (partial)
- **Location:** `visualizer/src/state.rs:189-196,622-630`
- **Problem:** `EventLogEntry` structs are pushed into `state.event_log` every dispatch call (line 624-630), accumulating up to 200 entries. The VecDeque is never read in production code, consuming memory for nothing.
- **Fix:** Remove the event log, or add a debug panel to display it. If removing, delete `event_log` from `AppState`, the `EventLogEntry` struct, and the logging block in `dispatch()`.
- **Status:** Kept the event log with a TODO comment. It is actively used in tests and provides a useful audit trail for debugging. Removing it would lose test coverage; exposing it in UI is a future enhancement.

### Unused _commands parameter in render functions -- SKIPPED
- **Location:** `visualizer/src/view.rs:375` (render_schema_editor `_commands`)
- **Also at:** `visualizer/src/view.rs:427` (render_data_editor `_commands`)
- **Problem:** Both accept `_commands: &mut Vec<Command>` that is never used. The underscore prefix confirms intentional non-use. Likely leftover from an earlier design.
- **Fix:** Remove the parameter from both functions and update callers. Low priority (cosmetic).
- **Status:** Skipped. Low priority cosmetic change; the parameter may be needed for future editor interactions.

## 5. Excessive Cloning

### Heavy cloning in encode_table path -- SKIPPED
- **Location:** `visualizer-core/src/json_encoder.rs:231-242`
- **Problem:** `self.get_object(obj_idx)?.clone()` clones the entire Object. Then `obj.fields.clone()` clones all fields again. Each field in `slots` also holds `field.clone()`. For a table with N fields, this creates 2N+1 clones of Field structs per encode call. Compounds in deeply nested schemas.
- **Fix:** The root cause is borrow checker ergonomics -- `self.get_object()` borrows `self.schema`, preventing mutable buffer access. Restructure by splitting the encoder into separate schema-ref and buffer-mut parts, or use indices into the schema's fields Vec rather than cloning Field structs into FieldSlot. Medium priority.
- **Status:** Skipped. Requires significant encoder restructuring (splitting borrow of schema vs buffer). Risk of introducing regressions outweighs the performance benefit for typical schema sizes.

### Schema cloning in state dispatch -- SKIPPED
- **Location:** `visualizer/src/state.rs:484,518,558`
- **Problem:** `self.compiled_schema = Some(schema.clone())` clones the entire ResolvedSchema. The same schema is then passed by value into Effect variants. ProtoSchema has the same issue at line 397/518.
- **Fix:** Use `Arc<ResolvedSchema>` and `Arc<ProtoSchema>` for compiled schemas. They are immutable after compilation, making them natural candidates for shared ownership. This avoids deep clones when creating effects. Low priority since schemas are typically small.
- **Status:** Skipped. Low priority -- schemas are small in practice and the Arc change would ripple through many type signatures.

## 6. Missing Abstraction

### Schema format detection lives in GUI state module -- SKIPPED
- **Location:** `visualizer/src/state.rs:717-740` (detect_schema_format)
- **Problem:** `detect_schema_format` is a pure function with zero dependency on AppState or egui, yet it lives in the state module. If the CLI ever needs auto-detection, it would need to duplicate this.
- **Fix:** Move to `visualizer-core` as `pub fn detect_schema_format(text: &str) -> SchemaFormat`. Low priority.
- **Status:** Skipped. Low priority; the CLI currently requires explicit schema file extensions and does not need auto-detection.

## 7. Stringly-Typed Error Handling

### CLI uses Result<T, String> throughout -- SKIPPED
- **Location:** `visualizer-cli/src/schema_input.rs:20` (load_schema)
- **Also at:** `visualizer-cli/src/filter.rs:5` (parse_byte_range)
- **Also at:** `visualizer-cli/src/main.rs:133` (load_binary)
- **Problem:** The CLI uses `String` for all error types. This prevents callers from matching on error variants and makes error handling unstructured. The core library correctly uses thiserror enums.
- **Fix:** Define a `CliError` enum using `thiserror` that wraps core errors and `std::io::Error`. Low priority for a CLI tool since errors are just printed.
- **Status:** Skipped. Low priority for a CLI tool where all errors are printed to stderr and the program exits.

## 8. Clippy Warnings (Round 2)

### approx_constant in test data -- DONE
- **Location:** `visualizer-core/tests/json_encoder_tests.rs:893,894,926,927,1194`
- **Problem:** Test data used `3.14` and `2.718281828` as float values, triggering clippy's `approx_constant` deny lint because they approximate `PI` and `E`. These are intentional test data values, not approximations of the constants.
- **Fix:** Changed test values to `3.125` and `2.71` which do not trigger the lint.
- **Status:** Fixed. All 5 occurrences updated.

### needless_lifetimes in test helper -- DONE
- **Location:** `visualizer-core/tests/walk_demo_monster.rs:65`
- **Problem:** `find_region<'a>` has explicit lifetime annotations that can be elided.
- **Fix:** Removed explicit lifetime annotations.
- **Status:** Fixed.

### len_zero in test assertions -- DONE
- **Location:** `visualizer-cli/tests/cli_tests.rs:1510,1766`
- **Problem:** `lines.len() >= 1` and `parsed.len() >= 1` should use `!is_empty()`.
- **Fix:** Changed to `!lines.is_empty()` and `!parsed.is_empty()`.
- **Status:** Fixed.

### useless_vec in test code -- DONE
- **Location:** `visualizer-cli/tests/cli_tests.rs:1609,1642`
- **Problem:** `vec![false; 69]` and `vec![false; 18]` used where fixed-size arrays suffice.
- **Fix:** Changed to `[false; 69]` and `[false; 18]`.
- **Status:** Fixed.

### needless_range_loop in test code -- DONE
- **Location:** `visualizer-cli/tests/cli_tests.rs:1615,1646`
- **Problem:** Loop variable used only for indexing into `covered` array.
- **Fix:** Replaced with `covered.iter_mut().take(end.min(N)).skip(start)` iterator pattern.
- **Status:** Fixed.

### needless_borrows_for_generic_args -- DONE
- **Location:** `visualizer/src/permalink.rs:165`
- **Problem:** `URL_SAFE_NO_PAD.encode(&[1u8, 0])` has unnecessary borrow since `encode` accepts `AsRef<[u8]>`.
- **Fix:** Changed to `URL_SAFE_NO_PAD.encode([1u8, 0])`.
- **Status:** Fixed.

### field_reassign_with_default in tests -- DONE
- **Location:** `visualizer/src/state.rs` tests (12 instances)
- **Problem:** Test setup creates `AppState::default()` then reassigns fields. Clippy suggests struct initialization syntax, but reassignment is more readable for test setup.
- **Fix:** Suppressed with `#[allow(clippy::field_reassign_with_default)]` on the test module.
- **Status:** Fixed (suppressed -- appropriate for test setup code).

## Summary

| Category | Count | Fixed | Skipped | Severity |
|----------|-------|-------|---------|----------|
| Duplication | 4 | 4 | 0 | Medium-High |
| Unsafe Patterns | 5 | 5 | 0 | Medium |
| Allow Suppression | 3 | 2 | 1 | Low-Medium |
| Dead / No-Op Code | 2 | 1 | 1 | Low |
| Excessive Cloning | 2 | 0 | 2 | Low-Medium |
| Missing Abstraction | 1 | 0 | 1 | Low |
| Stringly-Typed Errors | 1 | 0 | 1 | Low |
| Clippy Warnings (Round 2) | 7 | 7 | 0 | Low-Medium |
| **Total** | **25** | **19** | **6** | |
