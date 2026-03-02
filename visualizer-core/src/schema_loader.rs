use flatc_rs_schema::Schema;

#[derive(Debug, thiserror::Error)]
pub enum SchemaLoadError {
    #[error("invalid schema JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

/// Result of loading a schema from JSON.
pub struct SchemaLoadResult {
    pub schema: Schema,
    pub root_type_name: Option<String>,
}

/// Deserialize a [Schema] from its JSON representation and extract the root type name.
pub fn load_schema_from_json(json: &str) -> Result<SchemaLoadResult, SchemaLoadError> {
    let schema: Schema = serde_json::from_str(json)?;
    let root_type_name = schema.root_table.as_ref().and_then(|rt| rt.name.clone());
    Ok(SchemaLoadResult {
        schema,
        root_type_name,
    })
}
