// 测试公共辅助: 临时 SQLite + 内存中驱动完整 Router(oneshot), 不监听真实端口
#![allow(dead_code)]
use axum::body::Body;
use axum::http::{Request, StatusCode};
use pharmacy_split::{db, handlers::AppState, peripherals::Peripherals, router};
use serde_json::Value;
use tower::ServiceExt; // oneshot

pub struct TestApp {
    pub app: axum::Router,
    pub _dir: tempfile::TempDir,
}

impl TestApp {
    pub async fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = db::connect(&url).await.expect("connect");
        let app = router(AppState {
            pool,
            periph: Peripherals::new(),
        });
        TestApp { app, _dir: dir }
    }

    pub async fn req(
        &self,
        method: &str,
        uri: &str,
        store_id: Option<i64>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        request(&self.app, method, uri, store_id, body).await
    }
}

pub async fn request(
    app: &axum::Router,
    method: &str,
    uri: &str,
    store_id: Option<i64>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(s) = store_id {
        builder = builder.header("x-store-id", s.to_string());
    }
    let req = match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.expect("response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .expect("body");
    let value: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

/// 建门店、药品、母包装, 返回 (drug_id, pack_id)
pub async fn seed_pack(
    app: &axum::Router,
    store_count: i64,
    initial_qty: i64,
    batch: &str,
    opened: bool,
) -> (i64, i64) {
    for i in 1..=store_count {
        let code = format!("S{i:03}");
        let (st, _) = request(
            app,
            "POST",
            "/api/stores",
            None,
            Some(serde_json::json!({"code": code, "name": format!("门店{i}")})),
        )
        .await;
        assert!(st.is_success());
    }
    let (st, drug) = request(
        app,
        "POST",
        "/api/drugs",
        None,
        Some(serde_json::json!({"name": "测试胶囊", "spec": "0.25g*24粒", "unit_pack": "粒"})),
    )
    .await;
    assert!(st.is_success());
    let drug_id = drug["id"].as_i64().unwrap();

    let (st, pack) = request(
        app,
        "POST",
        "/api/packs",
        Some(1),
        Some(serde_json::json!({
            "drug_id": drug_id,
            "batch_no": batch,
            "pack_expiry": "2028-12-31",
            "opened_expiry": null,
            "initial_qty": initial_qty,
            "opened": opened,
        })),
    )
    .await;
    assert!(st.is_success(), "register pack failed: {pack}");
    (drug_id, pack["id"].as_i64().unwrap())
}

pub async fn get_pack(app: &axum::Router, id: i64) -> Value {
    let (st, v) = request(app, "GET", &format!("/api/packs/{id}"), None, None).await;
    assert_eq!(st, StatusCode::OK);
    v
}

pub async fn sale(app: &axum::Router, pack_id: i64, qty: i64, sim_fail: Option<&str>) -> (StatusCode, Value) {
    let mut body = serde_json::json!({
        "parent_pack_id": pack_id,
        "qty": qty,
        "dosage": "口服 一次1粒 一日3次",
    });
    if let Some(f) = sim_fail {
        body["sim_fail"] = serde_json::json!(f);
    }
    request(app, "POST", "/api/sales", Some(1), Some(body)).await
}
