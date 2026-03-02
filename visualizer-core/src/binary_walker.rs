use std::collections::HashSet;
use std::ops::Range;

use flatc_rs_schema::{BaseType, Enum, Field, Object, Schema, Type};

use crate::region::{AnnotatedRegion, RegionType, WalkError};

// ---------------------------------------------------------------------------
// Safe buffer reader
// ---------------------------------------------------------------------------

struct BufReader<'a> {
    buf: &'a [u8],
}

impl<'a> BufReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    fn len(&self) -> usize {
        self.buf.len()
    }

    fn check_bounds(&self, offset: usize, size: usize) -> Result<(), WalkError> {
        if offset
            .checked_add(size)
            .is_none_or(|end| end > self.buf.len())
        {
            return Err(WalkError::OutOfBounds {
                offset,
                size,
                buf_len: self.buf.len(),
            });
        }
        Ok(())
    }

    fn read_u8(&self, offset: usize) -> Result<u8, WalkError> {
        self.check_bounds(offset, 1)?;
        Ok(self.buf[offset])
    }

    fn read_i8(&self, offset: usize) -> Result<i8, WalkError> {
        self.check_bounds(offset, 1)?;
        Ok(self.buf[offset] as i8)
    }

    fn read_u16_le(&self, offset: usize) -> Result<u16, WalkError> {
        self.check_bounds(offset, 2)?;
        Ok(u16::from_le_bytes([self.buf[offset], self.buf[offset + 1]]))
    }

    fn read_i16_le(&self, offset: usize) -> Result<i16, WalkError> {
        self.check_bounds(offset, 2)?;
        Ok(i16::from_le_bytes([self.buf[offset], self.buf[offset + 1]]))
    }

    fn read_u32_le(&self, offset: usize) -> Result<u32, WalkError> {
        self.check_bounds(offset, 4)?;
        Ok(u32::from_le_bytes([
            self.buf[offset],
            self.buf[offset + 1],
            self.buf[offset + 2],
            self.buf[offset + 3],
        ]))
    }

    fn read_i32_le(&self, offset: usize) -> Result<i32, WalkError> {
        self.check_bounds(offset, 4)?;
        Ok(i32::from_le_bytes([
            self.buf[offset],
            self.buf[offset + 1],
            self.buf[offset + 2],
            self.buf[offset + 3],
        ]))
    }

    fn read_u64_le(&self, offset: usize) -> Result<u64, WalkError> {
        self.check_bounds(offset, 8)?;
        Ok(u64::from_le_bytes([
            self.buf[offset],
            self.buf[offset + 1],
            self.buf[offset + 2],
            self.buf[offset + 3],
            self.buf[offset + 4],
            self.buf[offset + 5],
            self.buf[offset + 6],
            self.buf[offset + 7],
        ]))
    }

    fn read_i64_le(&self, offset: usize) -> Result<i64, WalkError> {
        self.check_bounds(offset, 8)?;
        Ok(i64::from_le_bytes([
            self.buf[offset],
            self.buf[offset + 1],
            self.buf[offset + 2],
            self.buf[offset + 3],
            self.buf[offset + 4],
            self.buf[offset + 5],
            self.buf[offset + 6],
            self.buf[offset + 7],
        ]))
    }

    fn read_f32_le(&self, offset: usize) -> Result<f32, WalkError> {
        self.check_bounds(offset, 4)?;
        Ok(f32::from_le_bytes([
            self.buf[offset],
            self.buf[offset + 1],
            self.buf[offset + 2],
            self.buf[offset + 3],
        ]))
    }

    fn read_f64_le(&self, offset: usize) -> Result<f64, WalkError> {
        self.check_bounds(offset, 8)?;
        Ok(f64::from_le_bytes([
            self.buf[offset],
            self.buf[offset + 1],
            self.buf[offset + 2],
            self.buf[offset + 3],
            self.buf[offset + 4],
            self.buf[offset + 5],
            self.buf[offset + 6],
            self.buf[offset + 7],
        ]))
    }

    fn read_bytes(&self, offset: usize, len: usize) -> Result<&'a [u8], WalkError> {
        self.check_bounds(offset, len)?;
        Ok(&self.buf[offset..offset + len])
    }
}

// ---------------------------------------------------------------------------
// Binary walker
// ---------------------------------------------------------------------------

const MAX_DEPTH: usize = 64;

