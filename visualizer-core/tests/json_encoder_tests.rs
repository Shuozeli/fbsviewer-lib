use flatbuf_visualizer_core::{
    annotations_to_json, encode_json, load_schema_from_json, walk_binary, RegionType,
};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Schema fixtures
// ---------------------------------------------------------------------------

const MONSTER_SCHEMA_JSON: &str = r#"{
  "objects": [
    {
      "name": "Vec3",
      "fields": [
        { "name": "x", "type": { "base_type": "BASE_TYPE_FLOAT", "base_size": 4 }, "offset": 0 },
        { "name": "y", "type": { "base_type": "BASE_TYPE_FLOAT", "base_size": 4 }, "offset": 4 },
        { "name": "z", "type": { "base_type": "BASE_TYPE_FLOAT", "base_size": 4 }, "offset": 8 }
      ],
      "is_struct": true,
      "minalign": 4,
      "bytesize": 12
    },
    {
      "name": "Monster",
      "fields": [
        { "name": "pos", "type": { "base_type": "BASE_TYPE_STRUCT", "base_size": 4, "index": 0 }, "id": 0 },
        { "name": "mana", "type": { "base_type": "BASE_TYPE_SHORT", "base_size": 2 }, "id": 1, "default_integer": 150 },
        { "name": "hp", "type": { "base_type": "BASE_TYPE_SHORT", "base_size": 2 }, "id": 2, "default_integer": 100 },
        { "name": "name", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 3 },
        { "name": "color", "type": { "base_type": "BASE_TYPE_BYTE", "base_size": 1, "index": 0 }, "id": 4, "default_string": "Blue" },
        { "name": "inventory", "type": { "base_type": "BASE_TYPE_VECTOR", "base_size": 4, "element_size": 1, "element": "BASE_TYPE_U_BYTE" }, "id": 5 }
      ],
      "is_struct": false
    }
  ],
  "enums": [
    {
      "name": "Color",
      "values": [
        { "name": "Red", "value": 1 },
        { "name": "Green", "value": 2 },
        { "name": "Blue", "value": 3 }
      ],
      "underlying_type": { "base_type": "BASE_TYPE_BYTE" }
    }
  ],
  "root_table": {
    "name": "Monster",
    "fields": [
      { "name": "pos", "type": { "base_type": "BASE_TYPE_STRUCT", "base_size": 4, "index": 0 }, "id": 0 },
      { "name": "mana", "type": { "base_type": "BASE_TYPE_SHORT", "base_size": 2 }, "id": 1, "default_integer": 150 },
      { "name": "hp", "type": { "base_type": "BASE_TYPE_SHORT", "base_size": 2 }, "id": 2, "default_integer": 100 },
      { "name": "name", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 3 },
      { "name": "color", "type": { "base_type": "BASE_TYPE_BYTE", "base_size": 1, "index": 0 }, "id": 4, "default_string": "Blue" },
      { "name": "inventory", "type": { "base_type": "BASE_TYPE_VECTOR", "base_size": 4, "element_size": 1, "element": "BASE_TYPE_U_BYTE" }, "id": 5 }
    ],
    "is_struct": false
  }
}"#;

const SIMPLE_SCALARS_SCHEMA_JSON: &str = r#"{
  "objects": [
    {
      "name": "Config",
      "fields": [
        { "name": "debug", "type": { "base_type": "BASE_TYPE_BOOL", "base_size": 1 }, "id": 0 },
        { "name": "volume", "type": { "base_type": "BASE_TYPE_INT", "base_size": 4 }, "id": 1 },
        { "name": "brightness", "type": { "base_type": "BASE_TYPE_FLOAT", "base_size": 4 }, "id": 2 }
      ],
      "is_struct": false
    }
  ],
  "enums": [],
  "root_table": {
    "name": "Config",
    "fields": [
      { "name": "debug", "type": { "base_type": "BASE_TYPE_BOOL", "base_size": 1 }, "id": 0 },
      { "name": "volume", "type": { "base_type": "BASE_TYPE_INT", "base_size": 4 }, "id": 1 },
      { "name": "brightness", "type": { "base_type": "BASE_TYPE_FLOAT", "base_size": 4 }, "id": 2 }
    ],
    "is_struct": false
  }
}"#;

const STRING_FIELDS_SCHEMA_JSON: &str = r#"{
  "objects": [
    {
      "name": "UserProfile",
      "fields": [
        { "name": "username", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 0 },
        { "name": "email", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 1 },
        { "name": "bio", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 2 },
        { "name": "age", "type": { "base_type": "BASE_TYPE_INT", "base_size": 4 }, "id": 3 }
      ],
      "is_struct": false
    }
  ],
  "enums": [],
  "root_table": {
    "name": "UserProfile",
    "fields": [
      { "name": "username", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 0 },
      { "name": "email", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 1 },
      { "name": "bio", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 2 },
      { "name": "age", "type": { "base_type": "BASE_TYPE_INT", "base_size": 4 }, "id": 3 }
    ],
    "is_struct": false
  }
}"#;

