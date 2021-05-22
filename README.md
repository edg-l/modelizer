# modelizer

Generate rust code implementing the CRUD from a given database model.

## Example Run:

```text
Table name: post
Struct name: Post
Field name (empty to stop): id
Field name in database: id
Field (rust) type : Uuid
Field name (empty to stop): title
Field name in database: title
Field (rust) type : String
Field name (empty to stop): summary
Field name in database: summary
Field (rust) type : String
Field name (empty to stop): content
Field name in database: content
Field (rust) type : String
Field name (empty to stop): created_at
Field name in database: created_at
Field (rust) type : DateTime<Utc>
Field name (empty to stop):
Please select the fields which are primary keys: id: Uuid
Please select the fields which you want to exclude from a search: content: String
Should 'id' be included in the new() function parameters? no
Should 'title' be included in the new() function parameters? yes
Should 'summary' be included in the new() function parameters? yes
Should 'content' be included in the new() function parameters? yes
Should 'created_at' be included in the new() function parameters? no
Want to create a new struct using the fields added to new()? yes
Sub-struct name: PostPayload
Generate From<PostPayload> for Post ? yes
```

```rust
/// Represents a row in the post table
#[derive(Debug, serde::Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: Uuid,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}
impl Post {
    pub fn new(title: String, summary: String, content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            title,
            summary,
            content,
            created_at: chrono::Utc::now(),
        }
    }

    pub async fn save(&self, pool: &PgPool) -> Result<PgQueryResult> {
        sqlx::query!(
        "INSERT INTO post (id, title, summary, content, created_at) VALUES ($1, $2, $3, $4, $5)",
        self.id,
        self.title,
        self.summary,
        self.content,
        self.created_at,
        )
        .execute(pool)
        .await
    }

    pub async fn update(&self, pool: &PgPool) -> Result<PgQueryResult> {
        sqlx::query!(
        "UPDATE post set id = $1, title = $2, summary = $3, content = $4, created_at = $5 WHERE id = $6",
        self.id,
        self.title,
        self.summary,
        self.content,
        self.created_at,
        self.id,
        )
        .execute(pool)
        .await
    }

    pub async fn delete(&self, pool: &PgPool) -> Result<PgQueryResult> {
        sqlx::query!(
        "DELETE FROM post where id = $1",
        self.id,
        )
        .execute(pool)
        .await
    }

    pub async fn get(&self, pool: &PgPool, id: &Uuid) -> Result<Post> {
        sqlx::query_as!(Post,
        "SELECT id, title, summary, content, created_at FROM post WHERE id = $1",
        id,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn list(&self, pool: &PgPool, query: PostQuery) -> Result<Vec<Post>> {
        let limit = query.limit.map_or(50, |f| f.clamp(0, 100));
        let offset = query.offset.map_or(0, |f| f.min(0));
        let mut current_idx = 1i32;
        let mut where_clause = String::new();

        if query.id.is_some() {
            if current_idx == 1 {
                where_clause.push_str("WHERE ");
                where_clause.push_str(&format!("id = ${} ", current_idx));
            } else {
                where_clause.push_str(&format!("OR id = ${} ", current_idx));
            }
            current_idx += 1;
        }


        if query.title.is_some() {
            if current_idx == 1 {
                where_clause.push_str("WHERE ");
                where_clause.push_str(&format!("title LIKE ${} ", current_idx));
            } else {
                where_clause.push_str(&format!("OR title LIKE ${} ", current_idx));
            }
            current_idx += 1;
        }


        if query.summary.is_some() {
            if current_idx == 1 {
                where_clause.push_str("WHERE ");
                where_clause.push_str(&format!("summary LIKE ${} ", current_idx));
            } else {
                where_clause.push_str(&format!("OR summary LIKE ${} ", current_idx));
            }
            current_idx += 1;
        }


        if query.created_at.is_some() {
            if current_idx == 1 {
                where_clause.push_str("WHERE ");
                where_clause.push_str(&format!("created_at = ${} ", current_idx));
            } else {
                where_clause.push_str(&format!("OR created_at = ${} ", current_idx));
            }
            current_idx += 1;
        }

        let list_query = format!("SELECT id, title, summary, content, created_at FROM post {} LIMIT ${} OFFSET ${}", where_clause, current_idx + 1, current_idx + 2);
        let mut sql_query = sqlx::query_as(&list_query);

        if let Some(x) = query.id {
            sql_query = sql_query.bind(x);
        }


        if let Some(x) = query.title {
            sql_query = sql_query.bind(x);
        }


        if let Some(x) = query.summary {
            sql_query = sql_query.bind(x);
        }


        if let Some(x) = query.created_at {
            sql_query = sql_query.bind(x);
        }

        sql_query = sql_query.bind(limit).bind(offset);
        sql_query.fetch_all(pool)
        .await
    }
}
#[derive(Debug, Deserialize)]
pub struct PostPayload {
    pub title: String,
    pub summary: String,
    pub content: String,
}
impl From<PostPayload> for Post {
    fn from(p: PostPayload) -> Self {
        Post::new(p.title, p.summary, p.content)
    }
}
#[derive(Debug, Deserialize)]
pub struct PostQuery {
    pub id: Option<Uuid>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}
```
