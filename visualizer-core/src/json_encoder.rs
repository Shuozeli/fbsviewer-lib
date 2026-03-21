//! Encode a JSON value into a FlatBuffers binary using a compiled Schema.
//!
//! This is the inverse of the binary_walker + json_decoder pipeline:
//! instead of reading binary and producing JSON, it reads JSON and produces binary.

use flatc_rs_schema::resolved::ResolvedSchema;
use flatc_rs_schema::{BaseType, Enum, Field, Object, Schema, Type};
use serde_json::Value;

const MAX_DEPTH: usize = 64;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum JsonEncodeError {
    #[error("root type '{name}' not found in schema")]
    RootTypeNotFound { name: String },

    #[error("expected JSON object for table/struct '{type_name}', got {actual}")]
    ExpectedObject { type_name: String, actual: String },

    #[error("expected JSON array for vector field '{field_name}', got {actual}")]
    ExpectedArray { field_name: String, actual: String },

    #[error("expected JSON number for field '{field_name}' ({base_type}), got {actual}")]
    ExpectedNumber {
        field_name: String,
        base_type: String,
        actual: String,
    },

    #[error("expected JSON string for field '{field_name}', got {actual}")]
    ExpectedString { field_name: String, actual: String },

    #[error("unknown field '{field_name}' in table '{type_name}'")]
    UnknownField {
        type_name: String,
        field_name: String,
    },

    #[error("unknown enum value '{value}' for enum '{enum_name}'")]
    UnknownEnumValue { enum_name: String, value: String },

    #[error("union field '{field_name}' requires companion '{field_name}_type' field")]
    MissingUnionType { field_name: String },

    #[error("object index {index} out of range (have {count} objects)")]
    ObjectIndexOutOfRange { index: usize, count: usize },

    #[error("enum index {index} out of range (have {count} enums)")]
    EnumIndexOutOfRange { index: usize, count: usize },

    #[error("encoding depth exceeded maximum of {max}")]
    MaxDepthExceeded { max: usize },

    #[error("struct field '{field_name}' is missing in JSON (structs require all fields)")]
    MissingStructField { field_name: String },

    #[error("number out of range for field '{field_name}' ({base_type}): {value}")]
    NumberOutOfRange {
        field_name: String,
        base_type: String,
        value: String,
    },
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Encode a JSON value into a FlatBuffers binary.
///
/// `json` must be a JSON object representing the root table.
/// `schema` is the compiled FlatBuffers schema.
/// `root_type` is the name of the root table type.
pub fn encode_json(
    json: &Value,
    schema: &ResolvedSchema,
    root_type: &str,
) -> Result<Vec<u8>, JsonEncodeError> {
    let legacy = schema.as_legacy();
    let mut enc = Encoder::new(&legacy);
    enc.encode(json, root_type)
}

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

struct Encoder<'a> {
    schema: &'a Schema,
    buf: Vec<u8>,
}

impl<'a> Encoder<'a> {
    fn new(schema: &'a Schema) -> Self {
        Self {
            schema,
            buf: Vec::with_capacity(256),
        }
    }

    fn encode(&mut self, json: &Value, root_type: &str) -> Result<Vec<u8>, JsonEncodeError> {
        let root_idx = self.find_object_index(root_type)?;

        // Reserve space for root offset (4 bytes)
        self.write_u32_le(0); // placeholder

        // Optionally write file identifier
        if let Some(ref ident) = self.schema.file_ident {
            let bytes = ident.as_bytes();
            for i in 0..4 {
                self.buf.push(if i < bytes.len() { bytes[i] } else { 0 });
            }
        }

        // Encode the root table
        let root_offset = self.encode_table(json, root_idx, 0)?;

        // Patch root offset
        self.patch_u32_le(0, root_offset as u32);

        Ok(std::mem::take(&mut self.buf))
    }

    // -------------------------------------------------------------------
    // Schema lookup helpers
    // -------------------------------------------------------------------

