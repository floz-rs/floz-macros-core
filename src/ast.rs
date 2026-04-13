//! AST types for the parsed `schema!` DSL.
//!
//! These types represent the internal structure that the parser produces
//! and the code generator consumes.
//!
//! Includes validation rules parsed from `#[col(...)]` attributes.

use proc_macro2::Ident;

/// A single model declaration.
///
/// ```ignore
/// model User("users") {
///     id: integer("id").auto_increment().primary(),
///     ...
///     @primary_key(id),
/// }
/// ```
#[derive(Debug)]
pub struct ModelDef {
    /// The Rust struct name (e.g., `User`)
    pub name: Ident,
    /// The PostgreSQL table name (e.g., `"users"`)
    pub table_name: String,
    /// Database columns — generate struct fields + Column constants
    pub db_columns: Vec<FieldDef>,
    /// Relationships — only generate fetch_/add_/remove_ methods
    pub relationships: Vec<RelDef>,
    /// Table-level constraints (@primary_key, @unique, @index)
    pub constraints: Vec<TableConstraint>,
    /// If true, user provides custom `impl FlozHooks` (don't generate default)
    pub has_custom_hooks: bool,
    /// If true, this model supports soft deletes natively.
    pub soft_delete: bool,
}

/// A database column field definition.
///
/// ```ignore
/// name: varchar("name", 100).nullable().unique(),
/// ```
#[derive(Debug)]
pub struct FieldDef {
    /// The Rust field name (e.g., `name`)
    pub rust_name: Ident,
    /// The database column name (e.g., `"name"`)
    pub column_name: String,
    /// The column type
    pub type_info: TypeInfo,
    /// Chained modifiers
    pub modifiers: Vec<Modifier>,
    /// Validation rules parsed from `#[col(len(...), email, ...)]`
    pub validations: Vec<ValidationRule>,
}

/// A relationship declaration.
///
/// ```ignore
/// posts: array(Post, "author_id"),
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum RelationType {
    HasMany,
    BelongsTo,
    ManyToMany { through: String },
}

#[derive(Debug)]
pub struct RelDef {
    /// The Rust field name used for the relationship accessor
    pub rust_name: Ident,
    /// The target model path (e.g., `Post` or `crate::app::post::Post`)
    pub target_model: syn::Path,
    /// The foreign key column name in the target table (for HasMany/BelongsTo)
    pub fk_column: String,
    /// The logical relationship type
    pub relation_type: RelationType,
}

/// Column type information from the type function.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeInfo {
    // Numeric
    Integer,
    Short,
    BigInt,
    Real,
    Double,
    Decimal { precision: u32, scale: u32 },

    // String
    Varchar { max_length: u32 },
    Text,

    // Other scalars
    Bool,
    Date,
    Time,
    DateTime,
    Uuid,
    Binary,

    // JSON types
    Json,
    Jsonb,

    // Enum type
    Enum { rust_type: String },

    // Postgres Extension Types
    Ltree,

    // Generic escape hatch: col(Type, "column")
    Col { rust_type: String },

    // Native PostgreSQL array types
    TextArray,
    IntArray,
    BigIntArray,
    UuidArray,
    BoolArray,
    RealArray,
    DoubleArray,
    ShortArray,
    VarcharArray,
}

/// A modifier chained onto a field type.
#[derive(Debug, Clone, PartialEq)]
pub enum Modifier {
    /// `.primary()` / `key` — PRIMARY KEY
    Primary,
    /// `.auto_increment()` / `auto` — SERIAL / BIGSERIAL
    AutoIncrement,
    /// `.nullable()` / `Option<T>` — wraps Rust type in Option<T>
    Nullable,
    /// `.unique()` / `unique` — UNIQUE constraint
    Unique,
    /// `.default("expression")` / `default = "expr"` — DEFAULT <expr>
    Default(String),
    /// `.now()` / `now` — DEFAULT now()
    Now,
    /// `.tz()` — WITH TIME ZONE (used by schema! only)
    Tz,
    /// `index` — CREATE INDEX on this column
    Index,
    /// `references("table", "column")` — FOREIGN KEY
    References { table: String, column: String },
    /// `on_delete = "cascade|set_null|restrict|no_action"` — FK behavior
    OnDelete(String),
}

/// Validation rules attached to a field via `#[col(...)]`.
///
/// These are parsed at compile time and used to generate a `validate()`
/// method on the model struct. The generated code is pure Rust —
/// no external validation crate is exposed to the user.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationRule {
    /// `#[col(len(min = 1, max = 100))]` — string length bounds
    Length { min: Option<u32>, max: Option<u32> },
    /// `#[col(email)]` — must be a valid email address
    Email,
    /// `#[col(url)]` — must be a valid URL
    Url,
    /// `#[col(range(min = 0, max = 150))]` — numeric range bounds
    Range { min: Option<f64>, max: Option<f64> },
    /// `#[col(regex("^[a-z]+$"))]` — must match a regex pattern
    Regex(String),
    /// `#[col(required)]` — Option<T> must be Some
    Required,
}

