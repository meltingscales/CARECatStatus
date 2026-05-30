mod auth;
mod db;
mod models;
mod routes;
mod ws;

use std::sync::Arc;

use axum::{Router, middleware, routing::{get, post}};
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::ws::AppState;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "CARECatStatus",
        description = "Real-time cat status board for CARE Animal Shelter shift staff",
        version = "0.1.0"
    ),
    paths(
        routes::list_cats,
        routes::create_cat,
        routes::update_cat,
        routes::delete_cat,
    ),
    components(schemas(
        models::Cat,
        models::CatColor,
        models::CatLocation,
        models::CreateCat,
        models::UpdateCat,
    ))
)]
struct ApiDoc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "care_cat_status=debug,tower_http=info".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "cats.db".into());
    let pool = db::init(&database_url).await?;
    let state = Arc::new(AppState::new(pool));

    let protected = routes::router()
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_auth::<AppState>));

    let app = Router::new()
        .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
        .route("/ws", get(ws::handler))
        .nest("/api", protected)
        .route("/api/auth/status", get(auth::status_handler::<AppState>))
        .route("/api/auth/login", post(auth::login_handler::<AppState>))
        .fallback_service(ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("Listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
