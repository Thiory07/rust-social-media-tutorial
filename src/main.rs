use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    FromRow, SqlitePool,
};
use std::net::SocketAddr;
use std::str::FromStr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// FromRow lets SQLx map database table columns straight into Rust struct fields
#[derive(Clone, Serialize, FromRow)]
struct Post {
    id: i64,
    author: String,
    content: String,
}

#[derive(Deserialize)]
struct CreatePost {
    author: String,
    content: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 1. Configure SQLite to create "social.db" if missing
    let db_options = SqliteConnectOptions::from_str("sqlite://social.db")
        .unwrap()
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(db_options)
        .await
        .expect("Failed to connect to SQLite database");

    // 2. Create the posts table if it doesn't exist
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            author TEXT NOT NULL,
            content TEXT NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("Failed to create posts table");

    // 3. Mount router with pool as shared state
    let app = Router::new()
        .route("/posts", get(get_posts))
        .route("/posts", post(create_post))
        .with_state(pool);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// GET /posts - fetches all rows from SQLite
async fn get_posts(
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<Post>>, StatusCode> {
    let posts = sqlx::query_as::<_, Post>("SELECT id, author, content FROM posts ORDER BY id ASC")
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(posts))
}

// POST /posts - inserts into disk and returns the created post with auto-incremented ID
async fn create_post(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreatePost>,
) -> Result<(StatusCode, Json<Post>), StatusCode> {
    let result = sqlx::query("INSERT INTO posts (author, content) VALUES (?, ?)")
        .bind(&payload.author)
        .bind(&payload.content)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let post = Post {
        id: result.last_insert_rowid(),
        author: payload.author,
        content: payload.content,
    };

    Ok((StatusCode::CREATED, Json(post)))
}