const NESTED_TABLES_SCHEMA_JSON: &str = r#"{
  "objects": [
    {
      "name": "Address",
      "fields": [
        { "name": "street", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 0 },
        { "name": "city", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 1 },
        { "name": "zip", "type": { "base_type": "BASE_TYPE_INT", "base_size": 4 }, "id": 2 }
      ],
      "is_struct": false
    },
    {
      "name": "ContactInfo",
      "fields": [
        { "name": "email", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 0 },
        { "name": "phone", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 1 },
        { "name": "address", "type": { "base_type": "BASE_TYPE_TABLE", "base_size": 4, "index": 0 }, "id": 2 }
      ],
      "is_struct": false
    },
    {
      "name": "Employee",
      "fields": [
        { "name": "name", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 0 },
        { "name": "age", "type": { "base_type": "BASE_TYPE_INT", "base_size": 4 }, "id": 1 },
        { "name": "contact", "type": { "base_type": "BASE_TYPE_TABLE", "base_size": 4, "index": 1 }, "id": 2 }
      ],
      "is_struct": false
    }
  ],
  "enums": [],
  "root_table": {
    "name": "Employee",
    "fields": [
      { "name": "name", "type": { "base_type": "BASE_TYPE_STRING", "base_size": 4 }, "id": 0 },
      { "name": "age", "type": { "base_type": "BASE_TYPE_INT", "base_size": 4 }, "id": 1 },
      { "name": "contact", "type": { "base_type": "BASE_TYPE_TABLE", "base_size": 4, "index": 1 }, "id": 2 }
    ],
    "is_struct": false
  }
}"#;

// ---------------------------------------------------------------------------
// Helper: encode JSON, walk binary, decode back to JSON
// ---------------------------------------------------------------------------

fn roundtrip(schema_json: &str, root_type: &str, input: &Value) -> Value {
    let result = load_schema_from_json(schema_json).unwrap();
    let binary = encode_json(input, &result.schema, root_type).unwrap();

    // Verify the walker can parse it
    let annotations = walk_binary(&binary, &result.schema, root_type).unwrap();
    annotations_to_json(&annotations)
}

fn roundtrip_binary(schema_json: &str, root_type: &str, input: &Value) -> Vec<u8> {
    let result = load_schema_from_json(schema_json).unwrap();
    encode_json(input, &result.schema, root_type).unwrap()
}

/// Compile .fbs source, encode JSON, walk binary, decode back to JSON.
fn roundtrip_from_fbs(fbs_source: &str, input: &Value) -> Value {
    let result = flatc_rs_compiler::compile_single(fbs_source).unwrap();
    let root_name = result
        .schema
        .root_table
        .as_ref()
        .and_then(|t| t.name.as_deref())
        .unwrap();
    let binary = encode_json(input, &result.schema, root_name).unwrap();
    let annotations = walk_binary(&binary, &result.schema, root_name).unwrap();
    annotations_to_json(&annotations)
}

/// Compile .fbs source, encode JSON, return (binary, annotations).
fn encode_and_walk_from_fbs(
    fbs_source: &str,
    input: &Value,
) -> (Vec<u8>, Vec<flatbuf_visualizer_core::AnnotatedRegion>) {
    let result = flatc_rs_compiler::compile_single(fbs_source).unwrap();
    let root_name = result
        .schema
        .root_table
        .as_ref()
        .and_then(|t| t.name.as_deref())
        .unwrap();
    let binary = encode_json(input, &result.schema, root_name).unwrap();
    let annotations = walk_binary(&binary, &result.schema, root_name).unwrap();
    (binary, annotations)
}

// ---------------------------------------------------------------------------
// Round-trip tests
// ---------------------------------------------------------------------------

#[test]
fn test_roundtrip_simple_scalars() {
    let input = json!({
        "debug": true,
        "volume": 75,
        "brightness": 0.8
    });

    let output = roundtrip(SIMPLE_SCALARS_SCHEMA_JSON, "Config", &input);

    assert_eq!(output["debug"], json!(true));
    assert_eq!(output["volume"], json!(75));
    // Float comparison: 0.8 as f32 -> 0.800000011920929
    let brightness = output["brightness"].as_f64().unwrap();
    assert!((brightness - 0.8).abs() < 0.001, "brightness={brightness}");
}

#[test]
fn test_roundtrip_string_fields() {
    let input = json!({
        "username": "alice",
        "email": "alice@example.com",
        "bio": "Hello world",
        "age": 30
    });

    let output = roundtrip(STRING_FIELDS_SCHEMA_JSON, "UserProfile", &input);

    assert_eq!(output["username"], json!("alice"));
    assert_eq!(output["email"], json!("alice@example.com"));
    assert_eq!(output["bio"], json!("Hello world"));
    assert_eq!(output["age"], json!(30));
}

#[test]
fn test_roundtrip_monster() {
    let input = json!({
        "pos": { "x": 1.0, "y": 2.0, "z": 3.0 },
        "mana": 200,
        "hp": 300,
        "name": "Orc",
        "color": 1,
        "inventory": [0, 1, 2, 3, 4]
    });

    let output = roundtrip(MONSTER_SCHEMA_JSON, "Monster", &input);

    // Check struct fields
    let pos_x = output["pos"]["x"].as_f64().unwrap();
    assert!((pos_x - 1.0).abs() < 0.001, "pos.x={pos_x}");
    let pos_y = output["pos"]["y"].as_f64().unwrap();
    assert!((pos_y - 2.0).abs() < 0.001, "pos.y={pos_y}");
    let pos_z = output["pos"]["z"].as_f64().unwrap();
    assert!((pos_z - 3.0).abs() < 0.001, "pos.z={pos_z}");

    assert_eq!(output["mana"], json!(200));
    assert_eq!(output["hp"], json!(300));
    assert_eq!(output["name"], json!("Orc"));

    // Check vector
    let inv = output["inventory"].as_array().unwrap();
    assert_eq!(inv.len(), 5);
    assert_eq!(inv[0], json!(0));
    assert_eq!(inv[4], json!(4));
}

#[test]
fn test_roundtrip_monster_enum_by_name() {
    let input = json!({
        "pos": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "name": "Elf",
        "color": "Green"
    });

    let output = roundtrip(MONSTER_SCHEMA_JSON, "Monster", &input);
    assert_eq!(output["name"], json!("Elf"));
    // Color "Green" = 2
    assert_eq!(output["color"], json!(2));
}