    fn find_object_index(&self, name: &str) -> Result<usize, JsonEncodeError> {
        // Exact match first
        for (i, obj) in self.schema.objects.iter().enumerate() {
            if let Some(ref obj_name) = obj.name {
                if obj_name == name {
                    return Ok(i);
                }
            }
        }
        // Short name match (without namespace)
        for (i, obj) in self.schema.objects.iter().enumerate() {
            if let Some(ref obj_name) = obj.name {
                let short = obj_name.rsplit('.').next().unwrap_or(obj_name);
                if short == name {
                    return Ok(i);
                }
            }
        }
        Err(JsonEncodeError::RootTypeNotFound {
            name: name.to_string(),
        })
    }

    fn get_object(&self, idx: usize) -> Result<&Object, JsonEncodeError> {
        self.schema
            .objects
            .get(idx)
            .ok_or(JsonEncodeError::ObjectIndexOutOfRange {
                index: idx,
                count: self.schema.objects.len(),
            })
    }

    fn get_enum(&self, idx: usize) -> Result<&Enum, JsonEncodeError> {
        self.schema
            .enums
            .get(idx)
            .ok_or(JsonEncodeError::EnumIndexOutOfRange {
                index: idx,
                count: self.schema.enums.len(),
            })
    }

    fn obj_name(obj: &Object) -> String {
        obj.name.clone().unwrap_or_else(|| "?".to_string())
    }

    fn field_name(field: &Field) -> String {
        field.name.clone().unwrap_or_else(|| "?".to_string())
    }

    // -------------------------------------------------------------------
    // Buffer helpers
    // -------------------------------------------------------------------

    fn align(&mut self, alignment: usize) {
        while !self.buf.len().is_multiple_of(alignment) {
            self.buf.push(0);
        }
    }

