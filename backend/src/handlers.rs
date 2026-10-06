use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::peripherals::{FailMode, Peripherals};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub periph: Peripherals,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/stores", get(list_stores).post(create_store))
        .route("/api/drugs", get(list_drugs).post(create_drug))
        .route("/api/packs", get(list_packs).post(register_pack))
        .route("/api/packs/{id}", get(get_pack))
        .route("/api/sales", get(list_sales).post(create_sale))
        .route("/api/returns", get(list_returns).post(create_return))
        .route("/api/returns/{id}/resolve", post(resolve_return))
        .route("/api/transfers", get(list_transfers).post(create_transfer))
        .route("/api/recalls", get(list_recalls).post(create_recall))
        .route("/api/recalls/{id}/report", get(recall_report))
        .route("/api/sim/mode", post(set_sim_mode))
        .route("/api/sim/status", get(sim_status))
        .route("/api/sim/jobs", get(sim_jobs))
        .route("/api/sim/receipts", get(sim_receipts))
        .route("/api/seed", post(seed))
        .with_state(state)
}

fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// 柜台终端通过 x-store-id 选择当前门店, 缺省为 1 号店。
fn store_id_from(headers: &HeaderMap) -> i64 {
    headers
        .get("x-store-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(1)
}

async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

/* ---------------- 基础资料 ---------------- */

async fn list_stores(State(s): State<AppState>) -> AppResult<Json<Vec<Store>>> {
    let rows = sqlx::query_as::<_, Store>("SELECT id, code, name FROM stores ORDER BY id")
        .fetch_all(&s.pool)
        .await?;
    Ok(Json(rows))
}

async fn create_store(
    State(s): State<AppState>,
    Json(req): Json<CreateStoreReq>,
) -> AppResult<(StatusCode, Json<Store>)> {
    if req.code.trim().is_empty() || req.name.trim().is_empty() {
        return Err(AppError::bad_request("门店编码与名称不能为空"));
    }
    let r = sqlx::query("INSERT INTO stores(code, name) VALUES (?1, ?2)")
        .bind(&req.code)
        .bind(&req.name)
        .execute(&s.pool)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(Store {
            id: r.last_insert_rowid(),
            code: req.code,
            name: req.name,
        }),
    ))
}

async fn list_drugs(State(s): State<AppState>) -> AppResult<Json<Vec<Drug>>> {
    let rows = sqlx::query_as::<_, Drug>("SELECT id, name, spec, unit_pack FROM drugs ORDER BY id")
        .fetch_all(&s.pool)
        .await?;
    Ok(Json(rows))
}

async fn create_drug(
    State(s): State<AppState>,
    Json(req): Json<CreateDrugReq>,
) -> AppResult<(StatusCode, Json<Drug>)> {
    if req.name.trim().is_empty() {
        return Err(AppError::bad_request("药品名称不能为空"));
    }
    let r = sqlx::query("INSERT INTO drugs(name, spec, unit_pack) VALUES (?1, ?2, ?3)")
        .bind(&req.name)
        .bind(&req.spec)
        .bind(&req.unit_pack)
        .execute(&s.pool)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(Drug {
            id: r.last_insert_rowid(),
            name: req.name,
            spec: req.spec,
            unit_pack: req.unit_pack,
        }),
    ))
}

/* ---------------- 母包装 ---------------- */

const PACK_SELECT: &str = r#"
SELECT p.id, p.store_id, p.drug_id, d.name AS drug_name, d.spec, d.unit_pack,
       p.batch_no, p.opened, p.opened_at, p.opened_expiry, p.pack_expiry,
       p.initial_qty, p.remaining_qty, p.status, p.created_at
FROM parent_packs p JOIN drugs d ON d.id = p.drug_id
"#;

#[derive(Debug, Deserialize)]
struct PackQuery {
    store_id: Option<i64>,
    drug_id: Option<i64>,
    batch_no: Option<String>,
    status: Option<String>,
}

