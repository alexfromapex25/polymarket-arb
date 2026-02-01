//! HTTP API module for health, metrics, and status endpoints.

pub mod handlers;
pub mod routes;

pub use handlers::AppState;
pub use routes::create_router;