    fn write_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    fn write_u16_le(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    fn write_u32_le(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    fn write_i32_le(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    fn write_bytes(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    fn patch_u32_le(&mut self, pos: usize, v: u32) {
        self.buf[pos..pos + 4].copy_from_slice(&v.to_le_bytes());
    }

    // -------------------------------------------------------------------
    // Table encoding
    // -------------------------------------------------------------------

    fn encode_table(
        &mut self,
        json: &Value,
        obj_idx: usize,
        depth: usize,
    ) -> Result<usize, JsonEncodeError> {
        if depth > MAX_DEPTH {
            return Err(JsonEncodeError::MaxDepthExceeded { max: MAX_DEPTH });
        }

        let obj = self.get_object(obj_idx)?.clone();
        let type_name = Self::obj_name(&obj);

        let json_obj = json
            .as_object()
            .ok_or_else(|| JsonEncodeError::ExpectedObject {
                type_name: type_name.clone(),
                actual: json_type_name(json),
            })?;

        // Sort fields by id for vtable construction
        let mut fields: Vec<Field> = obj.fields.clone();
        fields.sort_by_key(|f| f.id.unwrap_or(0));

        let max_field_id = fields
            .iter()
            .map(|f| f.id.unwrap_or(0) as usize)
            .max()
            .unwrap_or(0);
        let num_vtable_entries = max_field_id + 1;

        // Determine which fields are present in JSON and compute table layout
        struct FieldSlot {
            field: Field,
            field_id: usize,
            present: bool,
            size: usize,      // inline size in the table data
            alignment: usize, // field alignment
        }

        let mut slots: Vec<FieldSlot> = Vec::new();
        for field in &fields {
            let field_id = field.id.unwrap_or(0) as usize;
            let fname = Self::field_name(field);
            let ty = field.type_.as_ref();
            let bt = ty
                .and_then(|t| t.base_type)
                .unwrap_or(BaseType::BASE_TYPE_NONE);

            let present = json_obj.contains_key(&fname);
            let (size, alignment) = field_inline_size(bt, ty, self.schema);

            slots.push(FieldSlot {
                field: field.clone(),
                field_id,
                present,
                size,
                alignment,
            });
        }

        // Compute table data layout: soffset (4 bytes) + field data
        // Fields are placed in id order with alignment padding
        let mut table_data_size: usize = 4; // soffset
        let mut field_offsets_in_table: Vec<u16> = vec![0; num_vtable_entries];

        for slot in &slots {
            if !slot.present {
                continue;
            }
            if slot.alignment > 0 {
                // Align within table data
                while !table_data_size.is_multiple_of(slot.alignment) {
                    table_data_size += 1;
                }
            }
            field_offsets_in_table[slot.field_id] = table_data_size as u16;
            table_data_size += slot.size;
        }

        // Align table_data_size to 4
        while !table_data_size.is_multiple_of(4) {
            table_data_size += 1;
        }

        // Write vtable
        let vtable_size: u16 = (4 + num_vtable_entries * 2) as u16;
        self.align(2);
        let vtable_pos = self.buf.len();
        self.write_u16_le(vtable_size);
        self.write_u16_le(table_data_size as u16);
        for offset in field_offsets_in_table.iter().take(num_vtable_entries) {
            self.write_u16_le(*offset);
        }

        // Align to 4 before table data
        self.align(4);
        let table_pos = self.buf.len();

        // Write soffset (table_pos - vtable_pos, as i32)
        let soffset = (table_pos as i32) - (vtable_pos as i32);
        self.write_i32_le(soffset);

        // Write inline field data (with placeholders for offset types)
        let mut deferred: Vec<(usize, Field)> = Vec::new(); // (slot index, field)

        // Pre-allocate table data area
        let table_data_start = table_pos;
        // We already wrote the soffset (4 bytes), now fill the rest
        let remaining = table_data_size - 4;
        self.buf.resize(self.buf.len() + remaining, 0);

        // Write inline field values
        for slot in &slots {
            if !slot.present {
                continue;
            }

            let fname = Self::field_name(&slot.field);
            let json_val = &json_obj[&fname];
            let ty = slot.field.type_.as_ref();
            let bt = ty
                .and_then(|t| t.base_type)
                .unwrap_or(BaseType::BASE_TYPE_NONE);
            let field_pos = table_data_start + field_offsets_in_table[slot.field_id] as usize;

            match bt {
                // Scalars: write inline
                BaseType::BASE_TYPE_BOOL
                | BaseType::BASE_TYPE_BYTE
                | BaseType::BASE_TYPE_U_BYTE
                | BaseType::BASE_TYPE_SHORT
                | BaseType::BASE_TYPE_U_SHORT
                | BaseType::BASE_TYPE_INT
                | BaseType::BASE_TYPE_U_INT
                | BaseType::BASE_TYPE_LONG
                | BaseType::BASE_TYPE_U_LONG
                | BaseType::BASE_TYPE_FLOAT
                | BaseType::BASE_TYPE_DOUBLE => {
                    let bytes = self.encode_scalar_value(json_val, bt, ty, &fname)?;
                    self.buf[field_pos..field_pos + bytes.len()].copy_from_slice(&bytes);
                }

                // Union type discriminant (u8)
                BaseType::BASE_TYPE_U_TYPE => {
                    let enum_idx = ty.and_then(|t| t.index).unwrap_or(0) as usize;
                    let val = self.resolve_enum_value(json_val, enum_idx, &fname)?;
                    self.buf[field_pos] = val as u8;
                }

                // Struct: inline
                BaseType::BASE_TYPE_STRUCT => {
                    let inner_idx = ty.and_then(|t| t.index).unwrap_or(0) as usize;
                    let bytes =
                        self.encode_struct_inline(json_val, inner_idx, &fname, depth + 1)?;
                    self.buf[field_pos..field_pos + bytes.len()].copy_from_slice(&bytes);
                }

                // String, Table, Vector, Union: deferred (write placeholder, serialize later)
                BaseType::BASE_TYPE_STRING
                | BaseType::BASE_TYPE_TABLE
                | BaseType::BASE_TYPE_VECTOR
                | BaseType::BASE_TYPE_UNION => {
                    // The 4 bytes at field_pos are already 0 (placeholder)
                    deferred.push((slot.field_id, slot.field.clone()));
                }

                _ => {}
            }
        }

        // Now serialize deferred children and patch uoffsets
        for (field_id, field) in &deferred {
            let fname = Self::field_name(field);
            let json_val = &json_obj[&fname];
            let ty = field.type_.as_ref();
            let bt = ty
                .and_then(|t| t.base_type)
                .unwrap_or(BaseType::BASE_TYPE_NONE);
            let field_pos = table_data_start + field_offsets_in_table[*field_id] as usize;

            match bt {
                BaseType::BASE_TYPE_STRING => {
                    let s = json_val
                        .as_str()
                        .ok_or_else(|| JsonEncodeError::ExpectedString {
                            field_name: fname.clone(),
                            actual: json_type_name(json_val),
                        })?;
                    let target = self.encode_string(s);
                    let uoffset = (target - field_pos) as u32;
                    self.patch_u32_le(field_pos, uoffset);
                }

                BaseType::BASE_TYPE_TABLE => {
                    let inner_idx = ty.and_then(|t| t.index).unwrap_or(0) as usize;
                    let target = self.encode_table(json_val, inner_idx, depth + 1)?;
                    let uoffset = (target - field_pos) as u32;
                    self.patch_u32_le(field_pos, uoffset);
                }

                BaseType::BASE_TYPE_VECTOR => {
                    let target = self.encode_vector(json_val, ty, &fname, depth + 1)?;
                    let uoffset = (target - field_pos) as u32;
                    self.patch_u32_le(field_pos, uoffset);
                }

                BaseType::BASE_TYPE_UNION => {
                    // The companion _type field has already been written as U_TYPE
                    // Now serialize the union data
                    let enum_idx = ty.and_then(|t| t.index).unwrap_or(0) as usize;
                    // Read the discriminant from the companion _type field
                    let type_field_name = format!("{fname}_type");
                    let disc_val = json_obj.get(&type_field_name).ok_or_else(|| {
                        JsonEncodeError::MissingUnionType {
                            field_name: fname.clone(),
                        }
                    })?;
                    let discriminant =
                        self.resolve_enum_value(disc_val, enum_idx, &type_field_name)?;

                    if discriminant != 0 {
                        let target = self.encode_union_data(
                            json_val,
                            enum_idx,
                            discriminant as u8,
                            &fname,
                            depth + 1,
                        )?;
                        let uoffset = (target - field_pos) as u32;
                        self.patch_u32_le(field_pos, uoffset);
                    }
                }

                _ => {}
            }
        }

        Ok(table_pos)
    }

    // -------------------------------------------------------------------
    // Scalar encoding
    // -------------------------------------------------------------------

    fn encode_scalar_value(
        &self,
        json_val: &Value,
        bt: BaseType,
        ty: Option<&Type>,
        field_name: &str,
    ) -> Result<Vec<u8>, JsonEncodeError> {
        // Check if this is an enum type
        let enum_idx = ty.and_then(|t| t.index);
        if let Some(idx) = enum_idx {
            if (idx as usize) < self.schema.enums.len() {
                // Try resolving as enum name first
                if let Value::String(_) = json_val {
                    let val = self.resolve_enum_value(json_val, idx as usize, field_name)?;
                    return self.integer_to_bytes(val, bt, field_name);
                }
            }
        }

        match bt {
            BaseType::BASE_TYPE_BOOL => {
                let v = match json_val {
                    Value::Bool(b) => *b,
                    Value::Number(n) => n.as_u64().unwrap_or(0) != 0,
                    _ => {
                        return Err(JsonEncodeError::ExpectedNumber {
                            field_name: field_name.to_string(),
                            base_type: format!("{bt:?}"),
                            actual: json_type_name(json_val),
                        });
                    }
                };
                Ok(vec![if v { 1 } else { 0 }])
            }

            BaseType::BASE_TYPE_BYTE => {
                let v = json_as_i64(json_val, field_name, bt)? as i8;
                Ok(v.to_le_bytes().to_vec())
            }
            BaseType::BASE_TYPE_U_BYTE => {
                let v = json_as_u64(json_val, field_name, bt)? as u8;
                Ok(v.to_le_bytes().to_vec())
            }
            BaseType::BASE_TYPE_SHORT => {
                let v = json_as_i64(json_val, field_name, bt)? as i16;
                Ok(v.to_le_bytes().to_vec())
            }
            BaseType::BASE_TYPE_U_SHORT => {
                let v = json_as_u64(json_val, field_name, bt)? as u16;
                Ok(v.to_le_bytes().to_vec())
            }
            BaseType::BASE_TYPE_INT => {
                let v = json_as_i64(json_val, field_name, bt)? as i32;
                Ok(v.to_le_bytes().to_vec())
            }
            BaseType::BASE_TYPE_U_INT => {
                let v = json_as_u64(json_val, field_name, bt)? as u32;
                Ok(v.to_le_bytes().to_vec())
            }
            BaseType::BASE_TYPE_LONG => {
                let v = json_as_i64(json_val, field_name, bt)?;
                Ok(v.to_le_bytes().to_vec())
            }
            BaseType::BASE_TYPE_U_LONG => {
                let v = json_as_u64(json_val, field_name, bt)?;
                Ok(v.to_le_bytes().to_vec())
            }
            BaseType::BASE_TYPE_FLOAT => {
                let v = json_as_f64(json_val, field_name, bt)? as f32;
                Ok(v.to_le_bytes().to_vec())
            }
            BaseType::BASE_TYPE_DOUBLE => {
                let v = json_as_f64(json_val, field_name, bt)?;
                Ok(v.to_le_bytes().to_vec())
            }

            _ => Ok(vec![0; bt.scalar_byte_size()]),
        }
    }

    fn integer_to_bytes(
        &self,
        val: i64,
        bt: BaseType,
        field_name: &str,
    ) -> Result<Vec<u8>, JsonEncodeError> {
        match bt {
            BaseType::BASE_TYPE_BOOL => Ok(vec![if val != 0 { 1 } else { 0 }]),
            BaseType::BASE_TYPE_BYTE => Ok((val as i8).to_le_bytes().to_vec()),
            BaseType::BASE_TYPE_U_BYTE => Ok((val as u8).to_le_bytes().to_vec()),
            BaseType::BASE_TYPE_SHORT => Ok((val as i16).to_le_bytes().to_vec()),
            BaseType::BASE_TYPE_U_SHORT => Ok((val as u16).to_le_bytes().to_vec()),
            BaseType::BASE_TYPE_INT => Ok((val as i32).to_le_bytes().to_vec()),
            BaseType::BASE_TYPE_U_INT => Ok((val as u32).to_le_bytes().to_vec()),
            BaseType::BASE_TYPE_LONG => Ok(val.to_le_bytes().to_vec()),
            BaseType::BASE_TYPE_U_LONG => Ok((val as u64).to_le_bytes().to_vec()),
            _ => Err(JsonEncodeError::NumberOutOfRange {
                field_name: field_name.to_string(),
                base_type: format!("{bt:?}"),
                value: val.to_string(),
            }),
        }
    }

    // -------------------------------------------------------------------
    // Enum resolution
    // -------------------------------------------------------------------

    fn resolve_enum_value(
        &self,
        json_val: &Value,
        enum_idx: usize,
        field_name: &str,
    ) -> Result<i64, JsonEncodeError> {
        let enum_def = self.get_enum(enum_idx)?;
        let enum_name = enum_def.name.clone().unwrap_or_else(|| "?".to_string());

        match json_val {
            Value::String(s) => {
                for ev in &enum_def.values {
                    if let Some(ref name) = ev.name {
                        if name == s {
                            return Ok(ev.value.unwrap_or(0));
                        }
                    }
                }
                Err(JsonEncodeError::UnknownEnumValue {
                    enum_name,
                    value: s.clone(),
                })
            }
            Value::Number(n) => Ok(n.as_i64().unwrap_or(0)),
            _ => Err(JsonEncodeError::ExpectedNumber {
                field_name: field_name.to_string(),
                base_type: format!("enum {enum_name}"),
                actual: json_type_name(json_val),
            }),
        }
    }

    // -------------------------------------------------------------------
    // String encoding
    // -------------------------------------------------------------------

    fn encode_string(&mut self, s: &str) -> usize {
        self.align(4);
        let pos = self.buf.len();
        self.write_u32_le(s.len() as u32);
        self.write_bytes(s.as_bytes());
        self.write_u8(0); // null terminator
        self.align(4);
        pos
    }

    // -------------------------------------------------------------------
    // Struct encoding (inline, returns bytes)
    // -------------------------------------------------------------------

    fn encode_struct_inline(
        &self,
        json_val: &Value,
        obj_idx: usize,
        _field_name: &str,
        depth: usize,
    ) -> Result<Vec<u8>, JsonEncodeError> {
        if depth > MAX_DEPTH {
            return Err(JsonEncodeError::MaxDepthExceeded { max: MAX_DEPTH });
        }

        let obj = self.get_object(obj_idx)?;
        let type_name = Self::obj_name(obj);
        let byte_size = obj.byte_size.unwrap_or(0) as usize;
        let fields = obj.fields.clone();

        let json_obj = json_val
            .as_object()
            .ok_or_else(|| JsonEncodeError::ExpectedObject {
                type_name: type_name.clone(),
                actual: json_type_name(json_val),
            })?;

        let mut data = vec![0u8; byte_size];

        for field in &fields {
            let fname = Self::field_name(field);
            let field_offset = field.offset.unwrap_or(0) as usize;
            let ty = field.type_.as_ref();
            let bt = ty
                .and_then(|t| t.base_type)
                .unwrap_or(BaseType::BASE_TYPE_NONE);

            let json_field_val =
                json_obj
                    .get(&fname)
                    .ok_or_else(|| JsonEncodeError::MissingStructField {
                        field_name: fname.clone(),
                    })?;

            match bt {
                BaseType::BASE_TYPE_STRUCT => {
                    let inner_idx = ty.and_then(|t| t.index).unwrap_or(0) as usize;
                    let inner_bytes =
                        self.encode_struct_inline(json_field_val, inner_idx, &fname, depth + 1)?;
                    let end = (field_offset + inner_bytes.len()).min(byte_size);
                    data[field_offset..end].copy_from_slice(&inner_bytes[..end - field_offset]);
                }
                _ => {
                    let bytes = self.encode_scalar_value(json_field_val, bt, ty, &fname)?;
                    let end = (field_offset + bytes.len()).min(byte_size);
                    data[field_offset..end].copy_from_slice(&bytes[..end - field_offset]);
                }
            }
        }

        Ok(data)
    }

    // -------------------------------------------------------------------
    // Vector encoding
    // -------------------------------------------------------------------

    fn encode_vector(
        &mut self,
        json_val: &Value,
        ty: Option<&Type>,
        field_name: &str,
        depth: usize,
    ) -> Result<usize, JsonEncodeError> {
        let arr = json_val
            .as_array()
            .ok_or_else(|| JsonEncodeError::ExpectedArray {
                field_name: field_name.to_string(),
                actual: json_type_name(json_val),
            })?;

        let elem_bt = ty
            .and_then(|t| t.element_type)
            .unwrap_or(BaseType::BASE_TYPE_U_BYTE);

        match elem_bt {
            bt if bt.is_scalar() => {
                let elem_size = bt.scalar_byte_size();
                let alignment = elem_size.max(4);
                self.align(alignment);
                let pos = self.buf.len();
                self.write_u32_le(arr.len() as u32);
                for (i, elem) in arr.iter().enumerate() {
                    let elem_name = format!("{field_name}[{i}]");
                    let bytes = self.encode_scalar_value(elem, bt, ty, &elem_name)?;
                    self.write_bytes(&bytes);
                }
                self.align(4);
                Ok(pos)
            }

            BaseType::BASE_TYPE_STRING => {
                // First, write the vector header (count + offset placeholders)
                self.align(4);
                let pos = self.buf.len();
                self.write_u32_le(arr.len() as u32);

                // Write placeholders for string offsets
                let mut placeholders = Vec::new();
                for _ in arr.iter() {
                    placeholders.push(self.buf.len());
                    self.write_u32_le(0); // placeholder
                }

                // Now write strings and patch offsets
                for (i, elem) in arr.iter().enumerate() {
                    let s = elem
                        .as_str()
                        .ok_or_else(|| JsonEncodeError::ExpectedString {
                            field_name: format!("{field_name}[{i}]"),
                            actual: json_type_name(elem),
                        })?;
                    let str_pos = self.encode_string(s);
                    let uoffset = (str_pos - placeholders[i]) as u32;
                    self.patch_u32_le(placeholders[i], uoffset);
                }

                Ok(pos)
            }

            BaseType::BASE_TYPE_TABLE => {
                let inner_idx = ty.and_then(|t| t.index).unwrap_or(0) as usize;
                self.align(4);
                let pos = self.buf.len();
                self.write_u32_le(arr.len() as u32);

                // Placeholders for table offsets
                let mut placeholders = Vec::new();
                for _ in arr.iter() {
                    placeholders.push(self.buf.len());
                    self.write_u32_le(0);
                }

                // Serialize tables and patch
                for (i, elem) in arr.iter().enumerate() {
                    let table_pos = self.encode_table(elem, inner_idx, depth + 1)?;
                    let uoffset = (table_pos - placeholders[i]) as u32;
                    self.patch_u32_le(placeholders[i], uoffset);
                }

                Ok(pos)
            }

            BaseType::BASE_TYPE_STRUCT => {
                let inner_idx = ty.and_then(|t| t.index).unwrap_or(0) as usize;
                let obj = self.get_object(inner_idx)?;
                let struct_align = obj.min_align.unwrap_or(1) as usize;
                // Align so that (pos + 4) -- where struct data starts -- is
                // struct-aligned.  Elements are contiguous (byte_size is already
                // padded to min_align by the schema compiler).
                let data_align = struct_align.max(4);
                while !(self.buf.len() + 4).is_multiple_of(data_align) {
                    self.buf.push(0);
                }
                let pos = self.buf.len();
                self.write_u32_le(arr.len() as u32);

                for (i, elem) in arr.iter().enumerate() {
                    let elem_name = format!("{field_name}[{i}]");
                    let bytes =
                        self.encode_struct_inline(elem, inner_idx, &elem_name, depth + 1)?;
                    self.write_bytes(&bytes);
                }
                self.align(4);

                Ok(pos)
            }

            _ => {
                // Unsupported element type -- write empty vector
                self.align(4);
                let pos = self.buf.len();
                self.write_u32_le(0);
                Ok(pos)
            }
        }
    }

    // -------------------------------------------------------------------
    // Union data encoding
    // -------------------------------------------------------------------

    fn encode_union_data(
        &mut self,
        json_val: &Value,
        enum_idx: usize,
        discriminant: u8,
        field_name: &str,
        depth: usize,
    ) -> Result<usize, JsonEncodeError> {
        let enum_def = self.get_enum(enum_idx)?;

        // Find the union variant for this discriminant
        let variant = enum_def
            .values
            .iter()
            .find(|v| v.value == Some(discriminant as i64));

        let variant = match variant {
            Some(v) => v.clone(),
            None => {
                // Unknown discriminant, skip
                return Ok(self.buf.len());
            }
        };

        let union_type = match variant.union_type {
            Some(ref t) => t.clone(),
            None => return Ok(self.buf.len()),
        };

        let variant_bt = union_type.base_type.unwrap_or(BaseType::BASE_TYPE_NONE);

        match variant_bt {
            BaseType::BASE_TYPE_TABLE => {
                let inner_idx = union_type.index.unwrap_or(0) as usize;
                self.encode_table(json_val, inner_idx, depth)
            }
            BaseType::BASE_TYPE_STRING => {
                let s = json_val
                    .as_str()
                    .ok_or_else(|| JsonEncodeError::ExpectedString {
                        field_name: field_name.to_string(),
                        actual: json_type_name(json_val),
                    })?;
                Ok(self.encode_string(s))
            }
            _ => Ok(self.buf.len()),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn json_type_name(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(_) => "bool".to_string(),
        Value::Number(_) => "number".to_string(),
        Value::String(_) => "string".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

fn json_as_i64(v: &Value, field_name: &str, bt: BaseType) -> Result<i64, JsonEncodeError> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .ok_or_else(|| JsonEncodeError::NumberOutOfRange {
                field_name: field_name.to_string(),
                base_type: format!("{bt:?}"),
                value: n.to_string(),
            }),
        Value::Bool(b) => Ok(if *b { 1 } else { 0 }),
        _ => Err(JsonEncodeError::ExpectedNumber {
            field_name: field_name.to_string(),
            base_type: format!("{bt:?}"),
            actual: json_type_name(v),
        }),
    }
}

fn json_as_u64(v: &Value, field_name: &str, bt: BaseType) -> Result<u64, JsonEncodeError> {
    match v {
        Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_i64().map(|i| i as u64))
            .or_else(|| n.as_f64().map(|f| f as u64))
            .ok_or_else(|| JsonEncodeError::NumberOutOfRange {
                field_name: field_name.to_string(),
                base_type: format!("{bt:?}"),
                value: n.to_string(),
            }),
        Value::Bool(b) => Ok(if *b { 1 } else { 0 }),
        _ => Err(JsonEncodeError::ExpectedNumber {
            field_name: field_name.to_string(),
            base_type: format!("{bt:?}"),
            actual: json_type_name(v),
        }),
    }
}

fn json_as_f64(v: &Value, field_name: &str, bt: BaseType) -> Result<f64, JsonEncodeError> {
    match v {
        Value::Number(n) => n.as_f64().ok_or_else(|| JsonEncodeError::NumberOutOfRange {
            field_name: field_name.to_string(),
            base_type: format!("{bt:?}"),
            value: n.to_string(),
        }),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        _ => Err(JsonEncodeError::ExpectedNumber {
            field_name: field_name.to_string(),
            base_type: format!("{bt:?}"),
            actual: json_type_name(v),
        }),
    }
}

/// Compute the inline size and alignment of a field within table data.
fn field_inline_size(bt: BaseType, ty: Option<&Type>, schema: &Schema) -> (usize, usize) {
    match bt {
        BaseType::BASE_TYPE_BOOL
        | BaseType::BASE_TYPE_BYTE
        | BaseType::BASE_TYPE_U_BYTE
        | BaseType::BASE_TYPE_U_TYPE => (1, 1),

        BaseType::BASE_TYPE_SHORT | BaseType::BASE_TYPE_U_SHORT => (2, 2),

        BaseType::BASE_TYPE_INT | BaseType::BASE_TYPE_U_INT | BaseType::BASE_TYPE_FLOAT => (4, 4),

        BaseType::BASE_TYPE_LONG | BaseType::BASE_TYPE_U_LONG | BaseType::BASE_TYPE_DOUBLE => {
            (8, 8)
        }

        // Offset types: stored as u32 uoffset
        BaseType::BASE_TYPE_STRING
        | BaseType::BASE_TYPE_TABLE
        | BaseType::BASE_TYPE_VECTOR
        | BaseType::BASE_TYPE_UNION => (4, 4),

        // Struct: inline, use byte_size from schema
        BaseType::BASE_TYPE_STRUCT => {
            let idx = ty.and_then(|t| t.index).unwrap_or(0) as usize;
            if let Some(obj) = schema.objects.get(idx) {
                let size = obj.byte_size.unwrap_or(0) as usize;
                let align = obj.min_align.unwrap_or(1) as usize;
                (size, align)
            } else {
                (0, 1)
            }
        }

        _ => (0, 1),
    }
}
