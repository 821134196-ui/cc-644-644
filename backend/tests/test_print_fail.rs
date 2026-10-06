// 规则验证 2: 打印失败(以及收银失败)的拆零单不能记为已售出, 余量必须回补。
mod common;

use common::*;

async fn sales_for(app: &axum::Router, pack_id: i64) -> serde_json::Value {
    let (st, v) = request(
        app,
        "GET",
        &format!("/api/sales?parent_pack_id={pack_id}"),
        None,
        None,
    )
    .await;
    assert_eq!(st, 200);
    v
}

#[tokio::test]
async fn printer_failure_voids_sale_and_restores_qty() {
    let t = TestApp::new().await;
    let (_drug, pack_id) = seed_pack(&t.app, 1, 24, "B-PRINT", false).await;

    // 注入打印机故障
    let (st, body) = sale(&t.app, pack_id, 5, Some("printer")).await;
    assert_eq!(st, 409, "打印失败应返回冲突: {body}");
    assert_eq!(body["code"], "PRINT_FAILED");

    // 余量已回补
    let pack = get_pack(&t.app, pack_id).await;
    assert_eq!(pack["remaining_qty"].as_i64().unwrap(), 24);

    // 单据存在但为 voided, 没有任何 sold
    let sales = sales_for(&t.app, pack_id).await;
    assert_eq!(sales.as_array().unwrap().len(), 1);
    assert_eq!(sales[0]["status"], "voided");
    assert!(sales[0]["fail_reason"].as_str().unwrap().contains("PRINT_FAILED"));

    // 打印机没有留下标签作业, 收银也未发生
    let (st, jobs) = request(&t.app, "GET", "/api/sim/jobs", None, None).await;
    assert_eq!(st, 200);
    assert_eq!(jobs["jobs"].as_array().unwrap().len(), 0);
    let (_, receipts) = request(&t.app, "GET", "/api/sim/receipts", None, None).await;
    assert_eq!(receipts["receipts"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn cashier_failure_also_not_sold() {
    let t = TestApp::new().await;
    let (_drug, pack_id) = seed_pack(&t.app, 1, 24, "B-CASH", false).await;

    // 标签打出来了但收银失败: 仍然不能算售出
    let (st, body) = sale(&t.app, pack_id, 7, Some("cashier")).await;
    assert_eq!(st, 409);
    assert_eq!(body["code"], "CASHIER_FAILED");

    let pack = get_pack(&t.app, pack_id).await;
    assert_eq!(pack["remaining_qty"].as_i64().unwrap(), 24, "收银失败余量也必须回补");

    let sales = sales_for(&t.app, pack_id).await;
    assert_eq!(sales[0]["status"], "voided");

    // 标签确实打印过 1 张, 但没有收银小票
    let (_, jobs) = request(&t.app, "GET", "/api/sim/jobs", None, None).await;
    assert_eq!(jobs["jobs"].as_array().unwrap().len(), 1);
    let (_, receipts) = request(&t.app, "GET", "/api/sim/receipts", None, None).await;
    assert_eq!(receipts["receipts"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn after_failure_normal_sale_succeeds() {
    let t = TestApp::new().await;
    let (_drug, pack_id) = seed_pack(&t.app, 1, 24, "B-RECOV", false).await;

    let (st, _) = sale(&t.app, pack_id, 5, Some("printer")).await;
    assert_eq!(st, 409);

    // 设备恢复(无注入即正常): 同一盒药可正常售出, 标签含药名/批号/用法/有效期
    let (st, body) = sale(&t.app, pack_id, 5, None).await;
    assert_eq!(st, 201, "正常销售应成功: {body}");
    assert_eq!(body["remaining_after"], 19);
    let label = &body["label"];
    assert_eq!(label["drug_name"], "测试胶囊");
    assert_eq!(label["batch_no"], "B-RECOV");
    assert_eq!(label["dosage"], "口服 一次1粒 一日3次");
    assert!(label["expiry"].is_string());
    assert!(label["trace_code"].as_str().unwrap().starts_with("RX-"));

    let pack = get_pack(&t.app, pack_id).await;
    assert_eq!(pack["remaining_qty"].as_i64().unwrap(), 19);

    // 该盒 1 张作废单 + 1 张已售单
    let sales = sales_for(&t.app, pack_id).await;
    let statuses: Vec<&str> = sales.as_array().unwrap().iter().map(|s| s["status"].as_str().unwrap()).collect();
    assert!(statuses.contains(&"sold"));
    assert!(statuses.contains(&"voided"));
    assert_eq!(statuses.iter().filter(|s| **s == "sold").count(), 1);
}
