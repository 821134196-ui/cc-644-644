use std::net::SocketAddr;
use std::path::Path;

use pharmacy_split::{db, handlers::AppState, peripherals::Peripherals, router};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,pharmacy_server=debug".into()),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);
    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/pharmacy.db?mode=rwc".into());

    // 确保 SQLite 文件目录存在(sqlite::memory: 除外)
    if let Some(rest) = db_url.strip_prefix("sqlite://") {
        let path_part = rest.split('?').next().unwrap_or(rest);
        if !path_part.is_empty() && path_part != ":memory:" {
            if let Some(parent) = Path::new(path_part).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
        }
    }

    let pool = db::connect(&db_url).await?;
    let state = AppState {
        pool,
        periph: Peripherals::new(),
    };

    // API 之外, 若前端已构建(dist/)则由后端一并托管, 单进程即可演示。
    // 开发时前端走 Vite (5173), /api 由 Vite 代理到 8080。
    let app = if Path::new("frontend/dist").exists() {
        router(state).fallback_service(ServeDir::new("frontend/dist"))
    } else {
        router(state)
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("药店拆零系统后端监听 http://{addr}");
    if !Path::new("frontend/dist").exists() {
        tracing::info!("未发现 frontend/dist, 请用 ./run.sh dev 模式启动前端");
    }
    axum::serve(listener, app).await?;
    Ok(())
}
