use codegen::Formatter;
use dialoguer::*;
use inflector::Inflector;
use std::fmt;

fn main() {
    create_model().unwrap();
}

#[derive(Debug, Clone)]
struct Field {
    pub name: String,
    pub db_name: String,
    pub r#type: String,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.r#type)
    }
}

impl Field {
    fn new(name: String, db_name: String, r#type: String) -> Self {
        Self {
            name,
            db_name,
            r#type,
        }
    }
}

fn create_model() -> anyhow::Result<()> {
    let mut output = String::new();
    let mut formatter = Formatter::new(&mut output);

    let table_name: String = Input::new()
        .with_prompt("Table name")
        .allow_empty(false)
        .interact_text()?;

    let struct_name: String = Input::new()
        .with_prompt("Struct name")
        .default(table_name.to_pascal_case())
        .show_default(true)
        .allow_empty(false)
        .interact_text()?;

    let mut fields: Vec<Field> = Vec::new();

    loop {
        let field_name: String = Input::new()
            .with_prompt("Field name (empty to stop)")
            .allow_empty(true)
            .interact_text()?;

        if field_name.is_empty() {
            break;
        }

        let field_db_name: String = Input::new()
            .with_prompt("Field name in database")
            .default(field_name.clone())
            .show_default(true)
            .allow_empty(false)
            .interact_text()?;

        let field_type: String = Input::new()
            .with_prompt("Field (rust) type ")
            .default("String".to_owned())
            .show_default(true)
            .allow_empty(false)
            .interact_text()?;

        fields.push(Field::new(field_name, field_db_name, field_type));
    }

    let chosen: Vec<usize> = MultiSelect::new()
        .with_prompt("Please select the fields which are primary keys")
        .items(&fields)
        .interact()?;

    let main_primary_keys: Vec<&Field> = fields
        .iter()
        .enumerate()
        .filter_map(|(i, x)| if chosen.contains(&i) { Some(x) } else { None })
        .collect();

    let chosen_search: Vec<usize> = MultiSelect::new()
        .with_prompt("Please select the fields which you want to exclude from a search")
        .items(&fields)
        .interact()?;

    let search_fields: Vec<&Field> = fields
        .iter()
        .enumerate()
        .filter_map(|(i, x)| {
            if chosen_search.contains(&i) {
                None
            } else {
                Some(x)
            }
        })
        .collect();

    let mut main_struct = codegen::Struct::new(&struct_name);
    main_struct.vis("pub");
    main_struct.doc(&format!("Represents a row in the {} table", table_name));
    main_struct.derive("Debug");
    main_struct.derive("serde::Deserialize");
    main_struct.derive("serde::Serialize");
    main_struct.derive("sqlx::FromRow");

    let mut main_impl = codegen::Impl::new(&struct_name);

    let mut new_f = codegen::Function::new("new");
    let mut new_f_block = codegen::Block::new("Self");
    new_f.vis("pub");
    new_f.ret("Self");

    let mut non_default_fields: Vec<&Field> = Vec::new();

    for field in &fields {
        let f = codegen::Field::new(&format!("pub {}", field.name), &field.r#type);
        main_struct.push_field(f);

        if Confirm::new()
            .with_prompt(&format!(
                "Should '{}' be included in the new() function parameters?",
                field.name
            ))
            .default(true)
            .interact()?
        {
            new_f.arg(&field.name, &field.r#type);
            new_f_block.line(&format!("{},", field.name));
            non_default_fields.push(&field);
        } else {
            let value: String = match field.r#type.as_str() {
                "DateTime<Local>" => "chrono::Local::now()".to_string(),
                "DateTime<Utc>" => "chrono::Utc::now()".to_string(),
                "Uuid" | "uuid::Uuid" => "uuid::Uuid::new_v4()".to_string(),
                "String" => "String::new()".to_string(),
                _ => format!("{}::default()", field.r#type),
            };

            new_f_block.line(&format!("{}: {},", field.name, value));
        }
    }

    new_f.push_block(new_f_block);
    main_impl.push_fn(new_f);

    // Save function
    let save_f = main_impl.new_fn("save");
    save_f.arg_ref_self();
    save_f.ret("Result<PgQueryResult>");
    save_f.vis("pub");
    save_f.set_async(true);
    save_f.arg("pool", "&PgPool");
    save_f.line("sqlx::query!(");

    let save_query = format!(
        "\"INSERT INTO {} ({}) VALUES ({})\",",
        table_name,
        fields
            .iter()
            .map(|x| x.db_name.as_str())
            .collect::<Vec<&str>>()
            .join(", "),
        fields
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect::<Vec<String>>()
            .join(", ")
    );
    save_f.line(&save_query);

    for field in &fields {
        save_f.line(&format!("self.{},", field.name));
    }
    save_f.line(")");
    save_f.line(".execute(pool)");
    save_f.line(".await");

    // Update function
    let update_f = main_impl.new_fn("update");
    update_f.arg_ref_self();
    update_f.ret("Result<PgQueryResult>");
    update_f.vis("pub");
    update_f.set_async(true);
    update_f.arg("pool", "&PgPool");
    update_f.line("sqlx::query!(");

    let update_query = format!(
        "\"UPDATE {} set {} WHERE {}\",",
        table_name,
        fields
            .iter()
            .enumerate()
            .map(|(i, x)| format!("{} = ${}", x.db_name, i + 1))
            .collect::<Vec<String>>()
            .join(", "),
        main_primary_keys
            .iter()
            .enumerate()
            .map(|(i, x)| format!("{} = ${}", x.db_name, i + fields.len() + 1))
            .collect::<Vec<String>>()
            .join(" AND ")
    );
    update_f.line(&update_query);

    for field in &fields {
        update_f.line(&format!("self.{},", field.name));
    }
    for field in &main_primary_keys {
        update_f.line(&format!("self.{},", field.name));
    }
    update_f.line(")");
    update_f.line(".execute(pool)");
    update_f.line(".await");

    // Delete function
    let delete_f = main_impl.new_fn("delete");
    delete_f.arg_ref_self();
    delete_f.ret("Result<PgQueryResult>");
    delete_f.vis("pub");
    delete_f.set_async(true);
    delete_f.arg("pool", "&PgPool");
    delete_f.line("sqlx::query!(");

    let delete_query = format!(
        "\"DELETE FROM {} where {}\",",
        table_name,
        main_primary_keys
            .iter()
            .enumerate()
            .map(|(i, x)| format!("{} = ${}", x.db_name, i + 1))
            .collect::<Vec<String>>()
            .join(" AND ")
    );
    delete_f.line(&delete_query);

    for field in &main_primary_keys {
        delete_f.line(&format!("self.{},", field.name));
    }
    delete_f.line(")");
    delete_f.line(".execute(pool)");
    delete_f.line(".await");

    // Get function
    let get_f = main_impl.new_fn("get");
    get_f.arg_ref_self();
    get_f.ret(&format!("Result<{}>", struct_name));
    get_f.vis("pub");
    get_f.set_async(true);
    get_f.arg("pool", "&PgPool");
    for field in &main_primary_keys {
        get_f.arg(&field.name, &format!("&{}", &field.r#type));
    }
    get_f.line(&format!("sqlx::query_as!({},", struct_name));

    let get_query = format!(
        "\"SELECT {} FROM {} WHERE {}\",",
        fields
            .iter()
            .map(|x| x.db_name.as_str())
            .collect::<Vec<&str>>()
            .join(", "),
        table_name,
        main_primary_keys
            .iter()
            .enumerate()
            .map(|(i, x)| format!("{} = ${}", x.db_name, i + 1))
            .collect::<Vec<String>>()
            .join(" AND ")
    );
    get_f.line(&get_query);

    for field in &main_primary_keys {
        get_f.line(&format!("{},", field.name));
    }
    get_f.line(")");
    get_f.line(".fetch_one(pool)");
    get_f.line(".await");

    // List function
    let list_f = main_impl.new_fn("list");
    list_f.arg_ref_self();
    list_f.ret(&format!("Result<Vec<{}>>", struct_name));
    list_f.vis("pub");
    list_f.set_async(true);
    list_f.arg("pool", "&PgPool");
    list_f.arg("query", &format!("&{}Query", struct_name).to_pascal_case());
    list_f.line("let limit = query.limit.map_or(50, |f| f.clamp(0, 100));");
    list_f.line("let offset = query.offset.map_or(0, |f| f.max(0));");
    list_f.line("let mut current_idx = 1i32;");
    list_f.line("let mut where_clause = String::new();");

    // TODO: need to create a query builder

    for field in &search_fields {
        let equal_char = match field.r#type.as_str() {
            "String" => "LIKE",
            _ => "=",
        };
        list_f.line(format!(
            r#"
if query.{0}.is_some() {{
    if current_idx == 1 {{
        where_clause.push_str("WHERE ");
        where_clause.push_str(&format!("{1} {2} ${{}} ", current_idx));
    }} else {{
        where_clause.push_str(&format!("OR {1} {2} ${{}} ", current_idx));
    }}
    current_idx += 1; 
}}
"#,
            field.name, field.db_name, equal_char
        ));
    }

    let list_query = format!(
        r#"let list_query = format!("SELECT {} FROM {} {{}} LIMIT ${{}} OFFSET ${{}}", where_clause, current_idx + 1, current_idx + 2);"#,
        fields
            .iter()
            .map(|x| x.db_name.as_str())
            .collect::<Vec<&str>>()
            .join(", "),
        table_name,
    );

    list_f.line(&list_query);
    list_f.line("let mut sql_query = sqlx::query_as(&list_query);");

    for field in &search_fields {
        list_f.line(format!(
            r#"
if let Some(x) = query.{} {{
    sql_query = sql_query.bind(x);
}}
"#,
            field.name
        ));
    }

    list_f.line("sql_query = sql_query.bind(limit).bind(offset);");

    list_f.line("sql_query.fetch_all(pool)");
    list_f.line(".await");

    let mut sub_structs: Vec<(codegen::Struct, Option<codegen::Impl>)> = Vec::new();

    let mut query_struct = codegen::Struct::new(&format!("{}Query", struct_name).to_pascal_case());
    query_struct.vis("pub");
    query_struct.derive("Debug");
    query_struct.derive("Deserialize");

    for field in &search_fields {
        let field_type = {
            if field.r#type.starts_with("Option") {
                field.r#type.clone()
            } else {
                format!("Option<{}>", field.r#type)
            }
        };
        let f = codegen::Field::new(&format!("pub {}", field.name), &field_type);
        query_struct.push_field(f);
    }

    let limit_field = codegen::Field::new("pub limit", "Option<i32>");
    let offset_field = codegen::Field::new("pub offset", "Option<i32>");

    query_struct.push_field(limit_field);
    query_struct.push_field(offset_field);

    if Confirm::new()
        .with_prompt("Want to create a new struct using the fields added to new()?")
        .interact()?
    {
        let sub_struct_name: String = Input::new()
            .with_prompt("Sub-struct name")
            .default(format!("{}Payload", struct_name).to_pascal_case())
            .show_default(true)
            .allow_empty(false)
            .interact_text()?;

        let mut sub_struct = codegen::Struct::new(&sub_struct_name);
        sub_struct.vis("pub");
        sub_struct.derive("Debug");
        sub_struct.derive("Deserialize");

        for field in &non_default_fields {
            let f = codegen::Field::new(&format!("pub {}", field.name), &field.r#type);
            sub_struct.push_field(f);
        }

        let impl_b = {
            if Confirm::new()
                .with_prompt(&format!(
                    "Generate From<{}> for {} ?",
                    sub_struct_name, struct_name
                ))
                .default(true)
                .interact()?
            {
                let mut block = codegen::Impl::new(&struct_name);
                block.impl_trait(&format!("From<{}>", sub_struct_name));
                let from_fn = block.new_fn("from");
                from_fn.arg("p", &sub_struct_name);
                from_fn.ret("Self");

                // For we call new and assume args are in order.

                from_fn.line(&format!(
                    "{}::new({})",
                    struct_name,
                    non_default_fields
                        .iter()
                        .map(|x| format!("p.{}", x.name))
                        .collect::<Vec<String>>()
                        .join(", "),
                ));

                Some(block)
            } else {
                None
            }
        };

        sub_structs.push((sub_struct, impl_b));
    }

    main_struct.fmt(&mut formatter).unwrap();
    main_impl.fmt(&mut formatter).unwrap();

    for (sub_struct, impl_b) in &sub_structs {
        sub_struct.fmt(&mut formatter).unwrap();
        if let Some(impl_b) = impl_b {
            impl_b.fmt(&mut formatter).unwrap();
        }
    }

    query_struct.fmt(&mut formatter).unwrap();

    println!();
    println!("{}", output);

    Ok(())
}