#[test]
fn test_roundtrip_empty_table() {
    // All fields use defaults when omitted
    let input = json!({});

    let result = load_schema_from_json(SIMPLE_SCALARS_SCHEMA_JSON).unwrap();
    let binary = encode_json(&input, &result.schema, "Config").unwrap();

    // Should produce a valid binary (walker should not fail)
    let annotations = walk_binary(&binary, &result.schema, "Config").unwrap();
    // With no fields present, the decoded JSON should be empty or have defaults
    assert!(!annotations.is_empty());
}

#[test]
fn test_roundtrip_empty_string() {
    let input = json!({
        "username": "",
        "email": "test@test.com",
        "bio": "",
        "age": 0
    });

    let output = roundtrip(STRING_FIELDS_SCHEMA_JSON, "UserProfile", &input);
    // The JSON decoder returns null for empty strings (walker shows length=0)
    assert!(
        output["username"] == json!("") || output["username"].is_null(),
        "username={:?}",
        output["username"]
    );
    assert_eq!(output["email"], json!("test@test.com"));
    assert!(
        output["bio"] == json!("") || output["bio"].is_null(),
        "bio={:?}",
        output["bio"]
    );
}

#[test]
fn test_roundtrip_empty_vector() {
    let input = json!({
        "pos": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "name": "Empty",
        "inventory": []
    });

    let output = roundtrip(MONSTER_SCHEMA_JSON, "Monster", &input);
    assert_eq!(output["name"], json!("Empty"));
    let inv = output["inventory"].as_array().unwrap();
    assert_eq!(inv.len(), 0);
}

// ---------------------------------------------------------------------------
// Error tests
// ---------------------------------------------------------------------------