/// Table-level constraints.
#[derive(Debug, Clone, PartialEq)]
pub enum TableConstraint {
    /// `@primary_key(col1, col2)` — composite primary key
    PrimaryKey(Vec<String>),
    /// `@unique(col1, col2)` — composite unique constraint
    Unique(Vec<String>),
    /// `@index(col1, col2)` — index
    Index(Vec<String>),
}

// ── Helper methods ──

impl FieldDef {
    /// Check if this field has the `.primary()` modifier.
    pub fn is_primary(&self) -> bool {
        self.modifiers
            .iter()
            .any(|m| matches!(m, Modifier::Primary))
    }

    /// Check if this field has the `.nullable()` modifier.
    pub fn is_nullable(&self) -> bool {
        self.modifiers
            .iter()
            .any(|m| matches!(m, Modifier::Nullable))
    }

    /// Check if this field has the `.auto_increment()` modifier.
    pub fn is_auto_increment(&self) -> bool {
        self.modifiers
            .iter()
            .any(|m| matches!(m, Modifier::AutoIncrement))
    }

    /// Check if this field has the `.unique()` modifier.
    pub fn is_unique(&self) -> bool {
        self.modifiers.iter().any(|m| matches!(m, Modifier::Unique))
    }

    /// Check if this field has the `.tz()` modifier.
    pub fn is_tz(&self) -> bool {
        self.modifiers.iter().any(|m| matches!(m, Modifier::Tz))
    }
}

impl ModelDef {
    /// Find primary key column(s). Checks field modifiers first,
    /// then falls back to `@primary_key(...)` constraint.
    pub fn primary_key_columns(&self) -> Vec<&FieldDef> {
        // First: check for @primary_key constraint
        for constraint in &self.constraints {
            if let TableConstraint::PrimaryKey(cols) = constraint {
                return self
                    .db_columns
                    .iter()
                    .filter(|f| cols.contains(&f.rust_name.to_string()))
                    .collect();
            }
        }

        // Fallback: fields with .primary() modifier
        self.db_columns.iter().filter(|f| f.is_primary()).collect()
    }

    /// Returns true if this model has no primary key.
    pub fn has_primary_key(&self) -> bool {
        !self.primary_key_columns().is_empty()
    }

    /// Total number of database columns (for the 64-column limit check).
    pub fn column_count(&self) -> usize {
        self.db_columns.len()
    }
}

impl TypeInfo {
    /// The Rust type string for this column type.
    pub fn rust_type(&self, nullable: bool) -> String {
        let base = match self {
            TypeInfo::Integer => "i32".to_string(),
            TypeInfo::Short => "i16".to_string(),
            TypeInfo::BigInt => "i64".to_string(),
            TypeInfo::Real => "f32".to_string(),
            TypeInfo::Double => "f64".to_string(),
            TypeInfo::Decimal { .. } => "sqlx::types::BigDecimal".to_string(),
            TypeInfo::Varchar { .. } | TypeInfo::Text => "String".to_string(),
            TypeInfo::Bool => "bool".to_string(),
            TypeInfo::Date => "chrono::NaiveDate".to_string(),
            TypeInfo::Time => "chrono::NaiveTime".to_string(),
            TypeInfo::DateTime => "chrono::NaiveDateTime".to_string(), // .tz() changes this
            TypeInfo::Uuid => "uuid::Uuid".to_string(),
            TypeInfo::Binary => "Vec<u8>".to_string(),
            TypeInfo::Col { rust_type } => rust_type.clone(),
            // Native PG arrays
            TypeInfo::TextArray | TypeInfo::VarcharArray => "Vec<String>".to_string(),
            TypeInfo::IntArray => "Vec<i32>".to_string(),
            TypeInfo::ShortArray => "Vec<i16>".to_string(),
            TypeInfo::BigIntArray => "Vec<i64>".to_string(),
            TypeInfo::UuidArray => "Vec<uuid::Uuid>".to_string(),
            TypeInfo::BoolArray => "Vec<bool>".to_string(),
            TypeInfo::RealArray => "Vec<f32>".to_string(),
            TypeInfo::DoubleArray => "Vec<f64>".to_string(),
            TypeInfo::Json | TypeInfo::Jsonb => "serde_json::Value".to_string(),
            TypeInfo::Enum { rust_type } => rust_type.clone(),
            TypeInfo::Ltree => "String".to_string(),
        };

        if nullable {
            format!("Option<{}>", base)
        } else {
            base
        }
    }
}