async fn list_packs(
    State(s): State<AppState>,
    Query(q): Query<PackQuery>,
) -> AppResult<Json<Vec<ParentPack>>> {
    let mut sql = String::from(PACK_SELECT);
    let mut conds: Vec<&'static str> = Vec::new();
    if q.store_id.is_some() {
        conds.push("p.store_id = ?");
    }
    if q.drug_id.is_some() {
        conds.push("p.drug_id = ?");
    }
    if q.batch_no.is_some() {
        conds.push("p.batch_no = ?");
    }
    if q.status.is_some() {
        conds.push("p.status = ?");
    }
    if !conds.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conds.join(" AND "));
    }
    sql.push_str(" ORDER BY p.id DESC");

    let mut query = sqlx::query_as::<_, ParentPack>(&sql);
    if let Some(v) = q.store_id {
        query = query.bind(v);
    }
    if let Some(v) = q.drug_id {
        query = query.bind(v);
    }
    if let Some(v) = q.batch_no {
        query = query.bind(v);
    }
    if let Some(v) = q.status {
        query = query.bind(v);
    }
    Ok(Json(query.fetch_all(&s.pool).await?))
}

async fn get_pack(State(s): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<ParentPack>> {
    let sql = format!("{PACK_SELECT} WHERE p.id = ?1");
    sqlx::query_as::<_, ParentPack>(&sql)
        .bind(id)
        .fetch_optional(&s.pool)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::not_found(format!("母包装 {id} 不存在")))
}

async fn register_pack(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterPackReq>,
) -> AppResult<(StatusCode, Json<ParentPack>)> {
    if req.batch_no.trim().is_empty() || req.pack_expiry.trim().is_empty() {
        return Err(AppError::bad_request("批号与包装有效期不能为空"));
    }
    if req.initial_qty <= 0 {
        return Err(AppError::bad_request("登记数量必须大于 0"));
    }
    let store_id = store_id_from(&headers);
    let exists: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM stores WHERE id = ?")
        .bind(store_id)
        .fetch_optional(&s.pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::bad_request(format!("门店 {store_id} 不存在")));
    }
    let opened_at = if req.opened { Some(now_iso()) } else { None };
    let r = sqlx::query(
        "INSERT INTO parent_packs
           (store_id, drug_id, batch_no, opened, opened_at, opened_expiry,
            pack_expiry, initial_qty, remaining_qty, status, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8,'in_stock',?9)",
    )
    .bind(store_id)
    .bind(req.drug_id)
    .bind(&req.batch_no)
    .bind(if req.opened { 1 } else { 0 })
    .bind(opened_at)
    .bind(&req.opened_expiry)
    .bind(&req.pack_expiry)
    .bind(req.initial_qty)
    .bind(now_iso())
    .execute(&s.pool)
    .await?;

    let pack = sqlx::query_as::<_, ParentPack>(&format!("{PACK_SELECT} WHERE p.id = ?1"))
        .bind(r.last_insert_rowid())
        .fetch_one(&s.pool)
        .await?;
    Ok((StatusCode::CREATED, Json(pack)))
}

/* ---------------- 拆零销售(核心) ---------------- */

#[derive(Debug, Deserialize)]
struct SaleQuery {
    parent_pack_id: Option<i64>,
    drug_id: Option<i64>,
    batch_no: Option<String>,
    status: Option<String>,
}

const SALE_SELECT: &str = r#"
SELECT s.id, s.store_id, s.parent_pack_id, s.drug_id, d.name AS drug_name,
       d.spec, d.unit_pack, p.batch_no, s.qty, s.dosage, s.status,
       s.label_json, s.fail_reason, s.created_at
FROM split_sales s
JOIN drugs d ON d.id = s.drug_id
JOIN parent_packs p ON p.id = s.parent_pack_id
"#;