#[test]
fn test_error_root_type_not_found() {
    let result = load_schema_from_json(SIMPLE_SCALARS_SCHEMA_JSON).unwrap();
    let err = encode_json(&json!({}), &result.schema, "NonExistent").unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn test_error_expected_object() {
    let result = load_schema_from_json(SIMPLE_SCALARS_SCHEMA_JSON).unwrap();
    let err = encode_json(&json!([1, 2, 3]), &result.schema, "Config").unwrap_err();
    assert!(err.to_string().contains("expected JSON object"));
}

#[test]
fn test_error_type_mismatch() {
    let result = load_schema_from_json(SIMPLE_SCALARS_SCHEMA_JSON).unwrap();
    let err =
        encode_json(&json!({"volume": "not_a_number"}), &result.schema, "Config").unwrap_err();
    assert!(
        err.to_string().contains("expected") || err.to_string().contains("number"),
        "error: {err}"
    );
}

// ---------------------------------------------------------------------------
// Binary validity: ensure the walker accepts all encoded output
// ---------------------------------------------------------------------------

#[test]
fn test_walker_accepts_encoded_binary() {
    let result = load_schema_from_json(MONSTER_SCHEMA_JSON).unwrap();

    let input = json!({
        "pos": { "x": 1.5, "y": 2.5, "z": 3.5 },
        "mana": 100,
        "hp": 200,
        "name": "Dragon",
        "color": "Red",
        "inventory": [10, 20, 30]
    });

    let binary = encode_json(&input, &result.schema, "Monster").unwrap();

    // The walker must not fail -- this validates the binary format is correct
    let annotations = walk_binary(&binary, &result.schema, "Monster").unwrap();
    assert!(!annotations.is_empty(), "walker produced no annotations");

    // Decode to JSON and spot check
    let decoded = annotations_to_json(&annotations);
    assert_eq!(decoded["name"], json!("Dragon"));
    assert_eq!(decoded["hp"], json!(200));
}

// ---------------------------------------------------------------------------
// Deeply nested tables tests
// ---------------------------------------------------------------------------

#[test]
fn test_roundtrip_nested_tables_full() {
    let input = json!({
        "name": "Alice",
        "age": 30,
        "contact": {
            "email": "alice@example.com",
            "phone": "555-1234",
            "address": {
                "street": "123 Main St",
                "city": "Springfield",
                "zip": 62701
            }
        }
    });

    let output = roundtrip(NESTED_TABLES_SCHEMA_JSON, "Employee", &input);

    // Level 1: Employee
    assert_eq!(output["name"], json!("Alice"));
    assert_eq!(output["age"], json!(30));

    // Level 2: ContactInfo
    assert_eq!(output["contact"]["email"], json!("alice@example.com"));
    assert_eq!(output["contact"]["phone"], json!("555-1234"));

    // Level 3: Address
    assert_eq!(output["contact"]["address"]["street"], json!("123 Main St"));
    assert_eq!(output["contact"]["address"]["city"], json!("Springfield"));
    assert_eq!(output["contact"]["address"]["zip"], json!(62701));
}

#[test]
fn test_roundtrip_nested_tables_partial() {
    // Only populate first two levels, omit deepest table
    let input = json!({
        "name": "Bob",
        "age": 25,
        "contact": {
            "email": "bob@test.com",
            "phone": "555-9999"
        }
    });

    let output = roundtrip(NESTED_TABLES_SCHEMA_JSON, "Employee", &input);

    assert_eq!(output["name"], json!("Bob"));
    assert_eq!(output["age"], json!(25));
    assert_eq!(output["contact"]["email"], json!("bob@test.com"));
    assert_eq!(output["contact"]["phone"], json!("555-9999"));
}

#[test]
fn test_roundtrip_nested_tables_minimal() {
    // Only root level, nested tables omitted entirely
    let input = json!({
        "name": "Charlie",
        "age": 40
    });

    let output = roundtrip(NESTED_TABLES_SCHEMA_JSON, "Employee", &input);

    assert_eq!(output["name"], json!("Charlie"));
    assert_eq!(output["age"], json!(40));
}

#[test]
fn test_roundtrip_nested_tables_empty_strings() {
    let input = json!({
        "name": "",
        "age": 0,
        "contact": {
            "email": "",
            "phone": "",
            "address": {
                "street": "",
                "city": "",
                "zip": 0
            }
        }
    });

    let result = load_schema_from_json(NESTED_TABLES_SCHEMA_JSON).unwrap();
    let binary = encode_json(&input, &result.schema, "Employee").unwrap();
    // Walker must not fail on empty strings in nested tables
    let annotations = walk_binary(&binary, &result.schema, "Employee").unwrap();
    assert!(!annotations.is_empty());
}

#[test]
fn test_nested_tables_walker_region_depth() {
    use flatbuf_visualizer_core::RegionType;

    let input = json!({
        "name": "Alice",
        "age": 30,
        "contact": {
            "email": "alice@example.com",
            "phone": "555-1234",
            "address": {
                "street": "123 Main St",
                "city": "Springfield",
                "zip": 62701
            }
        }
    });

    let result = load_schema_from_json(NESTED_TABLES_SCHEMA_JSON).unwrap();
    let binary = encode_json(&input, &result.schema, "Employee").unwrap();
    let annotations = walk_binary(&binary, &result.schema, "Employee").unwrap();

    // Should have TableSOffset regions for all three table types
    let table_soffsets: Vec<_> = annotations
        .iter()
        .filter(|r| matches!(&r.region_type, RegionType::TableSOffset { .. }))
        .collect();
    assert!(
        table_soffsets.len() >= 3,
        "expected at least 3 TableSOffset regions (Employee, ContactInfo, Address), got {}",
        table_soffsets.len()
    );

    // Should have VTable regions for all three tables
    let vtables: Vec<_> = annotations
        .iter()
        .filter(|r| matches!(&r.region_type, RegionType::VTable { .. }))
        .collect();
    assert!(
        vtables.len() >= 3,
        "expected at least 3 VTable regions, got {}",
        vtables.len()
    );

    // Verify depth ordering: deeper tables have higher depth values
    let employee_soffset = annotations
        .iter()
        .find(|r| matches!(&r.region_type, RegionType::TableSOffset { type_name } if type_name == "Employee"))
        .expect("should have Employee TableSOffset");
    let contact_soffset = annotations
        .iter()
        .find(|r| matches!(&r.region_type, RegionType::TableSOffset { type_name } if type_name == "ContactInfo"))
        .expect("should have ContactInfo TableSOffset");
    let address_soffset = annotations
        .iter()
        .find(|r| matches!(&r.region_type, RegionType::TableSOffset { type_name } if type_name == "Address"))
        .expect("should have Address TableSOffset");

    assert!(
        contact_soffset.depth > employee_soffset.depth,
        "ContactInfo depth ({}) should be greater than Employee depth ({})",
        contact_soffset.depth,
        employee_soffset.depth
    );
    assert!(
        address_soffset.depth > contact_soffset.depth,
        "Address depth ({}) should be greater than ContactInfo depth ({})",
        address_soffset.depth,
        contact_soffset.depth
    );
}

#[test]
#[ignore] // helper to generate hex for template
fn test_generate_nested_tables_hex() {
    let input = json!({
        "name": "Alice",
        "age": 30,
        "contact": {
            "email": "alice@example.com",
            "phone": "555-1234",
            "address": {
                "street": "123 Main St",
                "city": "Springfield",
                "zip": 62701
            }
        }
    });
    let binary = roundtrip_binary(NESTED_TABLES_SCHEMA_JSON, "Employee", &input);
    let hex: Vec<String> = binary.iter().map(|b| format!("{b:02x}")).collect();
    println!("HEX ({} bytes):\n{}", binary.len(), hex.join(" "));
}

#[test]
fn test_nested_tables_full_byte_coverage() {
    let input = json!({
        "name": "Alice",
        "age": 30,
        "contact": {
            "email": "alice@example.com",
            "phone": "555-1234",
            "address": {
                "street": "123 Main St",
                "city": "Springfield",
                "zip": 62701
            }
        }
    });

    let result = load_schema_from_json(NESTED_TABLES_SCHEMA_JSON).unwrap();
    let binary = encode_json(&input, &result.schema, "Employee").unwrap();
    let annotations = walk_binary(&binary, &result.schema, "Employee").unwrap();

    let mut covered = vec![false; binary.len()];
    for region in &annotations {
        for i in region.byte_range.clone() {
            if i < binary.len() {
                covered[i] = true;
            }
        }
    }
    for (i, &c) in covered.iter().enumerate() {
        assert!(c, "byte {} is not covered by any region", i);
    }
}

// ===========================================================================
// Template 6: Union
// ===========================================================================

const UNION_SCHEMA: &str = r#"table Sword {
  damage: int;
  name: string;
}

table Shield {
  armor: int;
  weight: float;
}

union Equipment { Sword, Shield }

table Hero {
  name: string;
  equipped: Equipment;
}

root_type Hero;
"#;

#[test]
fn test_roundtrip_union_sword() {
    let input = json!({
        "name": "Knight",
        "equipped_type": "Sword",
        "equipped": {
            "damage": 50,
            "name": "Excalibur"
        }
    });
    let output = roundtrip_from_fbs(UNION_SCHEMA, &input);
    assert_eq!(output["name"], json!("Knight"));
}

#[test]
fn test_roundtrip_union_shield() {
    let input = json!({
        "name": "Guardian",
        "equipped_type": "Shield",
        "equipped": {
            "armor": 80,
            "weight": 5.5
        }
    });
    let output = roundtrip_from_fbs(UNION_SCHEMA, &input);
    assert_eq!(output["name"], json!("Guardian"));
}

