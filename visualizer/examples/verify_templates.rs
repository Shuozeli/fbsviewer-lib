// Verify that each template's hex data is valid FlatBuffers for its schema.
// Run: cargo run --example verify_templates

use flatbuf_visualizer_core::{annotations_to_json, parse_hex_bytes, walk_binary};

fn main() {
    let templates = [
        (
            "Simple Scalars",
            "table Config {\n  debug: bool;\n  volume: int;\n  brightness: float;\n}\nroot_type Config;\n",
            "10 00 00 00 0a 00 10 00 0c 00 04 00 08 00 00 00 0c 00 00 00 4b 00 00 00 cd cc 4c 3f 01 00 00 00",
        ),
        (
            "Nested Structs",
            "struct Vec2 {\n  x: float;\n  y: float;\n}\nstruct Rect {\n  origin: Vec2;\n  size: Vec2;\n}\ntable UIElement {\n  name: string;\n  bounds: Rect;\n  opacity: float;\n}\nroot_type UIElement;\n",
            "10 00 00 00 0a 00 1c 00 14 00 04 00 18 00 00 00 0c 00 00 00 00 00 20 41 00 00 a0 41 00 00 48 43 00 00 48 42 08 00 00 00 66 66 66 3f 06 00 00 00 42 75 74 74 6f 6e 00 00",
        ),
        (
            "String Fields",
            "table UserProfile {\n  username: string;\n  email: string;\n  bio: string;\n  age: int;\n}\nroot_type UserProfile;\n",
            "10 00 00 00 0c 00 14 00 04 00 08 00 0c 00 10 00 0c 00 00 00 10 00 00 00 18 00 00 00 2c 00 00 00 1e 00 00 00 05 00 00 00 61 6c 69 63 65 00 00 00 11 00 00 00 61 6c 69 63 65 40 65 78 61 6d 70 6c 65 2e 63 6f 6d 00 00 00 0d 00 00 00 48 65 6c 6c 6f 2c 20 77 6f 72 6c 64 21 00 00 00",
        ),
    ];

    let mut all_ok = true;

    for (name, schema_src, hex) in &templates {
        print!("{name}: ");
        let result = flatc_rs_compiler::compile_single(schema_src);
        match result {
            Ok(r) => {
                let schema = r.schema;
                let root_name = schema
                    .root_table
                    .as_ref()
                    .and_then(|t| t.name.as_deref())
                    .unwrap_or("");
                let bytes = parse_hex_bytes(hex).unwrap();
                match walk_binary(&bytes, &schema, root_name) {
                    Ok(annotations) => {
                        let json = annotations_to_json(&annotations);
                        println!("OK ({} bytes, {} regions)", bytes.len(), annotations.len());
                        println!(
                            "  {}",
                            serde_json::to_string_pretty(&json)
                                .unwrap()
                                .replace('\n', "\n  ")
                        );
                    }
                    Err(e) => {
                        println!("WALK ERROR: {e}");
                        all_ok = false;
                    }
                }
            }
            Err(e) => {
                println!("COMPILE ERROR: {e}");
                all_ok = false;
            }
        }
    }

    if all_ok {
        println!("\nAll templates verified successfully.");
    } else {
        println!("\nSome templates FAILED.");
        std::process::exit(1);
    }
}