async fn list_sales(
    State(s): State<AppState>,
    Query(q): Query<SaleQuery>,
) -> AppResult<Json<Vec<SplitSale>>> {
    let mut sql = String::from(SALE_SELECT);
    let mut conds: Vec<&'static str> = Vec::new();
    if q.parent_pack_id.is_some() {
        conds.push("s.parent_pack_id = ?");
    }
    if q.drug_id.is_some() {
        conds.push("s.drug_id = ?");
    }
    if q.batch_no.is_some() {
        conds.push("p.batch_no = ?");
    }
    if q.status.is_some() {
        conds.push("s.status = ?");
    }
    if !conds.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conds.join(" AND "));
    }
    sql.push_str(" ORDER BY s.id DESC");

    let mut query = sqlx::query_as::<_, SplitSale>(&sql);
    if let Some(v) = q.parent_pack_id {
        query = query.bind(v);
    }
    if let Some(v) = q.drug_id {
        query = query.bind(v);
    }
    if let Some(v) = q.batch_no {
        query = query.bind(v);
    }
    if let Some(v) = q.status {
        query = query.bind(v);
    }
    Ok(Json(query.fetch_all(&s.pool).await?))
}

fn is_busy(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.message().to_lowercase().contains("busy")
        || db.code().as_deref() == Some("SQLITE_BUSY"))
}