pub struct BinaryWalker<'a> {
    reader: BufReader<'a>,
    schema: &'a Schema,
    regions: Vec<AnnotatedRegion>,
    annotated: Vec<bool>,
    visited_tables: HashSet<usize>,
    visited_vtables: HashSet<usize>,
}

impl<'a> BinaryWalker<'a> {
    pub fn new(buf: &'a [u8], schema: &'a Schema) -> Self {
        let len = buf.len();
        Self {
            reader: BufReader::new(buf),
            schema,
            regions: Vec::new(),
            annotated: vec![false; len],
            visited_tables: HashSet::new(),
            visited_vtables: HashSet::new(),
        }
    }

    pub fn walk(mut self, root_type_name: &str) -> Result<Vec<AnnotatedRegion>, WalkError> {
        if self.reader.len() < 4 {
            return Err(WalkError::OutOfBounds {
                offset: 0,
                size: 4,
                buf_len: self.reader.len(),
            });
        }

        // 1. Root offset (bytes 0..4)
        let root_offset = self.reader.read_u32_le(0)? as usize;
        self.add_region(
            0..4,
            RegionType::RootOffset,
            "root_offset".to_string(),
            vec!["root".to_string()],
            format!("-> 0x{root_offset:04X}"),
            0,
        );

        // 2. File identifier (bytes 4..8) if present
        if self.schema.file_ident.is_some() && self.reader.len() >= 8 {
            let id_bytes = self.reader.read_bytes(4, 4)?;
            let id_str = String::from_utf8_lossy(id_bytes);
            self.add_region(
                4..8,
                RegionType::FileIdentifier,
                "file_identifier".to_string(),
                vec!["file_id".to_string()],
                format!("\"{id_str}\""),
                0,
            );
        }

        // 3. Find root table
        let root_obj_idx = self.find_object_index(root_type_name)?;

        // 4. Walk root table
        let path = vec![root_type_name.to_string()];
        self.walk_table(root_offset, root_obj_idx, path, 0)?;

        // 5. Fill padding
        self.fill_padding();

        Ok(self.regions)
    }

    fn find_object_index(&self, name: &str) -> Result<usize, WalkError> {
        // Try exact match first, then short name match
        for (i, obj) in self.schema.objects.iter().enumerate() {
            if let Some(ref obj_name) = obj.name {
                if obj_name == name {
                    return Ok(i);
                }
            }
        }
        // Try matching just the short name (without namespace)
        for (i, obj) in self.schema.objects.iter().enumerate() {
            if let Some(ref obj_name) = obj.name {
                let short = obj_name.rsplit('.').next().unwrap_or(obj_name);
                if short == name {
                    return Ok(i);
                }
            }
        }
        Err(WalkError::RootTypeNotFound {
            name: name.to_string(),
        })
    }

    fn get_object(&self, idx: usize) -> Result<&Object, WalkError> {
        self.schema
            .objects
            .get(idx)
            .ok_or(WalkError::ObjectIndexOutOfRange {
                index: idx,
                count: self.schema.objects.len(),
            })
    }

