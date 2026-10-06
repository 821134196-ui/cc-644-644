// 规则验证 1: 多人同时拆零同一盒 —— 绝不扣成负数, 库存不足拒绝销售。
mod common;

use common::*;
use tokio::task::JoinSet;

async fn concurrent_sales(app: &axum::Router, pack_id: i64, qty: i64, n: usize) -> Vec<(u16, String)> {
    let mut set = JoinSet::new();
    for _ in 0..n {
        let app = app.clone();
        set.spawn(async move {
            let (status, body) = sale(&app, pack_id, qty, None).await;
            (status.as_u16(), body["code"].as_str().unwrap_or("").to_string())
        });
    }
    let mut out = Vec::new();
    while let Some(r) = set.join_next().await {
        out.push(r.unwrap());
    }
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_deduction_never_negative() {
    let t = TestApp::new().await;
    let (_drug, pack_id) = seed_pack(&t.app, 1, 10, "B-CONC", false).await;

    // 10 个药师同时各拆 3 粒: 只有前 3 单(9 粒)能成功, 其余必须被拒
    let results = concurrent_sales(&t.app, pack_id, 3, 10).await;
    let ok = results.iter().filter(|(s, _)| *s == 201).count();
    let rejected = results
        .iter()
        .filter(|(s, c)| *s == 409 && c == "INSUFFICIENT_STOCK")
        .count();

    assert_eq!(ok, 3, "成功单数应为 3, 实际: {results:?}");
    assert_eq!(rejected, 7, "拒售单数应为 7, 实际: {results:?}");

    let pack = get_pack(&t.app, pack_id).await;
    assert_eq!(pack["remaining_qty"].as_i64().unwrap(), 1, "余量应为 1(绝不为负)");
    assert_eq!(pack["initial_qty"].as_i64().unwrap(), 10);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_exact_boundary() {
    let t = TestApp::new().await;
    let (_drug, pack_id) = seed_pack(&t.app, 1, 10, "B-EDGE", false).await;

    // 3 人同时各拆 4 粒: 2 单成功(8 粒), 1 单失败, 余 2
    let results = concurrent_sales(&t.app, pack_id, 4, 3).await;
    assert_eq!(results.iter().filter(|(s, _)| *s == 201).count(), 2);
    let pack = get_pack(&t.app, pack_id).await;
    assert_eq!(pack["remaining_qty"].as_i64().unwrap(), 2);
}

#[tokio::test]
async fn single_overdraw_rejected() {
    let t = TestApp::new().await;
    let (_drug, pack_id) = seed_pack(&t.app, 1, 10, "B-OVER", false).await;

    // 库存不足时不能销售
    let (status, body) = sale(&t.app, pack_id, 11, None).await;
    assert_eq!(status, 409);
    assert_eq!(body["code"], "INSUFFICIENT_STOCK");

    // 余量原封不动
    let pack = get_pack(&t.app, pack_id).await;
    assert_eq!(pack["remaining_qty"].as_i64().unwrap(), 10);

    // 非法数量
    let (status, _) = sale(&t.app, pack_id, 0, None).await;
    assert_eq!(status, 400);
}