/// 拆零销售流程:
/// 1) IMMEDIATE 事务内校验母盒状态/效期, 条件扣量(WHERE remaining_qty >= qty),
///    写入 pending_print 销售单 —— 并发两人抢同一盒时写锁串行化, 绝不扣成负数。
/// 2) 提交后调用本地打印机与收银机; 任一失败 → 补偿事务回补余量并把单据置 voided,
///    该拆零单永远不会成为 sold。
async fn create_sale(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SaleReq>,
) -> AppResult<(StatusCode, Json<Value>)> {
    if req.qty <= 0 {
        return Err(AppError::bad_request("拆零数量必须大于 0"));
    }
    if let Some(m) = &req.sim_fail {
        if !matches!(m.as_str(), "printer" | "cashier") {
            return Err(AppError::bad_request("sim_fail 只允许 printer / cashier"));
        }
    }
    let store_id = store_id_from(&headers);
    let forced_fail = req.sim_fail.clone();

    // ---- 阶段 1: 扣量 + 建单(BEGIN IMMEDIATE, 与其他写事务串行) ----
    let mut conn = s.pool.acquire().await?;
    let outcome: (i64, i64, i64, Label) = {
        let mut done: Option<(i64, i64, i64, Label)> = None;

        for _attempt in 0..5 {
            let attempt: AppResult<(i64, i64, i64, Label)> = async {
                sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

                let result: AppResult<(i64, i64, i64, Label)> = async {
                    let pack = sqlx::query_as::<_, ParentPack>(&format!(
                        "{PACK_SELECT} WHERE p.id = ?1"
                    ))
                    .bind(req.parent_pack_id)
                    .fetch_optional(&mut *conn)
                    .await?
                    .ok_or_else(|| AppError::not_found("母包装不存在"))?;

                    if pack.store_id != store_id {
                        return Err(AppError::conflict(
                            "WRONG_STORE",
                            "该母包装不属于当前门店, 不能拆零销售",
                        ));
                    }
                    match pack.status.as_str() {
                        "transferred_out" => {
                            return Err(AppError::conflict(
                                "PACK_TRANSFERRED",
                                "母包装已整包调出, 不能销售",
                            ))
                        }
                        "recalled" => {
                            return Err(AppError::conflict(
                                "PACK_RECALLED",
                                "该批次已召回, 母包装冻结, 不能销售",
                            ))
                        }
                        _ => {}
                    }
                    let effective_expiry =
                        pack.opened_expiry.clone().unwrap_or_else(|| pack.pack_expiry.clone());
                    let today = chrono::Local::now().date_naive();
                    let date_part = effective_expiry.get(0..10).unwrap_or(&effective_expiry);
                    if let Ok(exp) =
                        chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d")
                    {
                        if exp < today {
                            return Err(AppError::conflict(
                                "EXPIRED",
                                format!("药品已过有效期({effective_expiry}), 不得拆零销售"),
                            ));
                        }
                    }
                    if pack.remaining_qty < req.qty {
                        return Err(AppError::conflict(
                            "INSUFFICIENT_STOCK",
                            format!(
                                "库存不足: 申请 {} {}, 母盒剩余 {} {}",
                                req.qty, pack.unit_pack, pack.remaining_qty, pack.unit_pack
                            ),
                        ));
                    }

                    // 条件扣量: 双重保险。CHECK(remaining_qty >= 0) 是第三重保险。
                    let upd = sqlx::query(
                        "UPDATE parent_packs
                            SET remaining_qty = remaining_qty - ?1,
                                opened = 1,
                                opened_at = COALESCE(opened_at, ?2)
                          WHERE id = ?3 AND status = 'in_stock' AND remaining_qty >= ?1",
                    )
                    .bind(req.qty)
                    .bind(now_iso())
                    .bind(pack.id)
                    .execute(&mut *conn)
                    .await?;
                    if upd.rows_affected() != 1 {
                        return Err(AppError::conflict(
                            "INSUFFICIENT_STOCK",
                            "库存不足或母盒状态已变化, 请刷新后重试",
                        ));
                    }

                    let ins = sqlx::query(
                        "INSERT INTO split_sales
                           (store_id, parent_pack_id, drug_id, qty, dosage, status, created_at)
                         VALUES (?1,?2,?3,?4,?5,'pending_print',?6)",
                    )
                    .bind(store_id)
                    .bind(pack.id)
                    .bind(pack.drug_id)
                    .bind(req.qty)
                    .bind(&req.dosage)
                    .bind(now_iso())
                    .execute(&mut *conn)
                    .await?;
                    let sale_id = ins.last_insert_rowid();

                    let store_name: String =
                        sqlx::query_scalar("SELECT name FROM stores WHERE id = ?1")
                            .bind(store_id)
                            .fetch_one(&mut *conn)
                            .await?;
                    let trace_code = format!("RX-{sale_id:07}");
                    let label = Label {
                        trace_code: trace_code.clone(),
                        sale_id,
                        parent_pack_id: pack.id,
                        store_name,
                        drug_name: pack.drug_name.clone(),
                        spec: pack.spec.clone(),
                        batch_no: pack.batch_no.clone(),
                        qty: req.qty,
                        unit_pack: pack.unit_pack.clone(),
                        dosage: req.dosage.clone(),
                        expiry: effective_expiry.clone(),
                        pack_expiry: pack.pack_expiry.clone(),
                        opened_expiry: pack.opened_expiry.clone(),
                        packed_at: now_iso(),
                    };
                    sqlx::query("UPDATE split_sales SET label_json = ?1 WHERE id = ?2")
                        .bind(serde_json::to_string(&label).unwrap_or_default())
                        .bind(sale_id)
                        .execute(&mut *conn)
                    .await?;

                    let remaining: (i64,) =
                        sqlx::query_as("SELECT remaining_qty FROM parent_packs WHERE id = ?1")
                            .bind(pack.id)
                            .fetch_one(&mut *conn)
                            .await?;

                    Ok((sale_id, pack.id, remaining.0, label))
                }
                .await;

                match result {
                    Ok(v) => {
                        sqlx::query("COMMIT").execute(&mut *conn).await?;
                        Ok(v)
                    }
                    Err(e) => {
                        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                        Err(e)
                    }
                }
            }
            .await;

            match attempt {
                Ok(v) => {
                    done = Some(v);
                    break;
                }
                // 写锁竞争: 全新事务重试, 库存判定会在锁释放后重新执行
                Err(AppError::Db(ref dbe)) if is_busy(dbe) => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(e) => return Err(e),
            }
        }

        match done {
            Some(v) => v,
            None => return Err(AppError::Other(anyhow::anyhow!("数据库忙, 重试后仍未取得写锁"))),
        }
    };
    let (sale_id, pack_id, _remaining_after_hold, label) = outcome;

    // ---- 阶段 2: 本地外设(打印机 → 收银机), 全部成功才算已售出 ----
    let fail = forced_fail.unwrap_or_else(|| "none".into());
    let print_err = if fail == "printer" {
        Some("打印机离线(模拟注入)".to_string())
    } else {
        s.periph.print_label(&label, &now_iso()).err()
    };

    if let Some(reason) = print_err {
        compensate_failure(&s.pool, sale_id, pack_id, req.qty, &reason, "PRINT_FAILED").await?;
        let remaining = remaining_of(&s.pool, pack_id).await?;
        return Err(AppError::conflict(
            "PRINT_FAILED",
            format!("标签打印失败({reason}), 销售未成立, 余量已回补为 {remaining}"),
        ));
    }

    let charge_err = if fail == "cashier" {
        Some("收银终端无应答(模拟注入)".to_string())
    } else {
        s.periph.charge(&label, sale_id, &now_iso()).err()
    };
    if let Some(reason) = charge_err {
        compensate_failure(&s.pool, sale_id, pack_id, req.qty, &reason, "CASHIER_FAILED").await?;
        let remaining = remaining_of(&s.pool, pack_id).await?;
        return Err(AppError::conflict(
            "CASHIER_FAILED",
            format!("收银失败({reason}), 销售未成立, 余量已回补为 {remaining}"),
        ));
    }

    // 外设全部成功 → 记为已售出
    sqlx::query("UPDATE split_sales SET status = 'sold' WHERE id = ?1")
        .bind(sale_id)
        .execute(&s.pool)
        .await?;
    let remaining = remaining_of(&s.pool, pack_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "sale_id": sale_id,
            "trace_code": label.trace_code,
            "parent_pack_id": pack_id,
            "qty": req.qty,
            "remaining_after": remaining,
            "label": label,
        })),
    ))
}