#[test]
fn test_union_region_types() {
    let input = json!({
        "name": "Knight",
        "equipped_type": "Sword",
        "equipped": {
            "damage": 50,
            "name": "Excalibur"
        }
    });
    let (_binary, annotations) = encode_and_walk_from_fbs(UNION_SCHEMA, &input);

    let has_union_type = annotations
        .iter()
        .any(|r| matches!(&r.region_type, RegionType::UnionTypeField { .. }));
    assert!(has_union_type, "should have UnionTypeField region");

    let has_union_offset = annotations
        .iter()
        .any(|r| matches!(&r.region_type, RegionType::UnionDataOffset { .. }));
    assert!(has_union_offset, "should have UnionDataOffset region");
}

// ===========================================================================
// Template 7: Vector of Tables
// ===========================================================================

const VECTOR_OF_TABLES_SCHEMA: &str = r#"table Item {
  name: string;
  quantity: int;
}

table Inventory {
  items: [Item];
  owner: string;
}

root_type Inventory;
"#;

#[test]
fn test_roundtrip_vector_of_tables() {
    let input = json!({
        "items": [
            { "name": "Potion", "quantity": 5 },
            { "name": "Arrow", "quantity": 20 },
            { "name": "Shield", "quantity": 1 }
        ],
        "owner": "Adventurer"
    });
    let output = roundtrip_from_fbs(VECTOR_OF_TABLES_SCHEMA, &input);
    assert_eq!(output["owner"], json!("Adventurer"));

    let items = output["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["name"], json!("Potion"));
    assert_eq!(items[0]["quantity"], json!(5));
    assert_eq!(items[1]["name"], json!("Arrow"));
    assert_eq!(items[1]["quantity"], json!(20));
    assert_eq!(items[2]["name"], json!("Shield"));
    assert_eq!(items[2]["quantity"], json!(1));
}

#[test]
fn test_vector_of_tables_regions() {
    let input = json!({
        "items": [
            { "name": "A", "quantity": 1 },
            { "name": "B", "quantity": 2 }
        ],
        "owner": "Test"
    });
    let (_binary, annotations) = encode_and_walk_from_fbs(VECTOR_OF_TABLES_SCHEMA, &input);

    // Should have VectorOffset for items
    let vec_offset = annotations
        .iter()
        .any(|r| matches!(&r.region_type, RegionType::VectorOffset { field_name } if field_name == "items"));
    assert!(vec_offset, "should have VectorOffset for items");

    // Should have multiple TableSOffset (root + 2 items)
    let table_count = annotations
        .iter()
        .filter(|r| matches!(&r.region_type, RegionType::TableSOffset { .. }))
        .count();
    assert!(
        table_count >= 3,
        "expected 3+ TableSOffset regions (root + 2 items), got {table_count}"
    );
}

#[test]
fn test_vector_of_tables_empty() {
    let input = json!({
        "items": [],
        "owner": "Empty"
    });
    let output = roundtrip_from_fbs(VECTOR_OF_TABLES_SCHEMA, &input);
    assert_eq!(output["owner"], json!("Empty"));
    let items = output["items"].as_array().unwrap();
    assert_eq!(items.len(), 0);
}

// ===========================================================================
// Template 8: Vector of Strings
// ===========================================================================

const VECTOR_OF_STRINGS_SCHEMA: &str = r#"table TagList {
  title: string;
  tags: [string];
  count: int;
}

root_type TagList;
"#;

