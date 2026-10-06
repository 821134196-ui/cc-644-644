//! 交付验证：并发扣量、打印失败、退药隔离、批次召回。
//! 运行：cd server && cargo test

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use repack_server::mock::{MockPos, MockPrinter};
use repack_server::{build_app, make_pool, AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn test_app(tag: &str) -> (axum::Router, AppState) {
    let dir = std::env::temp_dir().join(format!("repack-test-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let url = format!("sqlite://{}/test.db", dir.display());
    let pool = make_pool(&url).await.unwrap();
    repack_server::db::seed(&pool).await.unwrap();
    let state = AppState {
        pool,
        printer: Arc::new(MockPrinter::new()),
        pos: Arc::new(MockPos::new()),
    };
    (build_app(state.clone()), state)
}

async fn call(app: axum::Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(match body {
            Some(b) => Body::from(serde_json::to_string(&b).unwrap()),
            None => Body::empty(),
        })
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap_or(json!(null));
    (status, v)
}

async fn create_mother(app: &axum::Router, store: i64, drug: i64, batch: &str, qty: i64) -> i64 {
    let (s, v) = call(
        app.clone(),
        "POST",
        "/api/mothers",
        Some(json!({"store_id": store, "drug_id": drug, "batch_no": batch, "expiry_date": "2027-12-31", "total_qty": qty})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "create mother failed: {v}");
    v["id"].as_i64().unwrap()
}

async fn mother_remaining(state: &AppState, id: i64) -> i64 {
    sqlx::query_scalar("SELECT remaining_qty FROM mother_packages WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

/// 1) 并发扣量：多人同时拆同一盒，不能扣出负数，售出总量不得超过库存
#[tokio::test]
async fn concurrent_repack_never_oversells() {
    let (app, state) = test_app("conc").await;
    let mid = create_mother(&app, 1, 1, "CONC-BATCH", 50).await;

    // 20 个并发请求，每个拆 5 份（总需求 100 > 库存 50）
    let mut handles = Vec::new();
    for _ in 0..20 {
        let app = app.clone();
        handles.push(tokio::spawn(async move {
            call(app, "POST", "/api/repacks", Some(json!({"mother_id": mid, "qty": 5}))).await
        }));
    }
    let mut sold = 0i64;
    let mut rejected = 0;
    for h in handles {
        let (status, body) = h.await.unwrap();
        if status == StatusCode::OK {
            assert_eq!(body["status"], "SOLD");
            sold += 5;
        } else {
            rejected += 1;
            assert!(body["error"].as_str().unwrap().contains("库存不足"));
        }
    }
    assert_eq!(sold, 50, "售出总量必须恰好等于库存");
    assert_eq!(rejected, 10, "超卖部分必须被拒绝");
    assert_eq!(mother_remaining(&state, mid).await, 0, "余量必须归零且不为负");

    // 数据库层面再校验：SOLD 订单总量 == 50，无负数余量
    let sold_db: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(qty),0) FROM repack_orders WHERE mother_id=? AND status='SOLD'")
        .bind(mid)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(sold_db, 50);
    let min_remaining: i64 = sqlx::query_scalar("SELECT MIN(remaining_qty) FROM mother_packages")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(min_remaining >= 0);
}

/// 2) 打印失败：拆零单不得记为已售出，数量退回母包装，收银无记录
#[tokio::test]
async fn print_failure_is_not_sold() {
    let (app, state) = test_app("printfail").await;
    let mid = create_mother(&app, 1, 1, "PF-BATCH", 30).await;

    // 打开打印机故障开关
    let (s, _) = call(app.clone(), "POST", "/api/mock/printer/config", Some(json!({"fail": true}))).await;
    assert_eq!(s, StatusCode::OK);

    let (s, v) = call(app.clone(), "POST", "/api/repacks", Some(json!({"mother_id": mid, "qty": 10}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(v["error"].as_str().unwrap().contains("打印失败"));

    // 订单存在但状态为 PRINT_FAILED，不算售出
    let status: String = sqlx::query_scalar("SELECT status FROM repack_orders WHERE mother_id = ?")
        .bind(mid)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(status, "PRINT_FAILED");
    let sold_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repack_orders WHERE mother_id=? AND status='SOLD'")
        .bind(mid)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(sold_count, 0);

    // 数量已退回母包装
    assert_eq!(mother_remaining(&state, mid).await, 30);
    // 收银 mock 无销售记录
    assert!(state.pos.sales().is_empty());

    // 打印机恢复后可正常售出
    call(app.clone(), "POST", "/api/mock/printer/config", Some(json!({"fail": false}))).await;
    let (s, v) = call(app.clone(), "POST", "/api/repacks", Some(json!({"mother_id": mid, "qty": 10}))).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["status"], "SOLD");
    assert_eq!(v["remaining_after"], 20);
    assert_eq!(v["label"]["batch_no"], "PF-BATCH");
    assert_eq!(state.pos.sales().len(), 1);
}

/// 3) 退药隔离：退回散装药进入待处理库存，不增加母盒余量
#[tokio::test]
async fn returns_go_to_pending_stock_only() {
    let (app, state) = test_app("returns").await;
    let mid = create_mother(&app, 1, 1, "RET-BATCH", 40).await;
    let (s, v) = call(app.clone(), "POST", "/api/repacks", Some(json!({"mother_id": mid, "qty": 8}))).await;
    assert_eq!(s, StatusCode::OK);
    let order_id = v["order_id"].as_i64().unwrap();
    assert_eq!(mother_remaining(&state, mid).await, 32);

    // 顾客退回 3 份
    let (s, v) = call(
        app.clone(),
        "POST",
        "/api/returns",
        Some(json!({"store_id": 1, "drug_id": 1, "repack_order_id": order_id, "qty": 3})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["status"], "PENDING_DISPOSAL");

    // 母盒余量不变
    assert_eq!(mother_remaining(&state, mid).await, 32, "退药不得回充母盒余量");
    // 待处理库存可见
    let (_, list) = call(app.clone(), "GET", "/api/returns", None).await;
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["qty"], 3);
    assert_eq!(arr[0]["status"], "PENDING_DISPOSAL");
}

/// 4) 批次召回：能找到相关母包装与全部拆零销售记录，召回后禁止再拆零
#[tokio::test]
async fn batch_recall_traces_and_blocks() {
    let (app, state) = test_app("recall").await;
    let m1 = create_mother(&app, 1, 1, "RECALL-1", 24).await;
    let m2 = create_mother(&app, 2, 1, "RECALL-1", 24).await; // 同批次另一门店
    let m3 = create_mother(&app, 1, 1, "OTHER-BATCH", 24).await;

    let (s, _) = call(app.clone(), "POST", "/api/repacks", Some(json!({"mother_id": m1, "qty": 6}))).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(app.clone(), "POST", "/api/repacks", Some(json!({"mother_id": m2, "qty": 4}))).await;
    assert_eq!(s, StatusCode::OK);

    // 召回查询：应列出两个母包装 + 两条销售记录，不含其他批次
    let (s, v) = call(app.clone(), "GET", "/api/recalls/lookup?drug_id=1&batch_no=RECALL-1", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["mothers"].as_array().unwrap().len(), 2);
    let recs = v["repack_records"].as_array().unwrap();
    assert_eq!(recs.len(), 2);
    let total_sold: i64 = recs.iter().map(|r| r["qty"].as_i64().unwrap()).sum();
    assert_eq!(total_sold, 10);

    // 执行召回
    let (s, v) = call(app.clone(), "POST", "/api/recalls", Some(json!({"drug_id": 1, "batch_no": "RECALL-1"}))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["recalled_mothers"], 2);

    // 召回后禁止拆零
    let (s, v) = call(app.clone(), "POST", "/api/repacks", Some(json!({"mother_id": m1, "qty": 1}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(v["error"].as_str().unwrap().contains("召回"));

    // 其他批次不受影响
    let (s, _) = call(app.clone(), "POST", "/api/repacks", Some(json!({"mother_id": m3, "qty": 1}))).await;
    assert_eq!(s, StatusCode::OK);

    let recalled: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mother_packages WHERE batch_no='RECALL-1' AND status='RECALLED'")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(recalled, 2);
}

/// 5) 调拨规则：只允许未拆零整包在门店间调拨
#[tokio::test]
async fn transfer_only_unopened_packs() {
    let (app, _state) = test_app("transfer").await;
    let whole = create_mother(&app, 1, 1, "TR-WHOLE", 24).await;
    let opened = create_mother(&app, 1, 1, "TR-OPENED", 24).await;
    let (s, _) = call(app.clone(), "POST", "/api/repacks", Some(json!({"mother_id": opened, "qty": 2}))).await;
    assert_eq!(s, StatusCode::OK);

    // 已拆零的不能调拨
    let (s, v) = call(app.clone(), "POST", "/api/transfers", Some(json!({"mother_id": opened, "to_store_id": 2}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(v["error"].as_str().unwrap().contains("未拆零"));

    // 整包可调拨到分店
    let (s, _) = call(app.clone(), "POST", "/api/transfers", Some(json!({"mother_id": whole, "to_store_id": 2}))).await;
    assert_eq!(s, StatusCode::OK);
    let (_, v) = call(app.clone(), "GET", &format!("/api/mothers/{whole}"), None).await;
    assert_eq!(v["store_id"], 2);
}