async fn remaining_of(pool: &SqlitePool, pack_id: i64) -> AppResult<i64> {
    let r: (i64,) = sqlx::query_as("SELECT remaining_qty FROM parent_packs WHERE id = ?1")
        .bind(pack_id)
        .fetch_one(pool)
        .await?;
    Ok(r.0)
}

/// 打印/收银失败的补偿: 回补母盒余量, 单据置 voided(永不售出)。
async fn compensate_failure(
    pool: &SqlitePool,
    sale_id: i64,
    pack_id: i64,
    qty: i64,
    reason: &str,
    code: &str,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE parent_packs SET remaining_qty = remaining_qty + ?1 WHERE id = ?2",
    )
    .bind(qty)
    .bind(pack_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE split_sales SET status = 'voided', fail_reason = ?1 WHERE id = ?2",
    )
    .bind(format!("{code}: {reason}"))
    .bind(sale_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/* ---------------- 顾客退药: 待处理隔离库存 ---------------- */

async fn list_returns(
    State(s): State<AppState>,
    Query(q): Query<SaleQuery>,
) -> AppResult<Json<Vec<ReturnedItem>>> {
    // 复用 status 过滤; drug_id / batch_no 同样适用
    let mut sql = String::from(
        r#"SELECT r.id, r.store_id, r.drug_id, d.name AS drug_name, r.batch_no,
                  r.qty, r.reason, r.status, '' AS resolution, r.created_at
             FROM returned_stock r JOIN drugs d ON d.id = r.drug_id"#,
    );
    let mut conds: Vec<&'static str> = Vec::new();
    if q.drug_id.is_some() {
        conds.push("r.drug_id = ?");
    }
    if q.batch_no.is_some() {
        conds.push("r.batch_no = ?");
    }
    if q.status.is_some() {
        conds.push("r.status = ?");
    }
    if !conds.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conds.join(" AND "));
    }
    sql.push_str(" ORDER BY r.id DESC");
    let mut query = sqlx::query_as::<_, ReturnedItem>(&sql);
    if let Some(v) = q.drug_id {
        query = query.bind(v);
    }
    if let Some(v) = q.batch_no {
        query = query.bind(v);
    }
    if let Some(v) = q.status {
        query = query.bind(v);
    }
    Ok(Json(query.fetch_all(&s.pool).await?))
}

