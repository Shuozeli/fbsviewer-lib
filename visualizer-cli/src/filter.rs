use std::ops::Range;

use flatbuf_visualizer_core::AnnotatedRegion;

fn parse_byte_range(s: &str) -> Result<Range<usize>, String> {
    let parts: Vec<&str> = s.split("..").collect();
    if parts.len() != 2 {
        return Err(format!(
            "invalid byte range '{s}': expected format 'START..END' (e.g. '16..32' or '0x10..0x20')"
        ));
    }
    let start = parse_offset(parts[0].trim())?;
    let end = parse_offset(parts[1].trim())?;
    if start >= end {
        return Err(format!(
            "invalid byte range: start ({start}) must be less than end ({end})"
        ));
    }
    Ok(start..end)
}

fn parse_offset(s: &str) -> Result<usize, String> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        usize::from_str_radix(hex, 16).map_err(|e| format!("invalid hex offset '{s}': {e}"))
    } else {
        s.parse::<usize>()
            .map_err(|e| format!("invalid offset '{s}': {e}"))
    }
}

fn ranges_overlap(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

/// Apply all filters and return the indices of matching regions.
pub fn apply_filters(
    regions: &[AnnotatedRegion],
    byte_range_str: Option<&str>,
    field_filter: Option<&str>,
    region_type_filter: Option<&str>,
) -> Result<Vec<usize>, String> {
    let byte_range = byte_range_str.map(parse_byte_range).transpose()?;

    Ok(regions
        .iter()
        .enumerate()
        .filter(|(_, region)| {
            if let Some(ref range) = byte_range {
                if !ranges_overlap(&region.byte_range, range) {
                    return false;
                }
            }

            if let Some(field) = field_filter {
                let path_str = region.field_path.join(".");
                if !path_str.contains(field) {
                    return false;
                }
            }

            if let Some(rt) = region_type_filter {
                if region.region_type.short_name() != rt {
                    return false;
                }
            }

            true
        })
        .map(|(i, _)| i)
        .collect())
}
