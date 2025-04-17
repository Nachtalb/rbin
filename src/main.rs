use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use dotenvy::dotenv;
use rand::distributions::{Alphanumeric, DistString};
use std::{
    env,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};
use tokio::fs;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

// --- Configuration Constants ---
const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 3000;
const DEFAULT_PASTE_DIR: &str = "pastes";
const ID_LENGTH: usize = 6;
const MAX_BODY_SIZE: usize = 1024 * 1024 * 10; // 10 MB
                                               // Default log level for tower_http requests if RUST_LOG is not set
const DEFAULT_REQUEST_LOG_LEVEL: &str = "debug";

// --- Application State ---
#[derive(Clone)]
struct AppState {
    paste_dir: Arc<PathBuf>,
}

#[tokio::main]
async fn main() {
    // Load .env file if present
    match dotenv() {
        Ok(path) => println!("Loaded .env file from: {:?}", path), // Use println as logging isn't up yet
        Err(_) => {}
    }

    // --- Initialize Logging ---

    // Read the desired request log level from environment variable
    // This controls tower_http level *only* if RUST_LOG is not set.
    let request_log_level = env::var("RBIN_REQUEST_LOG_LEVEL")
        .unwrap_or_else(|_| DEFAULT_REQUEST_LOG_LEVEL.to_string());
    // Basic validation could be added here if needed (e.g., check if it's a valid level)

    // Set up the log filter:
    // 1. Try to use RUST_LOG environment variable if set.
    // 2. If RUST_LOG is not set, construct a default filter using:
    //    - "info" for the application crate (`rbin`)
    //    - The level from RBIN_REQUEST_LOG_LEVEL for `tower_http`
    let log_filter = EnvFilter::try_from_default_env()
        .or_else(|_| {
            // RUST_LOG was not set, build the default filter string
            let default_app_level = "info"; // Default level for our app
            let default_filter_str = format!(
                "{},tower_http={}", // Comma-separated directives
                default_app_level,
                request_log_level // Use the configured level for requests
            );
            EnvFilter::try_new(default_filter_str) // Parse the constructed default
        })
        .expect("Failed to parse log filter configuration"); // Panic if parsing fails

    // Initialize the tracing subscriber
    tracing_subscriber::registry()
        .with(log_filter) // Apply the determined filter
        .with(tracing_subscriber::fmt::layer()) // Format logs for printing
        .init(); // Set as the global default subscriber

    // Log service start (now respects the filter)
    tracing::info!("Starting rbin...");
    tracing::info!("Default request log level set to: {}", request_log_level); // Log the request level being used in default config

    // Read Configuration
    let host_str = env::var("RBIN_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port_str = env::var("RBIN_PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let paste_dir_str =
        env::var("RBIN_PASTE_DIR").unwrap_or_else(|_| DEFAULT_PASTE_DIR.to_string());

    let host: IpAddr = host_str.parse().unwrap_or_else(|e| {
        tracing::warn!(
            "Invalid RBIN_HOST '{}', using default {}: {}",
            host_str,
            DEFAULT_HOST,
            e
        );
        DEFAULT_HOST.parse().unwrap()
    });
    let port: u16 = port_str.parse().unwrap_or_else(|e| {
        tracing::warn!(
            "Invalid RBIN_PORT '{}', using default {}: {}",
            port_str,
            DEFAULT_PORT,
            e
        );
        DEFAULT_PORT
    });
    let paste_dir = PathBuf::from(paste_dir_str);

    // Ensure Paste Directory Exists
    if let Err(e) = fs::create_dir_all(&paste_dir).await {
        tracing::error!("Failed to create paste directory {:?}: {}", paste_dir, e);
        eprintln!(
            "Error: Could not create paste directory at {:?}. Please check permissions.",
            paste_dir
        );
        return;
    }
    tracing::info!("Using paste directory: {:?}", paste_dir);

    // Create Application State
    let app_state = AppState {
        paste_dir: Arc::new(paste_dir),
    };

    // Build Axum App
    let app = Router::new()
        .route("/", get(handle_root_get))
        .route("/", post(handle_paste_submission))
        .route("/:id", get(retrieve_paste))
        .layer(TraceLayer::new_for_http()) // tower_http logging is controlled by the EnvFilter
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        .with_state(app_state);

    // Start Server
    let addr = SocketAddr::from((host, port));
    tracing::info!("rbin configured. Attempting to listen on {}", addr);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            tracing::info!("Successfully bound to {}. rbin is running.", addr);
            listener
        }
        Err(e) => {
            tracing::error!("Failed to bind to address {}: {}", addr, e);
            eprintln!("Error: Could not bind to address {}. Is the port already in use or the IP address valid?", addr);
            return;
        }
    };
    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("Server error: {}", e);
        eprintln!("Server encountered an error: {}", e);
    }
}