async fn create_return(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReturnReq>,
) -> AppResult<(StatusCode, Json<ReturnedItem>)> {
    if req.qty <= 0 {
        return Err(AppError::bad_request("退药数量必须大于 0"));
    }
    let store_id = store_id_from(&headers);
    let ts = now_iso();
    // 注意: 退药只写入 returned_stock, 完全不触碰 parent_packs.remaining_qty。
    let r = sqlx::query(
        "INSERT INTO returned_stock (store_id, drug_id, batch_no, qty, reason, status, created_at)
         VALUES (?1,?2,?3,?4,?5,'pending',?6)",
    )
    .bind(store_id)
    .bind(req.drug_id)
    .bind(&req.batch_no)
    .bind(req.qty)
    .bind(&req.reason)
    .bind(&ts)
    .execute(&s.pool)
    .await?;
    let item = sqlx::query_as::<_, ReturnedItem>(
        r#"SELECT r.id, r.store_id, r.drug_id, d.name AS drug_name, r.batch_no,
                  r.qty, r.reason, r.status, '' AS resolution, r.created_at
             FROM returned_stock r JOIN drugs d ON d.id = r.drug_id
            WHERE r.id = ?1"#,
    )
    .bind(r.last_insert_rowid())
    .fetch_one(&s.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(item)))
}

async fn resolve_return(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<ResolveReturnReq>,
) -> AppResult<Json<Value>> {
    // 待处理退药经质量确认后只能"隔离/销毁/转报损", 不允许重新上架到母盒。
    let r = sqlx::query("UPDATE returned_stock SET status = 'quarantined' WHERE id = ?1 AND status = 'pending'")
        .bind(id)
        .execute(&s.pool)
        .await?;
    if r.rows_affected() != 1 {
        return Err(AppError::not_found("待处理退药记录不存在或已处理"));
    }
    Ok(Json(json!({ "id": id, "status": "quarantined", "resolution": req.resolution })))
}

/* ---------------- 门店调拨: 仅限未拆零整包 ---------------- */

async fn list_transfers(State(s): State<AppState>) -> AppResult<Json<Vec<Transfer>>> {
    let rows = sqlx::query_as::<_, Transfer>(
        r#"SELECT t.id, t.parent_pack_id, t.drug_id, d.name AS drug_name, t.batch_no, t.qty,
                  t.from_store_id, f.name AS from_store_name,
                  t.to_store_id, tt.name AS to_store_name, t.created_at
             FROM transfers t
             JOIN drugs d ON d.id = t.drug_id
             JOIN stores f ON f.id = t.from_store_id
             JOIN stores tt ON tt.id = t.to_store_id
            ORDER BY t.id DESC"#,
    )
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(rows))
}

