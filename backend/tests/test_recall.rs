// 规则验证 4: 批次召回 —— 找到相关母包装(跨门店)与全部拆零销售记录, 并冻结库存。
mod common;

use common::*;

async fn second_pack_same_batch(app: &axum::Router, drug_id: i64, batch: &str, store_id: i64, qty: i64) -> i64 {
    let (st, pack) = request(
        app,
        "POST",
        "/api/packs",
        Some(store_id),
        Some(serde_json::json!({
            "drug_id": drug_id,
            "batch_no": batch,
            "pack_expiry": "2028-12-31",
            "initial_qty": qty,
            "opened": false,
        })),
    )
    .await;
    assert_eq!(st, 201, "{pack}");
    pack["id"].as_i64().unwrap()
}

#[tokio::test]
async fn recall_finds_packs_and_all_sold_splits() {
    let t = TestApp::new().await;
    let (drug_id, pack1) = seed_pack(&t.app, 2, 24, "B-RECALL", false).await;
    // 2 号店同批次另一盒
    let pack2 = second_pack_same_batch(&t.app, drug_id, "B-RECALL", 2, 12).await;

    // 1 号店卖出两笔 4 + 6, 另有一笔打印失败(voided, 不算售出)
    let (st, _) = sale(&t.app, pack1, 4, None).await;
    assert_eq!(st, 201);
    let (st, _) = sale(&t.app, pack1, 6, None).await;
    assert_eq!(st, 201);
    let (st, _) = sale(&t.app, pack1, 3, Some("printer")).await;
    assert_eq!(st, 409);
    // 2 号店卖出 5
    let (st, body) = request(
        &t.app,
        "POST",
        "/api/sales",
        Some(2),
        Some(serde_json::json!({"parent_pack_id": pack2, "qty": 5, "dosage": "口服"})),
    )
    .await;
    assert_eq!(st, 201, "{body}");

    // 1 号店有一批同批号待处理退药
    let (st, _) = request(
        &t.app,
        "POST",
        "/api/returns",
        Some(1),
        Some(serde_json::json!({"drug_id": drug_id, "batch_no": "B-RECALL", "qty": 2, "reason": "召回自查退回"})),
    )
    .await;
    assert_eq!(st, 201);

    // 发起召回
    let (st, recall) = request(
        &t.app,
        "POST",
        "/api/recalls",
        None,
        Some(serde_json::json!({"drug_id": drug_id, "batch_no": "B-RECALL", "reason": "溶出度不合格"})),
    )
    .await;
    assert_eq!(st, 201, "{recall}");
    let recall_id = recall["id"].as_i64().unwrap();

    let (st, report) =
        request(&t.app, "GET", &format!("/api/recalls/{recall_id}/report"), None, None).await;
    assert_eq!(st, 200, "{report}");

    // 找到全部相关母包装(2 盒, 跨 2 个门店), 且状态均冻结
    let packs = report["parent_packs"].as_array().unwrap();
    assert_eq!(packs.len(), 2);
    assert!(packs.iter().all(|p| p["status"] == "recalled"));
    let stores: Vec<i64> = packs.iter().map(|p| p["store_id"].as_i64().unwrap()).collect();
    assert!(stores.contains(&1) && stores.contains(&2));

    // 全部已售出拆零记录: 4 + 6 + 5 = 3 笔, voided 单据不得出现
    let sold = report["sold_splits"].as_array().unwrap();
    assert_eq!(sold.len(), 3, "应只含 sold 单据: {sold:?}");
    assert!(sold.iter().all(|s| s["status"] == "sold"));
    assert_eq!(report["total_sold_qty"].as_i64().unwrap(), 15);
    assert!(sold.iter().all(|s| s["batch_no"] == "B-RECALL"));
    // 每条记录都能追溯到母包装
    let parent_ids: Vec<i64> = sold.iter().map(|s| s["parent_pack_id"].as_i64().unwrap()).collect();
    assert!(parent_ids.contains(&pack1) && parent_ids.contains(&pack2));

    // 待处理退药同样在召回视野内
    assert_eq!(report["returned_pending"].as_array().unwrap().len(), 1);

    // 召回后再尝试销售被冻结的母盒 → 拒绝
    let (st, body) = sale(&t.app, pack1, 1, None).await;
    assert_eq!(st, 409);
    assert_eq!(body["code"], "PACK_RECALLED");

    // 余量维持召回前的值(1 号店 24-4-6=14; 失败单已回补; 2 号店 12-5=7)
    let p1 = get_pack(&t.app, pack1).await;
    assert_eq!(p1["remaining_qty"].as_i64().unwrap(), 14);
    let p2 = get_pack(&t.app, pack2).await;
    assert_eq!(p2["remaining_qty"].as_i64().unwrap(), 7);
}