#[test]
fn test_roundtrip_vector_of_strings() {
    let input = json!({
        "title": "Languages",
        "tags": ["Rust", "TypeScript", "Go", "Python"],
        "count": 4
    });
    let output = roundtrip_from_fbs(VECTOR_OF_STRINGS_SCHEMA, &input);
    assert_eq!(output["title"], json!("Languages"));
    assert_eq!(output["count"], json!(4));

    let tags = output["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 4);
    assert_eq!(tags[0], json!("Rust"));
    assert_eq!(tags[1], json!("TypeScript"));
    assert_eq!(tags[2], json!("Go"));
    assert_eq!(tags[3], json!("Python"));
}

#[test]
fn test_vector_of_strings_empty() {
    let input = json!({
        "title": "Empty",
        "tags": [],
        "count": 0
    });
    let output = roundtrip_from_fbs(VECTOR_OF_STRINGS_SCHEMA, &input);
    let tags = output["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 0);
}

#[test]
fn test_vector_of_strings_tree_structure() {
    let input = json!({
        "title": "Languages",
        "tags": ["Rust", "TypeScript", "Go", "Python"],
        "count": 4
    });
    let (_, annotations) = encode_and_walk_from_fbs(VECTOR_OF_STRINGS_SCHEMA, &input);

    // Check that the VectorOffset for "tags" exists and has VectorElement children
    let vec_offset_idx = annotations.iter().position(|r| {
        matches!(&r.region_type, RegionType::VectorOffset { field_name } if field_name == "tags")
    }).expect("Should have VectorOffset for tags");

    let vec_region = &annotations[vec_offset_idx];
    let elem_count = vec_region
        .children
        .iter()
        .filter(|&&c| {
            matches!(
                &annotations[c].region_type,
                RegionType::VectorElement { .. }
            )
        })
        .count();
    assert_eq!(
        elem_count, 4,
        "Should have 4 VectorElement children for tags"
    );

    // Each VectorElement should have StringData children
    for &child_idx in &vec_region.children {
        let child = &annotations[child_idx];
        if let RegionType::VectorElement { index } = &child.region_type {
            let has_string_data = child
                .children
                .iter()
                .any(|&c| matches!(&annotations[c].region_type, RegionType::StringData { .. }));
            assert!(
                has_string_data,
                "VectorElement [{index}] should have StringData child"
            );
        }
    }
}

// ===========================================================================
// Template 9: All Scalar Types
// ===========================================================================

const ALL_SCALAR_TYPES_SCHEMA: &str = r#"table AllScalars {
  f_bool: bool;
  f_byte: byte;
  f_ubyte: ubyte;
  f_short: short;
  f_ushort: ushort;
  f_int: int;
  f_uint: uint;
  f_long: long;
  f_ulong: ulong;
  f_float: float;
  f_double: double;
}

root_type AllScalars;
"#;

#[test]
fn test_roundtrip_all_scalar_types() {
    let input = json!({
        "f_bool": true,
        "f_byte": -42,
        "f_ubyte": 255,
        "f_short": -1000,
        "f_ushort": 65535,
        "f_int": -100000,
        "f_uint": 4000000000u64,
        "f_long": -9000000000000i64,
        "f_ulong": 18000000000000000000u64,
        "f_float": 3.14,
        "f_double": 2.718281828
    });
    let output = roundtrip_from_fbs(ALL_SCALAR_TYPES_SCHEMA, &input);

    assert_eq!(output["f_bool"], json!(true));
    assert_eq!(output["f_byte"], json!(-42));
    assert_eq!(output["f_ubyte"], json!(255));
    assert_eq!(output["f_short"], json!(-1000));
    assert_eq!(output["f_ushort"], json!(65535));
    assert_eq!(output["f_int"], json!(-100000));
    assert_eq!(output["f_uint"], json!(4000000000u64));
    assert_eq!(output["f_long"], json!(-9000000000000i64));
    assert_eq!(output["f_ulong"], json!(18000000000000000000u64));

    let f = output["f_float"].as_f64().unwrap();
    assert!((f - 3.14).abs() < 0.01, "f_float={f}");
    let d = output["f_double"].as_f64().unwrap();
    assert!((d - 2.718281828).abs() < 0.000001, "f_double={d}");
}

#[test]
fn test_all_scalar_types_byte_coverage() {
    let input = json!({
        "f_bool": true,
        "f_byte": -42,
        "f_ubyte": 255,
        "f_short": -1000,
        "f_ushort": 65535,
        "f_int": -100000,
        "f_uint": 4000000000u64,
        "f_long": -9000000000000i64,
        "f_ulong": 18000000000000000000u64,
        "f_float": 3.14,
        "f_double": 2.718281828
    });
    let (binary, annotations) = encode_and_walk_from_fbs(ALL_SCALAR_TYPES_SCHEMA, &input);

    let mut covered = vec![false; binary.len()];
    for region in &annotations {
        for i in region.byte_range.clone() {
            if i < binary.len() {
                covered[i] = true;
            }
        }
    }
    for (i, &c) in covered.iter().enumerate() {
        assert!(c, "byte {} is not covered by any region", i);
    }
}

// ===========================================================================
// Template 10: Default Values
// ===========================================================================

const DEFAULT_VALUES_SCHEMA: &str = r#"table Settings {
  width: int = 800;
  height: int = 600;
  fullscreen: bool = false;
  volume: float = 0.5;
  title: string;
  fps_limit: int = 60;
}

root_type Settings;
"#;

#[test]
fn test_roundtrip_default_values_overrides() {
    let input = json!({
        "width": 1920,
        "height": 1080,
        "fullscreen": true,
        "volume": 0.5,
        "title": "My Game",
        "fps_limit": 60
    });
    let output = roundtrip_from_fbs(DEFAULT_VALUES_SCHEMA, &input);

    assert_eq!(output["width"], json!(1920));
    assert_eq!(output["height"], json!(1080));
    assert_eq!(output["fullscreen"], json!(true));
    assert_eq!(output["title"], json!("My Game"));
    // volume=0.5 is the default, may be omitted by encoder
    // fps_limit=60 is the default, may be omitted by encoder
}

#[test]
fn test_roundtrip_default_values_only_defaults() {
    // All fields use defaults -- minimal binary size
    let input = json!({});
    let result = flatc_rs_compiler::compile_single(DEFAULT_VALUES_SCHEMA).unwrap();
    let root_name = result
        .schema
        .root_table
        .as_ref()
        .and_then(|t| t.name.as_deref())
        .unwrap();
    let binary = encode_json(&input, &result.schema, root_name).unwrap();
    let annotations = walk_binary(&binary, &result.schema, root_name).unwrap();
    assert!(
        !annotations.is_empty(),
        "walker should produce regions even for all-defaults"
    );
}

// ===========================================================================
// Template 11: Vector of Structs
// ===========================================================================

const VECTOR_OF_STRUCTS_SCHEMA: &str = r#"struct Point {
  x: float;
  y: float;
}

table Path {
  name: string;
  points: [Point];
  closed: bool;
}

root_type Path;
"#;

#[test]
fn test_roundtrip_vector_of_structs() {
    let input = json!({
        "name": "Triangle",
        "points": [
            { "x": 0.0, "y": 0.0 },
            { "x": 100.0, "y": 0.0 },
            { "x": 50.0, "y": 86.6 }
        ],
        "closed": true
    });
    let output = roundtrip_from_fbs(VECTOR_OF_STRUCTS_SCHEMA, &input);

    assert_eq!(output["name"], json!("Triangle"));
    assert_eq!(output["closed"], json!(true));

    let points = output["points"].as_array().unwrap();
    assert_eq!(points.len(), 3);

    let p0x = points[0]["x"].as_f64().unwrap();
    assert!((p0x - 0.0).abs() < 0.001, "p0.x={p0x}");
    let p1x = points[1]["x"].as_f64().unwrap();
    assert!((p1x - 100.0).abs() < 0.001, "p1.x={p1x}");
    let p2y = points[2]["y"].as_f64().unwrap();
    assert!((p2y - 86.6).abs() < 0.1, "p2.y={p2y}");
}

