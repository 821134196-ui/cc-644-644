use std::sync::Arc;

use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

use repack_server::db;
use repack_server::mock::{MockPos, MockPrinter};
use repack_server::{build_app, make_pool, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://repack.db".into());
    let pool = make_pool(&database_url).await?;
    db::seed(&pool).await?;

    let state = AppState {
        pool,
        printer: Arc::new(MockPrinter::new()),
        pos: Arc::new(MockPos::new()),
    };

    // 前端构建产物（web/dist）由后端直接托管，一条命令即可启动
    let dist = std::env::var("WEB_DIST").unwrap_or_else(|_| "../web/dist".into());
    let app = build_app(state)
        .nest_service("/assets", ServeDir::new(format!("{dist}/assets")))
        .fallback_service(ServeDir::new(&dist).not_found_service(ServeFile::new(format!("{dist}/index.html"))))
        .layer(CorsLayer::permissive());

    let addr = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("拆零系统已启动: http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