async fn create_transfer(
    State(s): State<AppState>,
    Json(req): Json<TransferReq>,
) -> AppResult<(StatusCode, Json<Transfer>)> {
    if req.to_store_id <= 0 {
        return Err(AppError::bad_request("调入门店无效"));
    }
    let mut tx = s.pool.begin().await?;
    let pack = sqlx::query_as::<_, ParentPack>(&format!("{PACK_SELECT} WHERE p.id = ?1"))
        .bind(req.parent_pack_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::not_found("母包装不存在"))?;

    // 规则: 只允许调拨未拆零的整包 —— 未开封且数量等于初始数量。
    if pack.opened != 0 || pack.remaining_qty != pack.initial_qty {
        return Err(AppError::conflict(
            "NOT_SEALED_PACK",
            format!(
                "仅允许调拨未拆零整包: 该母盒{}开封, 剩余 {}/{} {}",
                if pack.opened != 0 { "已" } else { "未" },
                pack.remaining_qty, pack.initial_qty, pack.unit_pack
            ),
        ));
    }
    if pack.status != "in_stock" {
        return Err(AppError::conflict("PACK_UNAVAILABLE", "母包装当前状态不允许调拨"));
    }
    if pack.store_id == req.to_store_id {
        return Err(AppError::bad_request("调入与调出门店不能相同"));
    }
    let target: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM stores WHERE id = ?")
        .bind(req.to_store_id)
        .fetch_optional(&mut *tx)
        .await?;
    if target.is_none() {
        return Err(AppError::bad_request("调入门店不存在"));
    }

    let ts = now_iso();
    // 整包随店移动: 换门店, 保持 in_stock(未开封、数量不变)
    sqlx::query(
        "UPDATE parent_packs SET store_id = ?1 WHERE id = ?2",
    )
    .bind(req.to_store_id)
    .bind(pack.id)
    .execute(&mut *tx)
    .await?;
    let r = sqlx::query(
        "INSERT INTO transfers (parent_pack_id, drug_id, batch_no, qty, from_store_id, to_store_id, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
    )
    .bind(pack.id)
    .bind(pack.drug_id)
    .bind(&pack.batch_no)
    .bind(pack.initial_qty)
    .bind(pack.store_id)
    .bind(req.to_store_id)
    .bind(&ts)
    .execute(&mut *tx)
    .await?;

    let transfer = sqlx::query_as::<_, Transfer>(
        r#"SELECT t.id, t.parent_pack_id, t.drug_id, d.name AS drug_name, t.batch_no, t.qty,
                  t.from_store_id, f.name AS from_store_name,
                  t.to_store_id, tt.name AS to_store_name, t.created_at
             FROM transfers t
             JOIN drugs d ON d.id = t.drug_id
             JOIN stores f ON f.id = t.from_store_id
             JOIN stores tt ON tt.id = t.to_store_id
            WHERE t.id = ?1"#,
    )
    .bind(r.last_insert_rowid())
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(transfer)))
}

/* ---------------- 批次召回 ---------------- */

async fn list_recalls(State(s): State<AppState>) -> AppResult<Json<Vec<Recall>>> {
    let rows = sqlx::query_as::<_, Recall>(
        r#"SELECT r.id, r.drug_id, d.name AS drug_name, r.batch_no, r.reason, r.created_at
             FROM recalls r JOIN drugs d ON d.id = r.drug_id
            ORDER BY r.id DESC"#,
    )
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(rows))
}

async fn create_recall(
    State(s): State<AppState>,
    Json(req): Json<RecallReq>,
) -> AppResult<(StatusCode, Json<Recall>)> {
    let mut tx = s.pool.begin().await?;
    let r = sqlx::query(
        "INSERT INTO recalls (drug_id, batch_no, reason, created_at) VALUES (?1,?2,?3,?4)
         ON CONFLICT(drug_id, batch_no) DO UPDATE SET reason = excluded.reason",
    )
    .bind(req.drug_id)
    .bind(&req.batch_no)
    .bind(&req.reason)
    .bind(now_iso())
    .execute(&mut *tx)
    .await?;
    // 冻结该药品+批号的全部母包装(含已调拨到其他门店的)
    sqlx::query(
        "UPDATE parent_packs SET status = 'recalled'
          WHERE drug_id = ?1 AND batch_no = ?2 AND status != 'recalled'",
    )
    .bind(req.drug_id)
    .bind(&req.batch_no)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let recall = sqlx::query_as::<_, Recall>(
        r#"SELECT r.id, r.drug_id, d.name AS drug_name, r.batch_no, r.reason, r.created_at
             FROM recalls r JOIN drugs d ON d.id = r.drug_id
            WHERE r.id = ?1"#,
    )
    .bind(r.last_insert_rowid())
    .fetch_one(&s.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(recall)))
}

