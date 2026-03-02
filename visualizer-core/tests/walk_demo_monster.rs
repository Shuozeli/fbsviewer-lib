use flatbuf_visualizer_core::{
    load_schema_from_json, parse_hex_bytes, walk_binary, AnnotatedRegion, RegionType,
};

const DEMO_HEX: &str = "14 00 00 00 10 00 20 00 04 00 10 00 12 00 14 00 18 00 1c 00 10 00 00 00 00 00 80 3f 00 00 00 40 00 00 40 40 c8 00 2c 01 0c 00 00 00 01 00 00 00 0c 00 00 00 03 00 00 00 4f 72 63 00 05 00 00 00 00 01 02 03 04";

const DEMO_SCHEMA_JSON: &str = r#"{
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

fn walk_demo() -> Vec<AnnotatedRegion> {
    let binary = parse_hex_bytes(DEMO_HEX).unwrap();
    let result = load_schema_from_json(DEMO_SCHEMA_JSON).unwrap();
    let root_name = result.root_type_name.unwrap();
    walk_binary(&binary, &result.schema, &root_name).unwrap()
}

fn find_region<'a>(
    regions: &'a [AnnotatedRegion],
    pred: impl Fn(&AnnotatedRegion) -> bool,
) -> Option<&'a AnnotatedRegion> {
    regions.iter().find(|r| pred(r))
}

#[test]
fn test_walk_produces_regions() {
    let regions = walk_demo();
    assert!(
        regions.len() > 10,
        "expected >10 regions, got {}",
        regions.len()
    );
}

#[test]
fn test_root_offset_region() {
    let regions = walk_demo();
    let root = &regions[0];
    assert_eq!(root.region_type, RegionType::RootOffset);
    assert_eq!(root.byte_range, 0..4);
    assert!(
        root.value_display.contains("0014"),
        "root offset value_display should point to 0x0014, got: {}",
        root.value_display
    );
}

#[test]
fn test_vtable_region() {
    let regions = walk_demo();
    let vtable = find_region(
        &regions,
        |r| matches!(&r.region_type, RegionType::VTable { type_name } if type_name == "Monster"),
    );
    assert!(vtable.is_some(), "should have a VTable region for Monster");
}

#[test]
fn test_scalar_mana() {
    let regions = walk_demo();
    let mana = find_region(
        &regions,
        |r| matches!(&r.region_type, RegionType::ScalarField { field_name, .. } if field_name == "mana"),
    );
    assert!(mana.is_some(), "should have a scalar field for mana");
    let mana = mana.unwrap();
    assert!(
        mana.value_display.contains("200"),
        "mana should be 200, got: {}",
        mana.value_display
    );
}

#[test]
fn test_scalar_hp() {
    let regions = walk_demo();
    let hp = find_region(
        &regions,
        |r| matches!(&r.region_type, RegionType::ScalarField { field_name, .. } if field_name == "hp"),
    );
    assert!(hp.is_some(), "should have a scalar field for hp");
    let hp = hp.unwrap();
    assert!(
        hp.value_display.contains("300"),
        "hp should be 300, got: {}",
        hp.value_display
    );
}

#[test]
fn test_struct_vec3() {
    let regions = walk_demo();
    let vec3 = find_region(
        &regions,
        |r| matches!(&r.region_type, RegionType::StructInline { type_name } if type_name == "Vec3"),
    );
    assert!(vec3.is_some(), "should have a StructInline region for Vec3");
    let vec3 = vec3.unwrap();
    assert_eq!(
        vec3.children.len(),
        3,
        "Vec3 should have 3 children (x, y, z)"
    );

    // Verify child fields
    for &child_idx in &vec3.children {
        let child = &regions[child_idx];
        assert!(
            matches!(&child.region_type, RegionType::StructField { .. }),
            "Vec3 children should be StructField"
        );
    }
}

#[test]
fn test_string_name_orc() {
    let regions = walk_demo();
    let str_data = find_region(&regions, |r| {
        matches!(&r.region_type, RegionType::StringData { .. })
    });
    assert!(str_data.is_some(), "should have a StringData region");
    let str_data = str_data.unwrap();
    assert!(
        str_data.value_display.contains("Orc"),
        "string should contain 'Orc', got: {}",
        str_data.value_display
    );
}

