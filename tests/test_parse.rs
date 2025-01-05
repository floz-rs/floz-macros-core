use floz_macros_core::ast::{Modifier, SchemaInput, TableConstraint, TypeInfo};

fn parse(input: &str) -> SchemaInput {
    syn::parse_str::<SchemaInput>(input).unwrap()
}

fn try_parse(input: &str) -> syn::Result<SchemaInput> {
    syn::parse_str::<SchemaInput>(input)
}

// ── Basic model parsing ──

#[test]
fn parse_simple_model() {
    let schema = parse(r#"
        model User("users") {
            id: integer("id").auto_increment().primary(),
        }
    "#);
    assert_eq!(schema.models.len(), 1);
    assert_eq!(schema.models[0].name.to_string(), "User");
    assert_eq!(schema.models[0].table_name, "users");
    assert_eq!(schema.models[0].db_columns.len(), 1);
}

#[test]
fn parse_model_name_and_table() {
    let schema = parse(r#"
        model AppUser("app_users") {
            id: integer("user_id"),
        }
    "#);
    assert_eq!(schema.models[0].name.to_string(), "AppUser");
    assert_eq!(schema.models[0].table_name, "app_users");
    assert_eq!(schema.models[0].db_columns[0].column_name, "user_id");
}

// ── Field types ──

#[test]
fn parse_all_basic_types() {
    let schema = parse(r#"
        model Test("test") {
            a: integer("a"),
            b: short("b"),
            c: bigint("c"),
            d: real("d"),
            e: double("e"),
            f: text("f"),
            g: bool("g"),
            h: date("h"),
            i: time("i"),
            j: datetime("j"),
            k: uuid("k"),
            l: binary("l"),
        }
    "#);
    assert_eq!(schema.models[0].db_columns.len(), 12);
    assert_eq!(schema.models[0].db_columns[0].type_info, TypeInfo::Integer);
    assert_eq!(schema.models[0].db_columns[1].type_info, TypeInfo::Short);
    assert_eq!(schema.models[0].db_columns[2].type_info, TypeInfo::BigInt);
    assert_eq!(schema.models[0].db_columns[3].type_info, TypeInfo::Real);
    assert_eq!(schema.models[0].db_columns[4].type_info, TypeInfo::Double);
    assert_eq!(schema.models[0].db_columns[5].type_info, TypeInfo::Text);
    assert_eq!(schema.models[0].db_columns[6].type_info, TypeInfo::Bool);
    assert_eq!(schema.models[0].db_columns[7].type_info, TypeInfo::Date);
    assert_eq!(schema.models[0].db_columns[8].type_info, TypeInfo::Time);
    assert_eq!(schema.models[0].db_columns[9].type_info, TypeInfo::DateTime);
    assert_eq!(schema.models[0].db_columns[10].type_info, TypeInfo::Uuid);
    assert_eq!(schema.models[0].db_columns[11].type_info, TypeInfo::Binary);
}

#[test]
fn parse_varchar_with_length() {
    let schema = parse(r#"
        model T("t") { name: varchar("name", 100) }
    "#);
    assert_eq!(
        schema.models[0].db_columns[0].type_info,
        TypeInfo::Varchar { max_length: 100 }
    );
}

#[test]
fn parse_decimal_with_precision() {
    let schema = parse(r#"
        model T("t") { rating: decimal("rating", 5, 2) }
    "#);
    assert_eq!(
        schema.models[0].db_columns[0].type_info,
        TypeInfo::Decimal { precision: 5, scale: 2 }
    );
}

#[test]
fn parse_col_generic() {
    let schema = parse(r#"
        model T("t") { data: col(Value, "data") }
    "#);
    assert_eq!(
        schema.models[0].db_columns[0].type_info,
        TypeInfo::Col { rust_type: "Value".to_string() }
    );
}

#[test]
fn parse_array_types() {
    let schema = parse(r#"
        model T("t") {
            a: text_array("a"),
            b: int_array("b"),
            c: uuid_array("c"),
        }
    "#);
    assert_eq!(schema.models[0].db_columns[0].type_info, TypeInfo::TextArray);
    assert_eq!(schema.models[0].db_columns[1].type_info, TypeInfo::IntArray);
    assert_eq!(schema.models[0].db_columns[2].type_info, TypeInfo::UuidArray);
}

// ── Modifiers ──

#[test]
fn parse_modifiers() {
    let schema = parse(r#"
        model T("t") {
            id: integer("id").auto_increment().primary(),
            name: varchar("name", 50).nullable().unique(),
            created: datetime("created").now().tz(),
        }
    "#);
    let id = &schema.models[0].db_columns[0];
    assert!(id.is_auto_increment());
    assert!(id.is_primary());

    let name = &schema.models[0].db_columns[1];
    assert!(name.is_nullable());
    assert!(name.is_unique());

    let created = &schema.models[0].db_columns[2];
    assert!(created.modifiers.contains(&Modifier::Now));
    assert!(created.is_tz());
}

#[test]
fn parse_default_modifier() {
    let schema = parse(r#"
        model T("t") { status: varchar("status", 20).default("active") }
    "#);
    let field = &schema.models[0].db_columns[0];
    assert!(field.modifiers.contains(&Modifier::Default("active".to_string())));
}

// ── Relationships ──

#[test]
fn parse_relationship() {
    let schema = parse(r#"
        model User("users") {
            id: integer("id"),
            posts: array(Post, "author_id"),
        }
    "#);
    assert_eq!(schema.models[0].db_columns.len(), 1); // only id
    assert_eq!(schema.models[0].relationships.len(), 1);
    assert_eq!(schema.models[0].relationships[0].rust_name.to_string(), "posts");
    assert_eq!(schema.models[0].relationships[0].target_model.to_string(), "Post");
    assert_eq!(schema.models[0].relationships[0].fk_column, "author_id");
}

#[test]
fn parse_relationship_excluded_from_columns() {
    let schema = parse(r#"
        model Post("posts") {
            id: integer("id"),
            title: varchar("title", 255),
            authors: array(User, "author_id"),
        }
    "#);
    // Only id and title are db_columns — authors is a relationship
    assert_eq!(schema.models[0].db_columns.len(), 2);
    assert_eq!(schema.models[0].relationships.len(), 1);
}

// ── Constraints ──

#[test]
fn parse_primary_key_constraint() {
    let schema = parse(r#"
        model PostTag("post_tags") {
            post_id: integer("post_id"),
            tag_id: integer("tag_id"),
            @primary_key(post_id, tag_id),
        }
    "#);
    assert_eq!(schema.models[0].constraints.len(), 1);
    assert_eq!(
        schema.models[0].constraints[0],
        TableConstraint::PrimaryKey(vec!["post_id".into(), "tag_id".into()])
    );
}

#[test]
fn parse_unique_constraint() {
    let schema = parse(r#"
        model T("t") {
            a: integer("a"),
            b: integer("b"),
            @unique(a, b),
        }
    "#);
    assert_eq!(
        schema.models[0].constraints[0],
        TableConstraint::Unique(vec!["a".into(), "b".into()])
    );
}

#[test]
fn parse_index_constraint() {
    let schema = parse(r#"
        model T("t") {
            name: varchar("name", 100),
            @index(name),
        }
    "#);
    assert_eq!(
        schema.models[0].constraints[0],
        TableConstraint::Index(vec!["name".into()])
    );
}

// ── Multiple models ──

#[test]
fn parse_multiple_models() {
    let schema = parse(r#"
        model User("users") {
            id: integer("id").primary(),
            name: varchar("name", 100),
        }
        model Post("posts") {
            id: integer("id").primary(),
            title: text("title"),
        }
    "#);
    assert_eq!(schema.models.len(), 2);
    assert_eq!(schema.models[0].name.to_string(), "User");
    assert_eq!(schema.models[1].name.to_string(), "Post");
}

// ── Validation ──

#[test]
fn parse_unknown_type_error() {
    let result = try_parse(r#"
        model T("t") { id: intgr("id") }
    "#);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown type function `intgr`"));
    assert!(err.contains("available types"));
}

#[test]
fn parse_unknown_modifier_error() {
    let result = try_parse(r#"
        model T("t") { id: integer("id").primry() }
    "#);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown modifier `.primry`"));
}

#[test]
fn parse_unknown_constraint_error() {
    let result = try_parse(r#"
        model T("t") {
            id: integer("id"),
            @foreign_key(id),
        }
    "#);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown constraint `@foreign_key`"));
}

#[test]
fn parse_empty_schema_error() {
    let result = try_parse("");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("at least one model"));
}

// ── Helper methods ──

#[test]
fn primary_key_from_modifier() {
    let schema = parse(r#"
        model T("t") {
            id: integer("id").primary(),
            name: text("name"),
        }
    "#);
    let pks = schema.models[0].primary_key_columns();
    assert_eq!(pks.len(), 1);
    assert_eq!(pks[0].rust_name.to_string(), "id");
}

#[test]
fn primary_key_from_constraint() {
    let schema = parse(r#"
        model T("t") {
            a: integer("a"),
            b: integer("b"),
            @primary_key(a, b),
        }
    "#);
    let pks = schema.models[0].primary_key_columns();
    assert_eq!(pks.len(), 2);
}

#[test]
fn no_primary_key() {
    let schema = parse(r#"
        model AuditLog("audit_logs") {
            event: text("event"),
            timestamp: datetime("timestamp"),
        }
    "#);
    assert!(!schema.models[0].has_primary_key());
}