#[test]
fn test_vector_of_structs_empty() {
    let input = json!({
        "name": "Empty",
        "points": [],
        "closed": false
    });
    let output = roundtrip_from_fbs(VECTOR_OF_STRUCTS_SCHEMA, &input);
    let points = output["points"].as_array().unwrap();
    assert_eq!(points.len(), 0);
}

#[test]
fn test_vector_of_structs_contiguous_regions() {
    let input = json!({
        "name": "Line",
        "points": [
            { "x": 0.0, "y": 0.0 },
            { "x": 10.0, "y": 20.0 }
        ],
        "closed": false
    });
    let (_binary, annotations) = encode_and_walk_from_fbs(VECTOR_OF_STRUCTS_SCHEMA, &input);

    // Vector of structs should have VectorElement regions (not TableSOffset)
    let vec_elements = annotations
        .iter()
        .filter(|r| matches!(&r.region_type, RegionType::VectorElement { .. }))
        .count();
    assert!(
        vec_elements >= 2,
        "expected 2+ VectorElement regions, got {vec_elements}"
    );

    // No extra TableSOffset for struct elements (they're inline)
    let table_count = annotations
        .iter()
        .filter(|r| matches!(&r.region_type, RegionType::TableSOffset { .. }))
        .count();
    assert_eq!(
        table_count, 1,
        "only root table should have TableSOffset, got {table_count}"
    );
}

// ===========================================================================
// Template 12: File Identifier
// ===========================================================================

const FILE_IDENTIFIER_SCHEMA: &str = r#"table Document {
  version: int;
  title: string;
  page_count: int;
}

root_type Document;
file_identifier "DOCS";
"#;

#[test]
fn test_roundtrip_file_identifier() {
    let input = json!({
        "version": 3,
        "title": "FlatBuffers Guide",
        "page_count": 42
    });
    let output = roundtrip_from_fbs(FILE_IDENTIFIER_SCHEMA, &input);

    assert_eq!(output["version"], json!(3));
    assert_eq!(output["title"], json!("FlatBuffers Guide"));
    assert_eq!(output["page_count"], json!(42));
}

#[test]
fn test_file_identifier_region() {
    let input = json!({
        "version": 3,
        "title": "FlatBuffers Guide",
        "page_count": 42
    });
    let (binary, annotations) = encode_and_walk_from_fbs(FILE_IDENTIFIER_SCHEMA, &input);

    // File identifier should be at bytes 4-7
    let file_id = annotations
        .iter()
        .find(|r| matches!(&r.region_type, RegionType::FileIdentifier));
    assert!(file_id.is_some(), "should have FileIdentifier region");

    let file_id = file_id.unwrap();
    assert_eq!(
        file_id.byte_range,
        4..8,
        "file identifier should be at bytes 4-8"
    );
    assert!(
        file_id.value_display.contains("DOCS"),
        "file identifier should show 'DOCS', got: {}",
        file_id.value_display
    );

    // Verify the actual bytes
    assert_eq!(&binary[4..8], b"DOCS");
}

// ===========================================================================
// Hex generation helper (run with --ignored to generate template hex data)
// ===========================================================================

#[test]
#[ignore]
fn test_generate_all_template_hex() {
    let templates: Vec<(&str, &str, Value)> = vec![
        (
            "Union (Sword)",
            UNION_SCHEMA,
            json!({
                "name": "Knight",
                "equipped_type": "Sword",
                "equipped": { "damage": 50, "name": "Excalibur" }
            }),
        ),
        (
            "Vector of Tables",
            VECTOR_OF_TABLES_SCHEMA,
            json!({
                "items": [
                    { "name": "Potion", "quantity": 5 },
                    { "name": "Arrow", "quantity": 20 },
                    { "name": "Shield", "quantity": 1 }
                ],
                "owner": "Adventurer"
            }),
        ),
        (
            "Vector of Strings",
            VECTOR_OF_STRINGS_SCHEMA,
            json!({
                "title": "Languages",
                "tags": ["Rust", "TypeScript", "Go", "Python"],
                "count": 4
            }),
        ),
        (
            "All Scalar Types",
            ALL_SCALAR_TYPES_SCHEMA,
            json!({
                "f_bool": true, "f_byte": -42, "f_ubyte": 255,
                "f_short": -1000, "f_ushort": 65535,
                "f_int": -100000, "f_uint": 4000000000u64,
                "f_long": -9000000000000i64, "f_ulong": 18000000000000000000u64,
                "f_float": 3.14, "f_double": 2.718281828
            }),
        ),
        (
            "Default Values",
            DEFAULT_VALUES_SCHEMA,
            json!({
                "width": 1920, "height": 1080, "fullscreen": true,
                "volume": 0.5, "title": "My Game", "fps_limit": 60
            }),
        ),
        (
            "Vector of Structs",
            VECTOR_OF_STRUCTS_SCHEMA,
            json!({
                "name": "Triangle",
                "points": [
                    { "x": 0.0, "y": 0.0 },
                    { "x": 100.0, "y": 0.0 },
                    { "x": 50.0, "y": 86.6 }
                ],
                "closed": true
            }),
        ),
        (
            "File Identifier",
            FILE_IDENTIFIER_SCHEMA,
            json!({
                "version": 3, "title": "FlatBuffers Guide", "page_count": 42
            }),
        ),
    ];

    for (name, schema, input) in &templates {
        let result = flatc_rs_compiler::compile_single(schema).unwrap();
        let root_name = result
            .schema
            .root_table
            .as_ref()
            .and_then(|t| t.name.as_deref())
            .unwrap();
        let binary = encode_json(input, &result.schema, root_name).unwrap();
        let hex: Vec<String> = binary.iter().map(|b| format!("{b:02x}")).collect();
        println!("{name} ({} bytes):\n{}\n", binary.len(), hex.join(" "));
    }
}

