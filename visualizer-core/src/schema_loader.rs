use flatc_rs_schema::resolved::ResolvedSchema;
use flatc_rs_schema::Schema;

#[derive(Debug, thiserror::Error)]
pub enum SchemaLoadError {
    #[error("invalid schema JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("schema resolution failed: {0}")]
    ResolveError(#[from] flatc_rs_schema::resolved::ResolveError),
}

/// Result of loading a schema from JSON.
pub struct SchemaLoadResult {
    pub schema: ResolvedSchema,
    pub root_type_name: Option<String>,
}

/// Deserialize a [Schema] from its JSON representation, resolve it into a
/// [ResolvedSchema], and extract the root type name.
pub fn load_schema_from_json(json: &str) -> Result<SchemaLoadResult, SchemaLoadError> {
    let parsed: Schema = serde_json::from_str(json)?;
    let root_type_name = parsed.root_table.as_ref().and_then(|rt| rt.name.clone());
    let schema = ResolvedSchema::try_from_parsed(&parsed)?;
    Ok(SchemaLoadResult {
        schema,
        root_type_name,
    })
}
