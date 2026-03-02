use serde_json::{json, Map, Value};

use crate::region::{AnnotatedRegion, RegionType};

/// Reconstruct a JSON object from walker annotations.
///
/// The walker already extracts all field values into `value_display` strings.
/// This function walks the annotation tree and builds a `serde_json::Value`.
pub fn annotations_to_json(annotations: &[AnnotatedRegion]) -> Value {
    // Find the root table region (first TableSOffset at depth 0, or depth 1)
    let root_idx = annotations
        .iter()
        .position(|r| matches!(r.region_type, RegionType::TableSOffset { .. }) && r.depth <= 1);

    match root_idx {
        Some(idx) => region_to_json(annotations, idx),
        None => Value::Null,
    }
}

fn region_to_json(annotations: &[AnnotatedRegion], idx: usize) -> Value {
    let region = &annotations[idx];

    match &region.region_type {
        RegionType::TableSOffset { .. } => {
            let mut obj = Map::new();
            for &child_idx in &region.children {
                collect_field(annotations, child_idx, &mut obj);
            }
            Value::Object(obj)
        }
        RegionType::StructInline { .. } => {
            let mut obj = Map::new();
            for &child_idx in &region.children {
                collect_field(annotations, child_idx, &mut obj);
            }
            Value::Object(obj)
        }
        _ => Value::Null,
    }
}

fn collect_field(annotations: &[AnnotatedRegion], idx: usize, obj: &mut Map<String, Value>) {
    let region = &annotations[idx];

    match &region.region_type {
        RegionType::ScalarField { field_name, .. } => {
            // Check if this is a table offset (has TableSOffset children)
            let nested_table = region
                .children
                .iter()
                .find(|&&c| matches!(&annotations[c].region_type, RegionType::TableSOffset { .. }));
            if let Some(&child_idx) = nested_table {
                obj.insert(field_name.clone(), region_to_json(annotations, child_idx));
            } else {
                obj.insert(
                    field_name.clone(),
                    parse_scalar_value(&region.value_display),
                );
            }
        }
        RegionType::StructField { field_name, .. } => {
            obj.insert(
                field_name.clone(),
                parse_scalar_value(&region.value_display),
            );
        }
        RegionType::StringOffset { field_name } => {
            // The string data is in a child of this offset region
            let value = find_string_value(annotations, idx);
            obj.insert(field_name.clone(), value);
        }
        RegionType::StructInline { type_name } => {
            // Use the last component of the field_path as the field name
            let field_name = field_name_from_path(&region.field_path, type_name);
            let mut struct_obj = Map::new();
            for &child_idx in &region.children {
                collect_field(annotations, child_idx, &mut struct_obj);
            }
            obj.insert(field_name, Value::Object(struct_obj));
        }
        RegionType::VectorOffset { field_name } => {
            let arr = build_vector(annotations, idx);
            obj.insert(field_name.clone(), arr);
        }
        RegionType::UnionTypeField { field_name } => {
            obj.insert(
                format!("{field_name}_type"),
                parse_scalar_value(&region.value_display),
            );
        }
        RegionType::UnionDataOffset { field_name } => {
            // The union variant table is a child of this offset
            for &child_idx in &region.children {
                let child = &annotations[child_idx];
                if matches!(child.region_type, RegionType::TableSOffset { .. }) {
                    obj.insert(field_name.clone(), region_to_json(annotations, child_idx));
                    return;
                }
            }
        }
        RegionType::TableSOffset { .. } => {
            // Nested table -- use field_path to determine field name
            let field_name = region
                .field_path
                .last()
                .cloned()
                .unwrap_or_else(|| "?".to_string());
            obj.insert(field_name, region_to_json(annotations, idx));
        }
        // Skip structural regions
        _ => {}
    }
}

fn find_string_value(annotations: &[AnnotatedRegion], offset_idx: usize) -> Value {
    let offset_region = &annotations[offset_idx];
    // Walk children of the string offset to find StringData
    for &child_idx in &offset_region.children {
        let child = &annotations[child_idx];
        if matches!(child.region_type, RegionType::StringData { .. }) {
            let s = &child.value_display;
            // value_display is like: "Orc" (with quotes) or just the raw text
            let trimmed = s.trim_matches('"');
            return Value::String(trimmed.to_string());
        }
    }
    Value::Null
}

fn build_vector(annotations: &[AnnotatedRegion], offset_idx: usize) -> Value {
    let offset_region = &annotations[offset_idx];
    let mut elements: Vec<(usize, Value)> = Vec::new();

    for &child_idx in &offset_region.children {
        let child = &annotations[child_idx];
        if let RegionType::VectorElement { index } = &child.region_type {
            // Element might contain children (struct/table) or be a direct scalar
            if child.children.is_empty() {
                // Direct scalar value
                elements.push((*index, parse_scalar_value(&child.value_display)));
            } else {
                // Has children -- could be a struct, table, or string
                let val = vector_element_value(annotations, child_idx);
                elements.push((*index, val));
            }
        }
    }

    elements.sort_by_key(|(idx, _)| *idx);
    Value::Array(elements.into_iter().map(|(_, v)| v).collect())
}

fn vector_element_value(annotations: &[AnnotatedRegion], elem_idx: usize) -> Value {
    let elem = &annotations[elem_idx];
    // Check children for nested types
    for &child_idx in &elem.children {
        let child = &annotations[child_idx];
        match &child.region_type {
            RegionType::TableSOffset { .. } => {
                return region_to_json(annotations, child_idx);
            }
            RegionType::StructInline { .. } => {
                let mut obj = Map::new();
                for &sc in &child.children {
                    collect_field(annotations, sc, &mut obj);
                }
                return Value::Object(obj);
            }
            RegionType::StringData { .. } => {
                let s = child.value_display.trim_matches('"');
                return Value::String(s.to_string());
            }
            _ => {}
        }
    }
    // Fallback: parse the element's value_display
    parse_scalar_value(&elem.value_display)
}

fn field_name_from_path(path: &[String], fallback: &str) -> String {
    // path is like ["Monster", "pos"] -- use last component
    if path.len() >= 2 {
        path.last().unwrap().clone()
    } else {
        fallback.to_string()
    }
}

fn parse_scalar_value(value_display: &str) -> Value {
    let s = value_display.trim();
    if s.is_empty() {
        return Value::Null;
    }

    // Format: "value (type)" or "EnumName (value)" or just "value"
    // Check for enum pattern: "Red (1)" -- name followed by numeric in parens
    if let Some(paren_start) = s.rfind(" (") {
        let before = &s[..paren_start];
        let inside = &s[paren_start + 2..s.len().saturating_sub(1)];

        // Check if this is "EnumName (numericValue)"
        if !before.is_empty()
            && before.chars().next().is_some_and(|c| c.is_alphabetic())
            && inside.parse::<i64>().is_ok()
        {
            // Enum value -- return the name
            return Value::String(before.to_string());
        }

        // Otherwise it's "numericValue (type)" -- parse the numeric part
        return parse_raw_value(before);
    }

    parse_raw_value(s)
}

fn parse_raw_value(s: &str) -> Value {
    // Try bool
    if s == "true" {
        return Value::Bool(true);
    }
    if s == "false" {
        return Value::Bool(false);
    }

    // Try integer
    if let Ok(v) = s.parse::<i64>() {
        return json!(v);
    }
    // Try unsigned (for very large u64)
    if let Ok(v) = s.parse::<u64>() {
        return json!(v);
    }
    // Try float
    if let Ok(v) = s.parse::<f64>() {
        return json!(v);
    }

    // Fallback: string
    Value::String(s.to_string())
}
