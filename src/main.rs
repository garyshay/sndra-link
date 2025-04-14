use axum::{
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    routing::get_service,
    routing::{get, post},
    Json, Router,
};
use tower_http::services::ServeDir;

use dotenvy::dotenv;
use rand::{distr::Alphanumeric, Rng};
use serde::Deserialize;
use sqlx::migrate::MigrateDatabase;
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::env;
use std::net::SocketAddr;
#[derive(Clone)]
struct AppState {
    db: Pool<Sqlite>,
    domain: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    // initialize tracing
    tracing_subscriber::fmt::init();
    let static_dir = "./public";
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
    println!("{}", database_url);

    if !Sqlite::database_exists(&database_url)
        .await
        .unwrap_or(false)
    {
        println!("Creating database {}", database_url);
        match Sqlite::create_database(&database_url).await {
            Ok(_) => println!("Create db success"),
            Err(error) => panic!("error: {}", error),
        }
    } else {
        println!("Database already exists");
    }

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS short_links (
        id TEXT PRIMARY KEY,
        original_url TEXT NOT NULL,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&db)
    .await
    .unwrap();

    let domain: String = env::var("DOMAIN").expect("DOMAIN not set");

    let state = AppState { db, domain };

    let app = Router::new()
        .route("/", get_service(ServeDir::new(static_dir)))
        .route("/r/{key}", get(redirect_to_url))
        .route("/", post(create_link))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            verify_auth,
        ))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 7700));
    println!("🚀 Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn redirect_to_url(
    Path(key): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    println!("{}", key);
    let row = sqlx::query_scalar::<_, String>("SELECT original_url FROM short_links WHERE id = ?")
        .bind(&key)
        .fetch_optional(&state.db)
        .await;

    match row {
        Ok(Some(url)) => {
            println!("{}", url);
            return Redirect::permanent(&url).into_response();
        }
        Ok(None) => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", err)).into_response(),
    }
}

async fn verify_auth(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    next.run(request).await
}

async fn create_link(
    State(state): State<AppState>,
    Json(payload): Json<CreateLink>,
) -> impl IntoResponse {
    let key = generate_key();
    let domain = &state.domain;
    let short_url = format!("{}/r/{}", domain, key);

    if !is_valid_url(&payload.link) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid URL" })),
        );
    }

    sqlx::query("INSERT INTO short_links (id, original_url) VALUES (?, ?)")
        .bind(&key)
        .bind(&payload.link)
        .execute(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })
        .unwrap();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "short": short_url,
        })),
    )
}

fn is_valid_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn generate_key() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect()
}

#[derive(Deserialize)]
struct CreateLink {
    link: String,
}