/// 召回报告: 相关母包装(全门店, 含已调出) + 全部已售出的拆零记录 + 待处理退药。
async fn recall_report(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let recall = sqlx::query_as::<_, Recall>(
        r#"SELECT r.id, r.drug_id, d.name AS drug_name, r.batch_no, r.reason, r.created_at
             FROM recalls r JOIN drugs d ON d.id = r.drug_id WHERE r.id = ?1"#,
    )
    .bind(id)
    .fetch_optional(&s.pool)
    .await?
    .ok_or_else(|| AppError::not_found("召回记录不存在"))?;

    let packs = sqlx::query_as::<_, ParentPack>(&format!(
        "{PACK_SELECT} WHERE p.drug_id = ?1 AND p.batch_no = ?2 ORDER BY p.id"
    ))
    .bind(recall.drug_id)
    .bind(&recall.batch_no)
    .fetch_all(&s.pool)
    .await?;

    let sold = sqlx::query_as::<_, SplitSale>(&format!(
        "{SALE_SELECT} WHERE s.drug_id = ?1 AND p.batch_no = ?2 AND s.status = 'sold' ORDER BY s.id"
    ))
    .bind(recall.drug_id)
    .bind(&recall.batch_no)
    .fetch_all(&s.pool)
    .await?;
    let total_sold: i64 = sold.iter().map(|x| x.qty).sum();

    let returned = sqlx::query_as::<_, ReturnedItem>(
        r#"SELECT r.id, r.store_id, r.drug_id, d.name AS drug_name, r.batch_no,
                  r.qty, r.reason, r.status, '' AS resolution, r.created_at
             FROM returned_stock r JOIN drugs d ON d.id = r.drug_id
            WHERE r.drug_id = ?1 AND r.batch_no = ?2 ORDER BY r.id"#,
    )
    .bind(recall.drug_id)
    .bind(&recall.batch_no)
    .fetch_all(&s.pool)
    .await?;

    Ok(Json(json!({
        "recall": recall,
        "parent_packs": packs,
        "sold_splits": sold,
        "total_sold_qty": total_sold,
        "returned_pending": returned,
    })))
}

/* ---------------- 外设模拟控制 ---------------- */

async fn set_sim_mode(
    State(s): State<AppState>,
    Json(req): Json<SimModeReq>,
) -> AppResult<Json<Value>> {
    let mode = match req.mode.as_str() {
        "none" => FailMode::None,
        "printer" => FailMode::Printer,
        "cashier" => FailMode::Cashier,
        other => return Err(AppError::bad_request(format!("未知模拟模式 {other}"))),
    };
    s.periph.set_fail_mode(mode);
    Ok(Json(json!({ "fail_mode": req.mode })))
}

async fn sim_status(State(s): State<AppState>) -> Json<Value> {
    Json(serde_json::to_value(s.periph.status()).unwrap_or(json!({})))
}

async fn sim_jobs(State(s): State<AppState>) -> Json<Value> {
    Json(json!({ "jobs": s.periph.jobs() }))
}

async fn sim_receipts(State(s): State<AppState>) -> Json<Value> {
    Json(json!({ "receipts": s.periph.receipts() }))
}

/* ---------------- 演示数据 ---------------- */

async fn seed(State(s): State<AppState>) -> AppResult<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    sqlx::query("INSERT OR IGNORE INTO stores(id, code, name) VALUES (1,'S001','中心店'),(2,'S002','滨江店')")
        .execute(&mut *tx)
        .await?;
    let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM drugs").fetch_one(&mut *tx).await?;
    if n.0 == 0 {
        sqlx::query(
            "INSERT INTO drugs(id, name, spec, unit_pack) VALUES
              (1,'阿莫西林胶囊','0.25g*24粒','粒'),
              (2,'布洛芬片','0.1g*100片','片'),
              (3,'硝苯地平控释片','30mg*7片','片')",
        )
        .execute(&mut *tx)
        .await?;
        let ts = now_iso();
        sqlx::query(
            "INSERT INTO parent_packs(store_id,drug_id,batch_no,opened,opened_expiry,pack_expiry,initial_qty,remaining_qty,status,created_at)
             VALUES (1,1,'AMX20260801',0,NULL,'2028-07-31',24,24,'in_stock',?1),
                    (1,2,'BLF20260915',1,?2,'2028-08-31',100,100,'in_stock',?1),
                    (2,3,'XB20260601',0,NULL,'2027-05-31',7,7,'in_stock',?1)",
        )
        .bind(ts)
        .bind("2026-12-31")
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "seeded": true })))
}
