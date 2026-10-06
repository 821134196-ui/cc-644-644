// 规则验证 3: 顾客退回的散装药进入待处理库存, 不增加母盒余量。
mod common;

use common::*;

#[tokio::test]
async fn returned_loose_drug_is_quarantined_from_parent() {
    let t = TestApp::new().await;
    let (drug_id, pack_id) = seed_pack(&t.app, 1, 24, "B-RET", false).await;

    // 先正常拆零 6 粒, 母盒余 18
    let (st, _) = sale(&t.app, pack_id, 6, None).await;
    assert_eq!(st, 201);
    let pack = get_pack(&t.app, pack_id).await;
    assert_eq!(pack["remaining_qty"].as_i64().unwrap(), 18);

    // 顾客退回 3 粒散装药(批号能对上)
    let (st, ret) = request(
        &t.app,
        "POST",
        "/api/returns",
        Some(1),
        Some(serde_json::json!({
            "drug_id": drug_id,
            "batch_no": "B-RET",
            "qty": 3,
            "reason": "顾客用药后不适退回"
        })),
    )
    .await;
    assert_eq!(st, 201, "{ret}");
    assert_eq!(ret["status"], "pending");
    let return_id = ret["id"].as_i64().unwrap();

    // 母盒余量不得变化: 退药不回增母盒
    let pack = get_pack(&t.app, pack_id).await;
    assert_eq!(pack["remaining_qty"].as_i64().unwrap(), 18, "退药不得增加母盒余量");

    // 待处理库存列表能看到
    let (st, pending) = request(&t.app, "GET", "/api/returns?status=pending", None, None).await;
    assert_eq!(st, 200);
    assert_eq!(pending.as_array().unwrap().len(), 1);
    assert_eq!(pending[0]["qty"], 3);

    // 药师处理: 隔离/报损, 依然不回增母盒
    let (st, _) = request(
        &t.app,
        "POST",
        &format!("/api/returns/{return_id}/resolve"),
        None,
        Some(serde_json::json!({"resolution": "质量复核后隔离销毁, 不上架"})),
    )
    .await;
    assert_eq!(st, 200);

    let (_, pending) = request(&t.app, "GET", "/api/returns?status=pending", None, None).await;
    assert_eq!(pending.as_array().unwrap().len(), 0);

    let pack = get_pack(&t.app, pack_id).await;
    assert_eq!(pack["remaining_qty"].as_i64().unwrap(), 18, "处理退药后母盒余量仍不变");
}

#[tokio::test]
async fn returned_without_batch_still_isolated() {
    let t = TestApp::new().await;
    let (drug_id, pack_id) = seed_pack(&t.app, 1, 10, "B-RET2", false).await;
    let (st, _) = sale(&t.app, pack_id, 2, None).await;
    assert_eq!(st, 201);

    // 顾客说不清批号也允许登记, 同样进待处理
    let (st, ret) = request(
        &t.app,
        "POST",
        "/api/returns",
        Some(1),
        Some(serde_json::json!({"drug_id": drug_id, "batch_no": "", "qty": 1, "reason": "散装无法辨认批号"})),
    )
    .await;
    assert_eq!(st, 201, "{ret}");

    let pack = get_pack(&t.app, pack_id).await;
    assert_eq!(pack["remaining_qty"].as_i64().unwrap(), 8);
}
