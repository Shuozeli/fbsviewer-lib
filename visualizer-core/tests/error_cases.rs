use flatbuf_visualizer_core::{load_schema_from_json, walk_binary, WalkError};

const DEMO_SCHEMA_JSON: &str = r#"{
  "objects": [
    {
      "name": "Monster",
      "fields": [
        { "name": "hp", "type": { "base_type": "BASE_TYPE_SHORT", "base_size": 2 }, "id": 0 }
      ],
      "is_struct": false
    }
  ],
  "enums": [],
  "root_table": {
    "name": "Monster",
    "fields": [
      { "name": "hp", "type": { "base_type": "BASE_TYPE_SHORT", "base_size": 2 }, "id": 0 }
    ],
    "is_struct": false
  }
}"#;

#[test]
fn test_buffer_too_small() {
    let result = load_schema_from_json(DEMO_SCHEMA_JSON).unwrap();
    let binary = vec![0u8, 1]; // only 2 bytes, need at least 4 for root offset
    let err = walk_binary(&binary, &result.schema, "Monster").unwrap_err();
    assert!(
        matches!(err, WalkError::OutOfBounds { .. }),
        "expected OutOfBounds, got: {err}"
    );
}

#[test]
fn test_root_type_not_found() {
    let result = load_schema_from_json(DEMO_SCHEMA_JSON).unwrap();
    let binary = vec![0u8; 32];
    let err = walk_binary(&binary, &result.schema, "NonExistent").unwrap_err();
    assert!(
        matches!(err, WalkError::RootTypeNotFound { .. }),
        "expected RootTypeNotFound, got: {err}"
    );
}

#[test]
fn test_truncated_binary() {
    let result = load_schema_from_json(DEMO_SCHEMA_JSON).unwrap();
    // Root offset points to offset 20 but buffer is only 8 bytes
    let binary = vec![
        0x14, 0x00, 0x00, 0x00, // root offset -> 20 (out of bounds)
        0x00, 0x00, 0x00, 0x00,
    ];
    let err = walk_binary(&binary, &result.schema, "Monster").unwrap_err();
    assert!(
        matches!(err, WalkError::OutOfBounds { .. }),
        "expected OutOfBounds, got: {err}"
    );
}
