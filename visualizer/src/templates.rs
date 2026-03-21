pub struct Template {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: &'static str,
    pub json_data: &'static str,
}

pub fn all() -> &'static [Template] {
    &[
        MONSTER,
        SIMPLE_SCALARS,
        NESTED_STRUCTS,
        STRING_FIELDS,
        NESTED_TABLES,
        UNION,
        VECTOR_OF_TABLES,
        VECTOR_OF_STRINGS,
        ALL_SCALAR_TYPES,
        DEFAULT_VALUES,
        VECTOR_OF_STRUCTS,
        FILE_IDENTIFIER,
    ]
}

// ---------------------------------------------------------------------------
// Template 1: Monster (existing demo)
// Features: struct, enum, string, vector of scalars
// ---------------------------------------------------------------------------

const MONSTER: Template = Template {
    name: "Monster",
    description: "Struct, enum, string, vector of scalars",
    schema: r#"namespace MyGame;

enum Color : byte { Red = 1, Green, Blue }

struct Vec3 {
  x: float;
  y: float;
  z: float;
}

table Monster {
  pos: Vec3;
  mana: short = 150;
  hp: short = 100;
  name: string;
  color: Color = Blue;
  inventory: [ubyte];
}

root_type Monster;
"#,
    json_data: r#"{
  "pos": { "x": 1.0, "y": 2.0, "z": 3.0 },
  "mana": 200,
  "hp": 300,
  "name": "Orc",
  "color": "Red",
  "inventory": [0, 1, 2, 3, 4]
}"#,
};

// ---------------------------------------------------------------------------
// Template 2: Simple Scalars
// Features: bool, int, float -- minimal table
// ---------------------------------------------------------------------------

const SIMPLE_SCALARS: Template = Template {
    name: "Simple Scalars",
    description: "Minimal table with bool, int, float",
    schema: r#"table Config {
  debug: bool;
  volume: int;
  brightness: float;
}

root_type Config;
"#,
    json_data: r#"{
  "debug": true,
  "volume": 75,
  "brightness": 0.8
}"#,
};

// ---------------------------------------------------------------------------
// Template 3: Nested Structs
// Features: nested structs (Vec2 inside Rect), string, float
// ---------------------------------------------------------------------------

const NESTED_STRUCTS: Template = Template {
    name: "Nested Structs",
    description: "Nested structs (Vec2 in Rect), string, float",
    schema: r#"struct Vec2 {
  x: float;
  y: float;
}

struct Rect {
  origin: Vec2;
  size: Vec2;
}

table UIElement {
  name: string;
  bounds: Rect;
  opacity: float;
}

root_type UIElement;
"#,
    json_data: r#"{
  "name": "Button",
  "bounds": {
    "origin": { "x": 10.0, "y": 20.0 },
    "size": { "x": 200.0, "y": 50.0 }
  },
  "opacity": 0.9
}"#,
};

// ---------------------------------------------------------------------------
// Template 4: String Fields
// Features: multiple strings showing offset chaining
// ---------------------------------------------------------------------------

const STRING_FIELDS: Template = Template {
    name: "String Fields",
    description: "Multiple strings showing offset chaining",
    schema: r#"table UserProfile {
  username: string;
  email: string;
  bio: string;
  age: int;
}

root_type UserProfile;
"#,
    json_data: r#"{
  "username": "alice",
  "email": "alice@example.com",
  "bio": "Hello, world!",
  "age": 30
}"#,
};

// ---------------------------------------------------------------------------
// Template 5: Nested Tables
// Features: deeply nested tables (table -> table -> table), mixed scalars/strings
// ---------------------------------------------------------------------------

const NESTED_TABLES: Template = Template {
    name: "Nested Tables",
    description: "Deeply nested tables (3 levels), strings at each level",
    schema: r#"table Address {
  street: string;
  city: string;
  zip: int;
}

table ContactInfo {
  email: string;
  phone: string;
  address: Address;
}

table Employee {
  name: string;
  age: int;
  contact: ContactInfo;
}

root_type Employee;
"#,
    json_data: r#"{
  "name": "Alice",
  "age": 30,
  "contact": {
    "email": "alice@example.com",
    "phone": "555-1234",
    "address": {
      "street": "123 Main St",
      "city": "Springfield",
      "zip": 62701
    }
  }
}"#,
};

// ---------------------------------------------------------------------------
// Template 6: Union
// Features: union type with table variants, discriminant byte + data offset
// ---------------------------------------------------------------------------

const UNION: Template = Template {
    name: "Union",
    description: "Union type with multiple table variants",
    schema: r#"table Sword {
  damage: int;
  name: string;
}

table Shield {
  armor: int;
  weight: float;
}

union Equipment { Sword, Shield }

table Hero {
  name: string;
  equipped: Equipment;
}