    fn get_enum(&self, idx: usize) -> Result<&Enum, WalkError> {
        self.schema
            .enums
            .get(idx)
            .ok_or(WalkError::EnumIndexOutOfRange {
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

    // -----------------------------------------------------------------------
    // Table walking
    // -----------------------------------------------------------------------

    fn walk_table(
        &mut self,
        table_offset: usize,
        obj_idx: usize,
        path: Vec<String>,
        depth: usize,
    ) -> Result<usize, WalkError> {
        if depth > MAX_DEPTH {
            return Err(WalkError::MaxDepthExceeded { max: MAX_DEPTH });
        }
        if self.visited_tables.contains(&table_offset) {
            return Ok(usize::MAX);
        }
        self.visited_tables.insert(table_offset);

        let obj = self.get_object(obj_idx)?;
        let type_name = Self::obj_name(obj);
        let fields: Vec<Field> = obj.fields.clone();

        // Read SOffset32 pointing to vtable
        let vtable_soffset = self.reader.read_i32_le(table_offset)?;
        let vtable_offset = (table_offset as i64 - vtable_soffset as i64) as usize;

        let table_region = self.add_region(
            table_offset..table_offset + 4,
            RegionType::TableSOffset {
                type_name: type_name.clone(),
            },
            format!("{type_name} (soffset to vtable)"),
            path.clone(),
            format!("soffset={vtable_soffset} -> vtable@0x{vtable_offset:04X}"),
            depth,
        );

        // Walk vtable if not already visited
        let vtable_region = if !self.visited_vtables.contains(&vtable_offset) {
            Some(self.walk_vtable(vtable_offset, &fields, &type_name, &path, depth)?)
        } else {
            None
        };

        // Link table <-> vtable
        if let Some(vt_idx) = vtable_region {
            self.regions[table_region].related_regions.push(vt_idx);
            self.regions[vt_idx].related_regions.push(table_region);
        }

        // Read vtable to determine field presence
        let vtable_size = self.reader.read_u16_le(vtable_offset)? as usize;

        // Two-pass: first collect union type discriminants, then walk all fields
        let mut union_type_values: std::collections::HashMap<String, u8> =
            std::collections::HashMap::new();

        // First pass: read _type fields (BASE_TYPE_U_TYPE)
        for field in &fields {
            let ty = match field.type_.as_ref() {
                Some(t) => t,
                None => continue,
            };
            let bt = ty.base_type.unwrap_or(BaseType::BASE_TYPE_NONE);
            if bt != BaseType::BASE_TYPE_U_TYPE {
                continue;
            }

            let field_id = field.id.unwrap_or(0) as usize;
            let vtable_entry_offset = vtable_offset + 4 + field_id * 2;
            if vtable_entry_offset + 2 > vtable_offset + vtable_size {
                continue;
            }

            let field_offset_in_table = self.reader.read_u16_le(vtable_entry_offset)? as usize;
            if field_offset_in_table == 0 {
                continue;
            }

            let field_data_offset = table_offset + field_offset_in_table;
            let val = self.reader.read_u8(field_data_offset)?;
            let fname = Self::field_name(field);
            // Strip _type suffix to get the union field name
            let union_name = fname.strip_suffix("_type").unwrap_or(&fname).to_string();
            union_type_values.insert(union_name, val);
        }

        // Second pass: walk all fields
        for field in &fields {
            let ty = match field.type_.as_ref() {
                Some(t) => t,
                None => continue,
            };
            let bt = ty.base_type.unwrap_or(BaseType::BASE_TYPE_NONE);

            let field_id = field.id.unwrap_or(0) as usize;
            let vtable_entry_offset = vtable_offset + 4 + field_id * 2;
            if vtable_entry_offset + 2 > vtable_offset + vtable_size {
                continue;
            }

            let field_offset_in_table = self.reader.read_u16_le(vtable_entry_offset)? as usize;
            if field_offset_in_table == 0 {
                continue; // Field not present (uses default)
            }

            let field_data_offset = table_offset + field_offset_in_table;
            let mut field_path = path.clone();
            let fname = Self::field_name(field);
            field_path.push(fname.clone());

            self.walk_field(
                field_data_offset,
                field,
                ty,
                bt,
                &field_path,
                &fname,
                depth + 1,
                &union_type_values,
                table_region,
            )?;
        }

        Ok(table_region)
    }

    // -----------------------------------------------------------------------
    // VTable walking
    // -----------------------------------------------------------------------

    fn walk_vtable(
        &mut self,
        vtable_offset: usize,
        fields: &[Field],
        type_name: &str,
        path: &[String],
        depth: usize,
    ) -> Result<usize, WalkError> {
        self.visited_vtables.insert(vtable_offset);

        let vtable_size = self.reader.read_u16_le(vtable_offset)? as usize;
        let table_data_size = self.reader.read_u16_le(vtable_offset + 2)?;

        let vt_region = self.add_region(
            vtable_offset..vtable_offset + vtable_size,
            RegionType::VTable {
                type_name: type_name.to_string(),
            },
            format!("vtable for {type_name}"),
            path.to_vec(),
            String::new(),
            depth,
        );

        let size_region = self.add_region(
            vtable_offset..vtable_offset + 2,
            RegionType::VTableSize,
            "vtable_size".to_string(),
            path.to_vec(),
            format!("{vtable_size}"),
            depth + 1,
        );

        let tsize_region = self.add_region(
            vtable_offset + 2..vtable_offset + 4,
            RegionType::VTableTableSize,
            "table_data_size".to_string(),
            path.to_vec(),
            format!("{table_data_size}"),
            depth + 1,
        );

        self.regions[vt_region].children.push(size_region);
        self.regions[vt_region].children.push(tsize_region);

        let num_entries = (vtable_size.saturating_sub(4)) / 2;
        for i in 0..num_entries {
            let entry_offset = vtable_offset + 4 + i * 2;
            if entry_offset + 2 > vtable_offset + vtable_size {
                break;
            }
            let field_offset_val = self.reader.read_u16_le(entry_offset)?;

            let field_name = fields
                .iter()
                .find(|f| f.id == Some(i as u32))
                .and_then(|f| f.name.as_deref())
                .unwrap_or("?")
                .to_string();

            let entry_region = self.add_region(
                entry_offset..entry_offset + 2,
                RegionType::VTableEntry {
                    field_name: field_name.clone(),
                    field_id: i as u32,
                },
                format!("field[{i}] \"{field_name}\""),
                path.to_vec(),
                if field_offset_val == 0 {
                    "absent".to_string()
                } else {
                    format!("offset={field_offset_val}")
                },
                depth + 1,
            );

            self.regions[vt_region].children.push(entry_region);
        }

        Ok(vt_region)
    }

    // -----------------------------------------------------------------------
    // Field walking
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn walk_field(
        &mut self,
        offset: usize,
        _field: &Field,
        ty: &Type,
        bt: BaseType,
        path: &[String],
        fname: &str,
        depth: usize,
        union_type_values: &std::collections::HashMap<String, u8>,
        parent_region: usize,
    ) -> Result<(), WalkError> {
        match bt {
            // Scalars
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
                let size = scalar_byte_size(bt);
                let value = self.read_scalar_display(offset, bt, ty)?;
                let region = self.add_region(
                    offset..offset + size,
                    RegionType::ScalarField {
                        field_name: fname.to_string(),
                        base_type: bt,
                    },
                    fname.to_string(),
                    path.to_vec(),
                    value,
                    depth,
                );
                self.regions[parent_region].children.push(region);
            }

            // UType (union discriminant)
            BaseType::BASE_TYPE_U_TYPE => {
                let val = self.reader.read_u8(offset)?;
                let region = self.add_region(
                    offset..offset + 1,
                    RegionType::UnionTypeField {
                        field_name: fname.to_string(),
                    },
                    fname.to_string(),
                    path.to_vec(),
                    format!("{val}"),
                    depth,
                );
                self.regions[parent_region].children.push(region);
            }

            // String
            BaseType::BASE_TYPE_STRING => {
                let str_uoffset = self.reader.read_u32_le(offset)? as usize;
                let str_start = offset + str_uoffset;
                let off_region = self.add_region(
                    offset..offset + 4,
                    RegionType::StringOffset {
                        field_name: fname.to_string(),
                    },
                    format!("{fname} (string offset)"),
                    path.to_vec(),
                    format!("-> 0x{str_start:04X}"),
                    depth,
                );
                self.regions[parent_region].children.push(off_region);
                self.walk_string(str_start, path, depth, off_region)?;
            }

            // Table (referenced by offset)
            BaseType::BASE_TYPE_TABLE => {
                let table_uoffset = self.reader.read_u32_le(offset)? as usize;
                let table_start = offset + table_uoffset;
                let obj_idx = ty.index.unwrap_or(0) as usize;

                let off_region = self.add_region(
                    offset..offset + 4,
                    RegionType::ScalarField {
                        field_name: fname.to_string(),
                        base_type: BaseType::BASE_TYPE_U_INT,
                    },
                    format!("{fname} (table offset)"),
                    path.to_vec(),
                    format!("-> 0x{table_start:04X}"),
                    depth,
                );
                self.regions[parent_region].children.push(off_region);

                let nested_region = self.walk_table(table_start, obj_idx, path.to_vec(), depth)?;
                if nested_region != usize::MAX {
                    self.regions[off_region].children.push(nested_region);
                }
            }

            // Struct (inline)
            BaseType::BASE_TYPE_STRUCT => {
                let obj_idx = ty.index.unwrap_or(0) as usize;
                self.walk_struct_inline(offset, obj_idx, path, depth, parent_region)?;
            }

            // Vector
            BaseType::BASE_TYPE_VECTOR => {
                let vec_uoffset = self.reader.read_u32_le(offset)? as usize;
                let vec_start = offset + vec_uoffset;

                let off_region = self.add_region(
                    offset..offset + 4,
                    RegionType::VectorOffset {
                        field_name: fname.to_string(),
                    },
                    format!("{fname} (vector offset)"),
                    path.to_vec(),
                    format!("-> 0x{vec_start:04X}"),
                    depth,
                );
                self.regions[parent_region].children.push(off_region);

                self.walk_vector(vec_start, ty, path, depth, off_region)?;
            }

            // Union data
            BaseType::BASE_TYPE_UNION => {
                let data_uoffset = self.reader.read_u32_le(offset)? as usize;
                let data_start = offset + data_uoffset;

                let off_region = self.add_region(
                    offset..offset + 4,
                    RegionType::UnionDataOffset {
                        field_name: fname.to_string(),
                    },
                    format!("{fname} (union offset)"),
                    path.to_vec(),
                    format!("-> 0x{data_start:04X}"),
                    depth,
                );
                self.regions[parent_region].children.push(off_region);

                // Resolve union variant using companion _type value
                if let Some(&discriminant) = union_type_values.get(fname) {
                    self.walk_union_data(data_start, ty, discriminant, path, depth, off_region)?;
                }
            }

            _ => {
                // Unknown field type -- annotate raw bytes
                let region = self.add_region(
                    offset..offset + 4,
                    RegionType::Unknown,
                    format!("{fname} (unknown type)"),
                    path.to_vec(),
                    format!("{bt:?}"),
                    depth,
                );
                self.regions[parent_region].children.push(region);
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Struct walking (inline, fixed layout)
    // -----------------------------------------------------------------------

    fn walk_struct_inline(
        &mut self,
        offset: usize,
        obj_idx: usize,
        path: &[String],
        depth: usize,
        parent_region: usize,
    ) -> Result<(), WalkError> {
        if depth > MAX_DEPTH {
            return Err(WalkError::MaxDepthExceeded { max: MAX_DEPTH });
        }

        let obj = self.get_object(obj_idx)?;
        let type_name = Self::obj_name(obj);
        let byte_size = obj.byte_size.unwrap_or(0) as usize;
        let fields: Vec<Field> = obj.fields.clone();

        let struct_region = self.add_region(
            offset..offset + byte_size,
            RegionType::StructInline {
                type_name: type_name.clone(),
            },
            type_name,
            path.to_vec(),
            format!("{byte_size} bytes"),
            depth,
        );
        self.regions[parent_region].children.push(struct_region);

        for field in &fields {
            let field_ty = match field.type_.as_ref() {
                Some(t) => t,
                None => continue,
            };
            let field_bt = field_ty.base_type.unwrap_or(BaseType::BASE_TYPE_NONE);
            let field_offset = offset + field.offset.unwrap_or(0) as usize;
            let fname = Self::field_name(field);

            let mut field_path = path.to_vec();
            field_path.push(fname.clone());

            match field_bt {
                BaseType::BASE_TYPE_STRUCT => {
                    let inner_idx = field_ty.index.unwrap_or(0) as usize;
                    self.walk_struct_inline(
                        field_offset,
                        inner_idx,
                        &field_path,
                        depth + 1,
                        struct_region,
                    )?;
                }
                BaseType::BASE_TYPE_ARRAY => {
                    self.walk_fixed_array(
                        field_offset,
                        field_ty,
                        &field_path,
                        &fname,
                        depth + 1,
                        struct_region,
                    )?;
                }
                _ => {
                    let size = scalar_byte_size(field_bt);
                    if size > 0 {
                        let value = self.read_scalar_display(field_offset, field_bt, field_ty)?;
                        let region = self.add_region(
                            field_offset..field_offset + size,
                            RegionType::StructField {
                                field_name: fname,
                                base_type: field_bt,
                            },
                            Self::field_name(field),
                            field_path,
                            value,
                            depth + 1,
                        );
                        self.regions[struct_region].children.push(region);
                    }
                }
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Fixed-length array walking (struct-only)
    // -----------------------------------------------------------------------

    fn walk_fixed_array(
        &mut self,
        offset: usize,
        ty: &Type,
        path: &[String],
        fname: &str,
        depth: usize,
        parent_region: usize,
    ) -> Result<(), WalkError> {
        let elem_bt = ty.element_type.unwrap_or(BaseType::BASE_TYPE_U_BYTE);
        let fixed_len = ty.fixed_length.unwrap_or(0) as usize;

        match elem_bt {
            bt if is_scalar(bt) => {
                let elem_size = scalar_byte_size(bt);
                let total = elem_size * fixed_len;
                let arr_region = self.add_region(
                    offset..offset + total,
                    RegionType::VectorOffset {
                        field_name: fname.to_string(),
                    },
                    format!("{fname}[{fixed_len}]"),
                    path.to_vec(),
                    format!("{fixed_len} x {bt:?}"),
                    depth,
                );
                self.regions[parent_region].children.push(arr_region);

                for i in 0..fixed_len {
                    let elem_offset = offset + i * elem_size;
                    let value = self.read_scalar_display(elem_offset, bt, ty)?;
                    let elem_region = self.add_region(
                        elem_offset..elem_offset + elem_size,
                        RegionType::VectorElement { index: i },
                        format!("[{i}]"),
                        path.to_vec(),
                        value,
                        depth + 1,
                    );
                    self.regions[arr_region].children.push(elem_region);
                }
            }
            BaseType::BASE_TYPE_STRUCT => {
                let inner_idx = ty.index.unwrap_or(0) as usize;
                let inner_obj = self.get_object(inner_idx)?;
                let struct_size = inner_obj.byte_size.unwrap_or(0) as usize;
                let total = struct_size * fixed_len;

                let arr_region = self.add_region(
                    offset..offset + total,
                    RegionType::VectorOffset {
                        field_name: fname.to_string(),
                    },
                    format!("{fname}[{fixed_len}]"),
                    path.to_vec(),
                    format!("{fixed_len} structs"),
                    depth,
                );
                self.regions[parent_region].children.push(arr_region);

                for i in 0..fixed_len {
                    let elem_offset = offset + i * struct_size;
                    let mut elem_path = path.to_vec();
                    elem_path.push(format!("[{i}]"));
                    self.walk_struct_inline(
                        elem_offset,
                        inner_idx,
                        &elem_path,
                        depth + 1,
                        arr_region,
                    )?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Vector walking
    // -----------------------------------------------------------------------

    fn walk_vector(
        &mut self,
        vec_start: usize,
        ty: &Type,
        path: &[String],
        depth: usize,
        parent_region: usize,
    ) -> Result<(), WalkError> {
        let count = self.reader.read_u32_le(vec_start)? as usize;
        let len_region = self.add_region(
            vec_start..vec_start + 4,
            RegionType::VectorLength,
            "length".to_string(),
            path.to_vec(),
            format!("{count}"),
            depth + 1,
        );
        self.regions[parent_region].children.push(len_region);

        let elem_bt = ty.element_type.unwrap_or(BaseType::BASE_TYPE_U_BYTE);
        let data_start = vec_start + 4;

        match elem_bt {
            bt if is_scalar(bt) => {
                let elem_size = scalar_byte_size(bt);
                for i in 0..count {
                    let elem_offset = data_start + i * elem_size;
                    let value = self.read_scalar_display(elem_offset, bt, ty)?;
                    let elem_region = self.add_region(
                        elem_offset..elem_offset + elem_size,
                        RegionType::VectorElement { index: i },
                        format!("[{i}]"),
                        path.to_vec(),
                        value,
                        depth + 1,
                    );
                    self.regions[parent_region].children.push(elem_region);
                }
            }

            BaseType::BASE_TYPE_STRING => {
                for i in 0..count {
                    let elem_offset = data_start + i * 4;
                    let str_uoffset = self.reader.read_u32_le(elem_offset)? as usize;
                    let str_start = elem_offset + str_uoffset;

                    let elem_region = self.add_region(
                        elem_offset..elem_offset + 4,
                        RegionType::VectorElement { index: i },
                        format!("[{i}] (string offset)"),
                        path.to_vec(),
                        format!("-> 0x{str_start:04X}"),
                        depth + 1,
                    );
                    self.regions[parent_region].children.push(elem_region);

                    self.walk_string(str_start, path, depth + 1, elem_region)?;
                }
            }

            BaseType::BASE_TYPE_TABLE => {
                let obj_idx = ty.index.unwrap_or(0) as usize;
                for i in 0..count {
                    let elem_offset = data_start + i * 4;
                    let table_uoffset = self.reader.read_u32_le(elem_offset)? as usize;
                    let table_start = elem_offset + table_uoffset;

                    let elem_region = self.add_region(
                        elem_offset..elem_offset + 4,
                        RegionType::VectorElement { index: i },
                        format!("[{i}] (table offset)"),
                        path.to_vec(),
                        format!("-> 0x{table_start:04X}"),
                        depth + 1,
                    );
                    self.regions[parent_region].children.push(elem_region);

                    let mut elem_path = path.to_vec();
                    elem_path.push(format!("[{i}]"));
                    let nested_region =
                        self.walk_table(table_start, obj_idx, elem_path, depth + 1)?;
                    if nested_region != usize::MAX {
                        self.regions[elem_region].children.push(nested_region);
                    }
                }
            }

            BaseType::BASE_TYPE_STRUCT => {
                let obj_idx = ty.index.unwrap_or(0) as usize;
                let obj = self.get_object(obj_idx)?;
                let struct_size = obj.byte_size.unwrap_or(0) as usize;
                for i in 0..count {
                    let elem_offset = data_start + i * struct_size;
                    let elem_region = self.add_region(
                        elem_offset..elem_offset + struct_size,
                        RegionType::VectorElement { index: i },
                        format!("[{i}]"),
                        path.to_vec(),
                        String::new(),
                        depth + 1,
                    );
                    self.regions[parent_region].children.push(elem_region);

                    let mut elem_path = path.to_vec();
                    elem_path.push(format!("[{i}]"));
                    self.walk_struct_inline(
                        elem_offset,
                        obj_idx,
                        &elem_path,
                        depth + 1,
                        elem_region,
                    )?;
                }
            }

            _ => {}
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // String walking
    // -----------------------------------------------------------------------

    fn walk_string(
        &mut self,
        str_start: usize,
        path: &[String],
        depth: usize,
        parent_region: usize,
    ) -> Result<(), WalkError> {
        let length = self.reader.read_u32_le(str_start)? as usize;

        let len_region = self.add_region(
            str_start..str_start + 4,
            RegionType::StringLength,
            "string_length".to_string(),
            path.to_vec(),
            format!("{length}"),
            depth + 1,
        );
        self.regions[parent_region].children.push(len_region);

        if length > 0 {
            let data_bytes = self.reader.read_bytes(str_start + 4, length)?;
            let text = String::from_utf8_lossy(data_bytes);
            let display = if text.len() > 40 {
                format!("\"{}...\"", &text[..37])
            } else {
                format!("\"{text}\"")
            };

            let data_region = self.add_region(
                str_start + 4..str_start + 4 + length,
                RegionType::StringData {
                    field_name: path.last().cloned().unwrap_or_default(),
                },
                "string_data".to_string(),
                path.to_vec(),
                display,
                depth + 1,
            );
            self.regions[parent_region].children.push(data_region);
        }

        // Null terminator
        if str_start + 4 + length < self.reader.len() {
            let term_region = self.add_region(
                str_start + 4 + length..str_start + 4 + length + 1,
                RegionType::StringTerminator,
                "null".to_string(),
                path.to_vec(),
                "\\0".to_string(),
                depth + 1,
            );
            self.regions[parent_region].children.push(term_region);
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Union data walking
    // -----------------------------------------------------------------------

    fn walk_union_data(
        &mut self,
        offset: usize,
        ty: &Type,
        discriminant: u8,
        path: &[String],
        depth: usize,
        parent_region: usize,
    ) -> Result<(), WalkError> {
        if discriminant == 0 {
            return Ok(()); // NONE variant
        }

        let enum_idx = ty.index.unwrap_or(0) as usize;
        let enum_def = self.get_enum(enum_idx)?;

        // Find the enum value matching the discriminant
        let variant = enum_def
            .values
            .iter()
            .find(|v| v.value == Some(discriminant as i64));

        if let Some(variant) = variant {
            if let Some(ref union_type) = variant.union_type {
                let variant_bt = union_type.base_type.unwrap_or(BaseType::BASE_TYPE_TABLE);
                let variant_idx = union_type.index.unwrap_or(0) as usize;

                match variant_bt {
                    BaseType::BASE_TYPE_TABLE => {
                        let mut variant_path = path.to_vec();
                        if let Some(ref vname) = variant.name {
                            variant_path.push(vname.clone());
                        }
                        let table_region =
                            self.walk_table(offset, variant_idx, variant_path, depth + 1)?;
                        self.regions[parent_region].children.push(table_region);
                    }
                    BaseType::BASE_TYPE_STRUCT => {
                        self.walk_struct_inline(
                            offset,
                            variant_idx,
                            path,
                            depth + 1,
                            parent_region,
                        )?;
                    }
                    BaseType::BASE_TYPE_STRING => {
                        self.walk_string(offset, path, depth, parent_region)?;
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn add_region(
        &mut self,
        byte_range: Range<usize>,
        region_type: RegionType,
        label: String,
        field_path: Vec<String>,
        value_display: String,
        depth: usize,
    ) -> usize {
        // Mark bytes as annotated
        for i in byte_range.clone() {
            if i < self.annotated.len() {
                self.annotated[i] = true;
            }
        }

        let idx = self.regions.len();
        self.regions.push(AnnotatedRegion {
            byte_range,
            region_type,
            label,
            field_path,
            value_display,
            children: Vec::new(),
            related_regions: Vec::new(),
            depth,
        });
        idx
    }

    fn fill_padding(&mut self) {
        let mut i = 0;
        while i < self.annotated.len() {
            if !self.annotated[i] {
                let start = i;
                while i < self.annotated.len() && !self.annotated[i] {
                    i += 1;
                }
                self.regions.push(AnnotatedRegion {
                    byte_range: start..i,
                    region_type: RegionType::Padding,
                    label: "padding".to_string(),
                    field_path: vec![],
                    value_display: format!("{} bytes", i - start),
                    children: Vec::new(),
                    related_regions: Vec::new(),
                    depth: 0,
                });
            } else {
                i += 1;
            }
        }
    }

    fn read_scalar_display(
        &self,
        offset: usize,
        bt: BaseType,
        ty: &Type,
    ) -> Result<String, WalkError> {
        let display = match bt {
            BaseType::BASE_TYPE_BOOL => {
                let v = self.reader.read_u8(offset)?;
                format!("{} (bool)", v != 0)
            }
            BaseType::BASE_TYPE_BYTE => {
                let v = self.reader.read_i8(offset)?;
                format!("{v} (i8)")
            }
            BaseType::BASE_TYPE_U_BYTE => {
                let v = self.reader.read_u8(offset)?;
                format!("{v} (u8)")
            }
            BaseType::BASE_TYPE_SHORT => {
                let v = self.reader.read_i16_le(offset)?;
                format!("{v} (i16)")
            }
            BaseType::BASE_TYPE_U_SHORT => {
                let v = self.reader.read_u16_le(offset)?;
                format!("{v} (u16)")
            }
            BaseType::BASE_TYPE_INT => {
                let v = self.reader.read_i32_le(offset)?;
                // Check if this is an enum
                if let Some(idx) = ty.index {
                    if idx >= 0 && (idx as usize) < self.schema.enums.len() {
                        let e = &self.schema.enums[idx as usize];
                        if let Some(val) = e.values.iter().find(|ev| ev.value == Some(v as i64)) {
                            if let Some(ref name) = val.name {
                                return Ok(format!("{name} ({v})"));
                            }
                        }
                    }
                }
                format!("{v} (i32)")
            }
            BaseType::BASE_TYPE_U_INT => {
                let v = self.reader.read_u32_le(offset)?;
                format!("{v} (u32)")
            }
            BaseType::BASE_TYPE_LONG => {
                let v = self.reader.read_i64_le(offset)?;
                format!("{v} (i64)")
            }
            BaseType::BASE_TYPE_U_LONG => {
                let v = self.reader.read_u64_le(offset)?;
                format!("{v} (u64)")
            }
            BaseType::BASE_TYPE_FLOAT => {
                let v = self.reader.read_f32_le(offset)?;
                format!("{v} (f32)")
            }
            BaseType::BASE_TYPE_DOUBLE => {
                let v = self.reader.read_f64_le(offset)?;
                format!("{v} (f64)")
            }
            BaseType::BASE_TYPE_U_TYPE => {
                let v = self.reader.read_u8(offset)?;
                format!("{v} (utype)")
            }
            _ => format!("({bt:?})"),
        };
        Ok(display)
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

pub fn scalar_byte_size(bt: BaseType) -> usize {
    match bt {
        BaseType::BASE_TYPE_BOOL
        | BaseType::BASE_TYPE_BYTE
        | BaseType::BASE_TYPE_U_BYTE
        | BaseType::BASE_TYPE_U_TYPE => 1,
        BaseType::BASE_TYPE_SHORT | BaseType::BASE_TYPE_U_SHORT => 2,
        BaseType::BASE_TYPE_INT | BaseType::BASE_TYPE_U_INT | BaseType::BASE_TYPE_FLOAT => 4,
        BaseType::BASE_TYPE_LONG | BaseType::BASE_TYPE_U_LONG | BaseType::BASE_TYPE_DOUBLE => 8,
        _ => 0,
    }
}

pub fn is_scalar(bt: BaseType) -> bool {
    scalar_byte_size(bt) > 0
}
