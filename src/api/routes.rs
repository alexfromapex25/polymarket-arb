//! HTTP API route definitions.

use axum::{routing::get, Router};

use super::handlers::{health, ready, status, AppState};

/// Create the API router.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health endpoints
        .route("/health", get(health))
        .route("/ready", get(ready))
        // Status endpoint
        .route("/api/v1/status", get(status))
        // TODO: Add metrics endpoint
        // .route("/metrics", get(metrics))
        .with_state(state)
}

/// Create a minimal health-only router (for startup).
pub fn health_router() -> Router {
    Router::new().route("/health", get(health))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let state = AppState::new();
        let app = create_router(state);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn ready_endpoint_returns_503_when_not_ready() {
        let state = AppState::new();
        let app = create_router(state);

        let response = app
            .oneshot(Request::builder().uri("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn ready_endpoint_returns_200_when_ready() {
        let state = AppState::new();
        state.set_ready(true);
        let app = create_router(state);

        let response = app
            .oneshot(Request::builder().uri("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