#[test]
fn test_vector_inventory() {
    let regions = walk_demo();
    let vec_len = find_region(&regions, |r| {
        matches!(&r.region_type, RegionType::VectorLength)
    });
    assert!(vec_len.is_some(), "should have a VectorLength region");
    let vec_len = vec_len.unwrap();
    assert!(
        vec_len.value_display.contains("5"),
        "vector length should be 5, got: {}",
        vec_len.value_display
    );

    // Count VectorElement regions
    let elem_count = regions
        .iter()
        .filter(|r| matches!(&r.region_type, RegionType::VectorElement { .. }))
        .count();
    assert_eq!(elem_count, 5, "should have 5 VectorElement regions");
}

#[test]
fn test_full_byte_coverage() {
    let binary = parse_hex_bytes(DEMO_HEX).unwrap();
    let buf_len = binary.len();
    let regions = walk_demo();

    let mut covered = vec![false; buf_len];
    for region in &regions {
        for i in region.byte_range.clone() {
            if i < buf_len {
                covered[i] = true;
            }
        }
    }

    for (i, &c) in covered.iter().enumerate() {
        assert!(c, "byte {} is not covered by any region", i);
    }
}

#[test]
fn test_region_color_exhaustive() {
    // Construct one of each variant and ensure color() returns a valid [u8; 3]
    use flatc_rs_schema::BaseType;
    let variants = vec![
        RegionType::RootOffset,
        RegionType::FileIdentifier,
        RegionType::VTable {
            type_name: "T".into(),
        },
        RegionType::VTableSize,
        RegionType::VTableTableSize,
        RegionType::VTableEntry {
            field_name: "f".into(),
            field_id: 0,
        },
        RegionType::TableSOffset {
            type_name: "T".into(),
        },
        RegionType::ScalarField {
            field_name: "f".into(),
            base_type: BaseType::BASE_TYPE_INT,
        },
        RegionType::StringOffset {
            field_name: "f".into(),
        },
        RegionType::StringLength,
        RegionType::StringData {
            field_name: "f".into(),
        },
        RegionType::StringTerminator,
        RegionType::VectorOffset {
            field_name: "f".into(),
        },
        RegionType::VectorLength,
        RegionType::VectorElement { index: 0 },
        RegionType::StructInline {
            type_name: "T".into(),
        },
        RegionType::StructField {
            field_name: "f".into(),
            base_type: BaseType::BASE_TYPE_FLOAT,
        },
        RegionType::UnionTypeField {
            field_name: "f".into(),
        },
        RegionType::UnionDataOffset {
            field_name: "f".into(),
        },
        RegionType::Padding,
        RegionType::Unknown,
    ];
    for v in &variants {
        let c = v.color();
        // Just check it doesn't panic and returns an array
        assert_eq!(c.len(), 3, "color should be [u8; 3] for {:?}", v);
    }
}

#[test]
fn test_region_short_name_exhaustive() {
    use flatc_rs_schema::BaseType;
    let variants = vec![
        RegionType::RootOffset,
        RegionType::FileIdentifier,
        RegionType::VTable {
            type_name: "T".into(),
        },
        RegionType::VTableSize,
        RegionType::VTableTableSize,
        RegionType::VTableEntry {
            field_name: "f".into(),
            field_id: 0,
        },
        RegionType::TableSOffset {
            type_name: "T".into(),
        },
        RegionType::ScalarField {
            field_name: "f".into(),
            base_type: BaseType::BASE_TYPE_INT,
        },
        RegionType::StringOffset {
            field_name: "f".into(),
        },
        RegionType::StringLength,
        RegionType::StringData {
            field_name: "f".into(),
        },
        RegionType::StringTerminator,
        RegionType::VectorOffset {
            field_name: "f".into(),
        },
        RegionType::VectorLength,
        RegionType::VectorElement { index: 0 },
        RegionType::StructInline {
            type_name: "T".into(),
        },
        RegionType::StructField {
            field_name: "f".into(),
            base_type: BaseType::BASE_TYPE_FLOAT,
        },
        RegionType::UnionTypeField {
            field_name: "f".into(),
        },
        RegionType::UnionDataOffset {
            field_name: "f".into(),
        },
        RegionType::Padding,
        RegionType::Unknown,
    ];
    for v in &variants {
        let name = v.short_name();
        assert!(
            !name.is_empty(),
            "short_name should not be empty for {:?}",
            v
        );
    }
}