#[tokio::test]
async fn only_sealed_packs_can_transfer_and_recall_follows_the_pack() {
    let t = TestApp::new().await;
    let (drug_id, opened_pack) = seed_pack(&t.app, 2, 24, "B-MOVE", true).await;
    // 已开封盒卖 4 粒
    let (st, _) = sale(&t.app, opened_pack, 4, None).await;
    assert_eq!(st, 201);

    // 已拆零的盒不允许调拨
    let (st, body) = request(
        &t.app,
        "POST",
        "/api/transfers",
        None,
        Some(serde_json::json!({"parent_pack_id": opened_pack, "to_store_id": 2})),
    )
    .await;
    assert_eq!(st, 409);
    assert_eq!(body["code"], "NOT_SEALED_PACK");

    // 未拆零整包允许调拨到 2 号店
    let sealed = second_pack_same_batch(&t.app, drug_id, "B-MOVE", 1, 24).await;
    let (st, tr) = request(
        &t.app,
        "POST",
        "/api/transfers",
        None,
        Some(serde_json::json!({"parent_pack_id": sealed, "to_store_id": 2})),
    )
    .await;
    assert_eq!(st, 201, "{tr}");
    assert_eq!(tr["qty"], 24);

    let moved = get_pack(&t.app, sealed).await;
    assert_eq!(moved["store_id"], 2);
    assert_eq!(moved["opened"].as_i64().unwrap(), 0);
    assert_eq!(moved["remaining_qty"].as_i64().unwrap(), 24);

    // 1 号店不能再卖, 2 号店可以
    let (st, body) = sale(&t.app, sealed, 2, None).await;
    assert_eq!(st, 409);
    assert_eq!(body["code"], "WRONG_STORE");
    let (st, body) = request(
        &t.app,
        "POST",
        "/api/sales",
        Some(2),
        Some(serde_json::json!({"parent_pack_id": sealed, "qty": 2, "dosage": "口服"})),
    )
    .await;
    assert_eq!(st, 201, "{body}");

    // 召回依然能定位到已调到 2 号店的整包及其拆零记录
    let (st, _) = request(
        &t.app,
        "POST",
        "/api/recalls",
        None,
        Some(serde_json::json!({"drug_id": drug_id, "batch_no": "B-MOVE", "reason": "例行召回"})),
    )
    .await;
    assert_eq!(st, 201);
    let (_, recalls) = request(&t.app, "GET", "/api/recalls", None, None).await;
    let rid = recalls[0]["id"].as_i64().unwrap();
    let (_, report) = request(&t.app, "GET", &format!("/api/recalls/{rid}/report"), None, None).await;
    let pack_ids: Vec<i64> = report["parent_packs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_i64().unwrap())
        .collect();
    assert!(pack_ids.contains(&sealed));
    assert!(pack_ids.contains(&opened_pack));
    // 2 号店拆出的 2 粒也在销售记录里
    assert!(report["sold_splits"].as_array().unwrap().iter().any(
        |s| s["parent_pack_id"].as_i64() == Some(sealed) && s["qty"].as_i64() == Some(2)
    ));
}