root_type Hero;
"#,
    json_data: r#"{
  "name": "Knight",
  "equipped_type": "Sword",
  "equipped": {
    "damage": 50,
    "name": "Excalibur"
  }
}"#,
};

// ---------------------------------------------------------------------------
// Template 7: Vector of Tables
// Features: [Table] showing offset array, each pointing to separate tables
// ---------------------------------------------------------------------------

const VECTOR_OF_TABLES: Template = Template {
    name: "Vector of Tables",
    description: "Vector of tables showing per-element offsets",
    schema: r#"table Item {
  name: string;
  quantity: int;
}

table Inventory {
  items: [Item];
  owner: string;
}

root_type Inventory;
"#,
    json_data: r#"{
  "items": [
    { "name": "Potion", "quantity": 5 },
    { "name": "Arrow", "quantity": 20 },
    { "name": "Shield", "quantity": 1 }
  ],
  "owner": "Adventurer"
}"#,
};

// ---------------------------------------------------------------------------
// Template 8: Vector of Strings
// Features: [string] showing vector of offsets to length-prefixed strings
// ---------------------------------------------------------------------------

const VECTOR_OF_STRINGS: Template = Template {
    name: "Vector of Strings",
    description: "Vector of strings showing offset-per-element pattern",
    schema: r#"table TagList {
  title: string;
  tags: [string];
  count: int;
}

root_type TagList;
"#,
    json_data: r#"{
  "title": "Languages",
  "tags": ["Rust", "TypeScript", "Go", "Python"],
  "count": 4
}"#,
};

// ---------------------------------------------------------------------------
// Template 9: All Scalar Types
// Features: every scalar type (1/2/4/8-byte) showing alignment and padding
// ---------------------------------------------------------------------------

const ALL_SCALAR_TYPES: Template = Template {
    name: "All Scalar Types",
    description: "Every scalar type: bool, byte..ulong, float, double",
    schema: r#"table AllScalars {
  f_bool: bool;
  f_byte: byte;
  f_ubyte: ubyte;
  f_short: short;
  f_ushort: ushort;
  f_int: int;
  f_uint: uint;
  f_long: long;
  f_ulong: ulong;
  f_float: float;
  f_double: double;
}

root_type AllScalars;
"#,
    json_data: r#"{
  "f_bool": true,
  "f_byte": -42,
  "f_ubyte": 255,
  "f_short": -1000,
  "f_ushort": 65535,
  "f_int": -100000,
  "f_uint": 4000000000,
  "f_long": -9000000000000,
  "f_ulong": 18000000000000000000,
  "f_float": 3.14,
  "f_double": 2.718281828
}"#,
};

// ---------------------------------------------------------------------------
// Template 10: Default Values
// Features: fields with non-default values alongside defaulted (omitted) fields
// ---------------------------------------------------------------------------

const DEFAULT_VALUES: Template = Template {
    name: "Default Values",
    description: "Defaults vs. explicit values, vtable field omission",
    schema: r#"table Settings {
  width: int = 800;
  height: int = 600;
  fullscreen: bool = false;
  volume: float = 0.5;
  title: string;
  fps_limit: int = 60;
}

root_type Settings;
"#,
    json_data: r#"{
  "width": 1920,
  "height": 1080,
  "fullscreen": true,
  "volume": 0.5,
  "title": "My Game",
  "fps_limit": 60
}"#,
};

// ---------------------------------------------------------------------------
// Template 11: Vector of Structs
// Features: [Struct] showing contiguous inline data (no per-element offsets)
// ---------------------------------------------------------------------------

const VECTOR_OF_STRUCTS: Template = Template {
    name: "Vector of Structs",
    description: "Vector of structs: contiguous inline data, no offsets",
    schema: r#"struct Point {
  x: float;
  y: float;
}

table Path {
  name: string;
  points: [Point];
  closed: bool;
}

root_type Path;
"#,
    json_data: r#"{
  "name": "Triangle",
  "points": [
    { "x": 0.0, "y": 0.0 },
    { "x": 100.0, "y": 0.0 },
    { "x": 50.0, "y": 86.6 }
  ],
  "closed": true
}"#,
};

// ---------------------------------------------------------------------------
// Template 12: File Identifier
// Features: file_identifier showing 4-byte magic at bytes 4-7
// ---------------------------------------------------------------------------

const FILE_IDENTIFIER: Template = Template {
    name: "File Identifier",
    description: "File identifier (4-byte magic) at bytes 4-7",
    schema: r#"table Document {
  version: int;
  title: string;
  page_count: int;
}

root_type Document;
file_identifier "DOCS";
"#,
    json_data: r#"{
  "version": 3,
  "title": "FlatBuffers Guide",
  "page_count": 42
}"#,
};