// --- Handler for GET / ---
async fn handle_root_get() -> impl IntoResponse {
    tracing::debug!("Serving root plain text info.");
    let plain_text_content = format!(
        r#"rbin - Simple Command-Line Pastebin
===================================

Usage:
------
Pipe text using curl (or similar tools) with the form field name 'rbin':

  echo "Your text here" | curl -F 'rbin=<-' http://<host>:<port>/

Or paste from a file:

  cat your_file.txt | curl -F 'rbin=<-' http://<host>:<port>/

rbin will respond with a URL like http://<host>:<port>/<id>

Configuration (Environment Variables):
--------------------------------------
RBIN_HOST               : Listen IP address (Default: {})
RBIN_PORT               : Listen port (Default: {})
RBIN_PASTE_DIR          : Directory for storing pastes (Default: "{}")
RBIN_REQUEST_LOG_LEVEL  : Log level for HTTP requests (tower_http) if RUST_LOG is not set (Default: {})
RUST_LOG                : Overrides all log levels (e.g., "info", "rbin=debug,tower_http=warn")

Place these in a .env file or set them in your environment.
"#,
        DEFAULT_HOST,
        DEFAULT_PORT,
        DEFAULT_PASTE_DIR,
        DEFAULT_REQUEST_LOG_LEVEL // Added new env var to help text
    );
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        plain_text_content,
    )
}

// --- Handler for POST / ---
async fn handle_paste_submission(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    tracing::debug!("Received paste submission request.");
    let mut paste_content: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Error reading multipart field: {}", e);
        (
            StatusCode::BAD_REQUEST,
            format!("Error processing form data: {}", e),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "rbin" {
            let data = field.text().await.map_err(|e| {
                tracing::error!("Failed to read 'rbin' field data as text: {}", e);
                (
                    StatusCode::BAD_REQUEST,
                    format!("Failed to read field data: {}", e),
                )
            })?;
            paste_content = Some(data);
            break;
        } else {
            let _ = field.bytes().await;
            tracing::debug!("Ignoring field '{}'", name);
        }
    }

    let content = paste_content.ok_or_else(|| {
        tracing::warn!("Missing 'rbin' field in submission.");
        (
            StatusCode::BAD_REQUEST,
            "Missing 'rbin' form field".to_string(),
        )
    })?;

    if content.is_empty() {
        tracing::warn!("Received empty 'rbin' field.");
        return Err((
            StatusCode::BAD_REQUEST,
            "Paste content cannot be empty".to_string(),
        ));
    }

    let id = Alphanumeric.sample_string(&mut rand::thread_rng(), ID_LENGTH);
    let file_path = state.paste_dir.join(format!("{}.txt", id));

    tracing::info!("Generated ID: {}, saving to {:?}", id, file_path);
    fs::write(&file_path, content).await.map_err(|e| {
        tracing::error!("Failed to write paste file {:?}: {}", file_path, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save paste: {}", e),
        )
    })?;

    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    let scheme = headers
        .get("X-Forwarded-Proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("http");
    let base_url = format!("{}://{}", scheme, host);
    let result_url = format!("{}/{}", base_url, id);

    tracing::info!("Paste created successfully: {}", result_url);
    Ok((StatusCode::OK, result_url))
}

// --- Handler for GET /:id ---
async fn retrieve_paste(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    tracing::debug!("Received request to retrieve paste ID: {}", id);
    if id.len() != ID_LENGTH || !id.chars().all(char::is_alphanumeric) {
        tracing::warn!("Invalid ID format received: {}", id);
        return (StatusCode::BAD_REQUEST, Html("Invalid paste ID format.")).into_response();
    }

    let file_path = state.paste_dir.join(format!("{}.txt", id));
    tracing::debug!("Attempting to read file: {:?}", file_path);

    match fs::read_to_string(&file_path).await {
        Ok(content) => {
            tracing::debug!("Successfully retrieved paste ID: {}", id);
            (
                StatusCode::OK,
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/plain; charset=utf-8"),
                )],
                content,
            )
                .into_response()
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                tracing::warn!("Paste ID not found: {}, path: {:?}", id, file_path);
                (
                    StatusCode::NOT_FOUND,
                    Html(format!("Paste '{}' not found.", id)),
                )
                    .into_response()
            } else {
                tracing::error!("Error reading paste file {:?}: {}", file_path, e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html("Error retrieving paste."),
                )
                    .into_response()
            }
        }
    }
}
