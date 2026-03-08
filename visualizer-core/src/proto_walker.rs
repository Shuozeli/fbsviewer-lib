//! Bridge between protobuf-rs annotator and the shared AnnotatedRegion type.
//!
//! Converts `protoc_rs_annotator::AnnotatedRegion` (protobuf-specific) into
//! `flatc_rs_annotator::AnnotatedRegion` (shared viewer type) so the visualizer
//! can render protobuf binaries with the same UI as FlatBuffers.

use flatc_rs_annotator::{AnnotatedRegion, RegionType};
use protoc_rs_annotator::ProtoRegionKind;

/// Walk a protobuf binary with schema information, returning shared `AnnotatedRegion`s.
///
/// This is the main entry point for protobuf binary visualization. It delegates to
/// the protobuf-rs annotator and converts the results to the shared region type.
pub fn walk_protobuf(
    data: &[u8],
    schema: &protoc_rs_schema::FileDescriptorSet,
    root_message: &str,
) -> Result<Vec<AnnotatedRegion>, ProtoWalkError> {
    let proto_regions = protoc_rs_annotator::walk_protobuf(data, schema, root_message)
        .map_err(|e| ProtoWalkError::WalkFailed(e.to_string()))?;
    Ok(convert_regions(&proto_regions))
}

/// Convert protobuf-rs annotated regions to the shared viewer type.
fn convert_regions(proto_regions: &[protoc_rs_annotator::AnnotatedRegion]) -> Vec<AnnotatedRegion> {
    proto_regions
        .iter()
        .map(|r| AnnotatedRegion {
            byte_range: r.byte_range.clone(),
            region_type: convert_region_kind(&r.kind),
            label: r.label.clone(),
            field_path: r.field_path.clone(),
            value_display: r.value_display.clone(),
            children: r.children.clone(),
            related_regions: Vec::new(),
            depth: r.depth,
        })
        .collect()
}

/// Map a protobuf-rs `ProtoRegionKind` to the shared `RegionType`.
fn convert_region_kind(kind: &ProtoRegionKind) -> RegionType {
    match kind {
        ProtoRegionKind::Message { type_name } => RegionType::ProtoMessage {
            type_name: type_name.clone(),
        },
        ProtoRegionKind::Tag {
            field_number,
            wire_type,
        } => RegionType::ProtoTag {
            field_number: *field_number,
            wire_type: *wire_type as u8,
        },
        ProtoRegionKind::LengthPrefix => RegionType::ProtoLength,
        ProtoRegionKind::Varint { field_name } => RegionType::ProtoVarint {
            field_name: field_name.clone(),
        },
        ProtoRegionKind::Fixed32 { field_name } => RegionType::ProtoFixed32 {
            field_name: field_name.clone(),
        },
        ProtoRegionKind::Fixed64 { field_name } => RegionType::ProtoFixed64 {
            field_name: field_name.clone(),
        },
        ProtoRegionKind::StringData { field_name } => RegionType::ProtoString {
            field_name: field_name.clone(),
        },
        ProtoRegionKind::BytesData { field_name } => RegionType::ProtoBytes {
            field_name: field_name.clone(),
        },
        ProtoRegionKind::PackedRepeated {
            field_name,
            element_type: _,
        } => RegionType::ProtoLengthDelimited {
            field_name: field_name.clone(),
        },
        ProtoRegionKind::EnumValue {
            field_name,
            enum_name: _,
            value_name: _,
        } => RegionType::ProtoVarint {
            field_name: field_name.clone(),
        },
        ProtoRegionKind::UnknownField {
            field_number,
            wire_type,
        } => RegionType::ProtoTag {
            field_number: *field_number,
            wire_type: *wire_type as u8,
        },
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProtoWalkError {
    #[error("protobuf walk failed: {0}")]
    WalkFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_simple_message() {
        let proto_regions = vec![protoc_rs_annotator::AnnotatedRegion {
            byte_range: 0..10,
            kind: ProtoRegionKind::Message {
                type_name: ".test.Foo".to_string(),
            },
            label: "Foo".to_string(),
            field_path: vec!["Foo".to_string()],
            value_display: "10 bytes".to_string(),
            children: vec![],
            depth: 0,
        }];

        let converted = convert_regions(&proto_regions);
        assert_eq!(converted.len(), 1);
        assert_eq!(
            converted[0].region_type,
            RegionType::ProtoMessage {
                type_name: ".test.Foo".to_string()
            }
        );
        assert_eq!(converted[0].byte_range, 0..10);
        assert_eq!(converted[0].label, "Foo");
    }

    #[test]
    fn convert_preserves_children() {
        let proto_regions = vec![
            protoc_rs_annotator::AnnotatedRegion {
                byte_range: 0..10,
                kind: ProtoRegionKind::Message {
                    type_name: ".test.Foo".to_string(),
                },
                label: "Foo".to_string(),
                field_path: vec!["Foo".to_string()],
                value_display: "10 bytes".to_string(),
                children: vec![1],
                depth: 0,
            },
            protoc_rs_annotator::AnnotatedRegion {
                byte_range: 0..2,
                kind: ProtoRegionKind::Varint {
                    field_name: "id".to_string(),
                },
                label: "id: 42".to_string(),
                field_path: vec!["Foo".to_string(), "id".to_string()],
                value_display: "42".to_string(),
                children: vec![],
                depth: 1,
            },
        ];

        let converted = convert_regions(&proto_regions);
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].children, vec![1]);
        assert_eq!(
            converted[1].region_type,
            RegionType::ProtoVarint {
                field_name: "id".to_string()
            }
        );
    }
}