// ===========================================================================
// Template 13: Struct with force_align
// ===========================================================================

const FORCE_ALIGN_STRUCT_SCHEMA: &str = r#"struct StructAlpha (force_align: 16) {
  field_a: ulong;
  field_b: ubyte;
  field_c: uint;
  field_d: double;
}

table Root {
  alpha: StructAlpha;
  label: string;
}

root_type Root;
"#;

#[test]
fn test_roundtrip_force_align_struct() {
    let input = json!({
        "alpha": {
            "field_a": 12345678901u64,
            "field_b": 42,
            "field_c": 999999,
            "field_d": 59.19
        },
        "label": "test"
    });
    let output = roundtrip_from_fbs(FORCE_ALIGN_STRUCT_SCHEMA, &input);

    let field_a = output["alpha"]["field_a"].as_u64().unwrap();
    assert_eq!(field_a, 12345678901, "field_a mismatch");

    let field_b = output["alpha"]["field_b"].as_u64().unwrap();
    assert_eq!(field_b, 42, "field_b mismatch");

    let field_c = output["alpha"]["field_c"].as_u64().unwrap();
    assert_eq!(field_c, 999999, "field_c mismatch");

    let field_d = output["alpha"]["field_d"].as_f64().unwrap();
    assert!(
        (field_d - 59.19).abs() < 0.001,
        "field_d mismatch: got {field_d}, expected 59.19"
    );
}

#[test]
fn test_force_align_struct_byte_layout() {
    let input = json!({
        "alpha": {
            "field_a": 1u64,
            "field_b": 2,
            "field_c": 3,
            "field_d": 4.0
        },
        "label": "hi"
    });
    let (binary, annotations) = encode_and_walk_from_fbs(FORCE_ALIGN_STRUCT_SCHEMA, &input);

    // Find the StructInline region for StructAlpha
    let struct_region = annotations
        .iter()
        .find(|r| {
            matches!(&r.region_type, RegionType::StructInline { type_name } if type_name == "StructAlpha")
        })
        .expect("should have StructInline for StructAlpha");

    // StructAlpha with force_align:16 should be 32 bytes
    let struct_size = struct_region.byte_range.end - struct_region.byte_range.start;
    assert_eq!(
        struct_size, 32,
        "StructAlpha should be 32 bytes (force_align:16)"
    );

    let struct_start = struct_region.byte_range.start;

    // field_a (ulong) at offset 0: should be 1
    let field_a = u64::from_le_bytes(binary[struct_start..struct_start + 8].try_into().unwrap());
    assert_eq!(field_a, 1, "field_a raw bytes mismatch");

    // field_b (ubyte) at offset 8: should be 2
    assert_eq!(binary[struct_start + 8], 2, "field_b raw bytes mismatch");

    // field_c (uint) at offset 12: should be 3
    let field_c = u32::from_le_bytes(
        binary[struct_start + 12..struct_start + 16]
            .try_into()
            .unwrap(),
    );
    assert_eq!(field_c, 3, "field_c raw bytes mismatch");

    // field_d (double) at offset 16: should be 4.0
    let field_d = f64::from_le_bytes(
        binary[struct_start + 16..struct_start + 24]
            .try_into()
            .unwrap(),
    );
    assert!(
        (field_d - 4.0).abs() < 0.001,
        "field_d raw bytes mismatch: got {field_d}, expected 4.0"
    );

    // Find the field_d StructField annotation and verify it reads the right value
    let field_d_region = annotations.iter().find(|r| {
        matches!(&r.region_type, RegionType::StructField { field_name, .. } if field_name == "field_d")
    });
    if let Some(region) = field_d_region {
        eprintln!(
            "field_d region: byte_range={:?}, value_display={}",
            region.byte_range, region.value_display
        );
        // The byte range should be [struct_start+16 .. struct_start+24]
        assert_eq!(
            region.byte_range.start,
            struct_start + 16,
            "field_d region start mismatch"
        );
        assert_eq!(
            region.byte_range.end,
            struct_start + 24,
            "field_d region end mismatch"
        );
        assert!(
            region.value_display.contains("4"),
            "field_d display should contain '4', got: {}",
            region.value_display
        );
    } else {
        panic!("No StructField region found for field_d");
    }
}

#[test]
fn test_force_align_struct_vector() {
    // Test vector of force_align structs
    let schema = r#"struct StructAlpha (force_align: 16) {
  field_a: ulong;
  field_b: ubyte;
  field_c: uint;
  field_d: double;
}

table Root {
  items: [StructAlpha];
}

root_type Root;
"#;

    let input = json!({
        "items": [
            {
                "field_a": 100u64,
                "field_b": 10,
                "field_c": 1000,
                "field_d": 59.19
            },
            {
                "field_a": 200u64,
                "field_b": 20,
                "field_c": 2000,
                "field_d": -86.89
            }
        ]
    });
    let output = roundtrip_from_fbs(schema, &input);

    let items = output["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);

    let d0 = items[0]["field_d"].as_f64().unwrap();
    assert!(
        (d0 - 59.19).abs() < 0.001,
        "items[0].field_d mismatch: got {d0}, expected 59.19"
    );

    let d1 = items[1]["field_d"].as_f64().unwrap();
    assert!(
        (d1 - (-86.89)).abs() < 0.001,
        "items[1].field_d mismatch: got {d1}, expected -86.89"
    );
}
