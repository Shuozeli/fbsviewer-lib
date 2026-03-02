use std::fmt;
use std::str::FromStr;

use flatbuf_visualizer_core::AnnotatedRegion;
use serde::Serialize;

// ---------------------------------------------------------------------------
// OutputFormat enum for clap
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Json,
    Compact,
}

impl FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            "compact" => Ok(OutputFormat::Compact),
            other => Err(format!(
                "unknown format '{other}': expected 'table', 'json', or 'compact'"
            )),
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Compact => write!(f, "compact"),
        }
    }
}

// ---------------------------------------------------------------------------
// Serializable output for JSON format
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct RegionOutput {
    byte_start: String,
    byte_end: String,
    size: usize,
    region_type: String,
    label: String,
    field_path: String,
    value: String,
    depth: usize,
}

impl RegionOutput {
    fn from_annotated(region: &AnnotatedRegion) -> Self {
        Self {
            byte_start: format!("0x{:04X}", region.byte_range.start),
            byte_end: format!("0x{:04X}", region.byte_range.end),
            size: region.byte_range.end - region.byte_range.start,
            region_type: region.region_type.short_name().to_string(),
            label: region.label.clone(),
            field_path: region.field_path.join("."),
            value: region.value_display.clone(),
            depth: region.depth,
        }
    }
}

// ---------------------------------------------------------------------------
// Render dispatch
// ---------------------------------------------------------------------------

pub fn render(all_regions: &[AnnotatedRegion], filtered_indices: &[usize], format: &OutputFormat) {
    match format {
        OutputFormat::Table => render_table(all_regions, filtered_indices),
        OutputFormat::Json => render_json(all_regions, filtered_indices),
        OutputFormat::Compact => render_compact(all_regions, filtered_indices),
    }
}

fn render_table(regions: &[AnnotatedRegion], indices: &[usize]) {
    println!(
        "{:<15} {:>5}  {:<18} {:<30} {:<30} VALUE",
        "BYTE RANGE", "SIZE", "TYPE", "LABEL", "PATH"
    );
    println!("{}", "-".repeat(120));

    for &idx in indices {
        let r = &regions[idx];
        let range = format!("0x{:04X}..0x{:04X}", r.byte_range.start, r.byte_range.end);
        let size = r.byte_range.end - r.byte_range.start;
        let indent = "  ".repeat(r.depth);
        let label = format!("{}{}", indent, r.label);

        println!(
            "{:<15} {:>5}  {:<18} {:<30} {:<30} {}",
            range,
            size,
            r.region_type.short_name(),
            truncate(&label, 30),
            truncate(&r.field_path.join("."), 30),
            r.value_display,
        );
    }

    println!();
    println!("{} region(s) shown.", indices.len());
}

fn render_json(regions: &[AnnotatedRegion], indices: &[usize]) {
    let output: Vec<RegionOutput> = indices
        .iter()
        .map(|&idx| RegionOutput::from_annotated(&regions[idx]))
        .collect();

    match serde_json::to_string_pretty(&output) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("error: failed to serialize JSON: {e}");
            std::process::exit(1);
        }
    }
}

fn render_compact(regions: &[AnnotatedRegion], indices: &[usize]) {
    for &idx in indices {
        let r = &regions[idx];
        println!(
            "0x{:04X}..0x{:04X} {} {} [{}] = {}",
            r.byte_range.start,
            r.byte_range.end,
            r.region_type.short_name(),
            r.label,
            r.field_path.join("."),
            r.value_display,
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
