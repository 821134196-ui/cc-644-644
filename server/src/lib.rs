use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

pub mod db;
pub mod mock;

use mock::{MockPos, MockPrinter};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub printer: Arc<MockPrinter>,
    pub pos: Arc<MockPos>,
}

pub async fn make_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;
    db::migrate(&pool).await?;
    Ok(pool)
}

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/api/meta", get(meta))
        .route("/api/mothers", get(list_mothers).post(create_mother))
        .route("/api/mothers/{id}", get(get_mother))
        .route("/api/repacks", get(list_repacks).post(create_repack))
        .route("/api/returns", get(list_returns).post(create_return))
        .route("/api/transfers", get(list_transfers).post(create_transfer))
        .route("/api/recalls/lookup", get(recall_lookup))
        .route("/api/recalls", post(execute_recall).get(list_recalls))
        // 本地模拟：打印机与收银
        .route("/api/mock/printer/config", post(set_printer_fail).get(printer_state))
        .route("/api/mock/printer/jobs", get(printer_jobs))
        .route("/api/mock/pos/sales", get(pos_sales))
        .with_state(state)
}

// ---------- 错误处理 ----------

pub struct ApiErr(anyhow::Error);
impl From<anyhow::Error> for ApiErr {
    fn from(e: anyhow::Error) -> Self {
        ApiErr(e)
    }
}
impl From<sqlx::Error> for ApiErr {
    fn from(e: sqlx::Error) -> Self {
        ApiErr(e.into())
    }
}
impl IntoResponse for ApiErr {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": self.0.to_string()}))).into_response()
    }
}
type ApiResult<T> = Result<T, ApiErr>;
fn bad(msg: impl Into<String>) -> ApiErr {
    ApiErr(anyhow::anyhow!(msg.into()))
}

// ---------- DTO ----------

#[derive(Serialize)]
pub struct MotherRow {
    pub id: i64,
    pub store_id: i64,
    pub store_name: String,
    pub drug_id: i64,
    pub drug_name: String,
    pub spec: String,
    pub usage_text: String,
    pub unit: String,
    pub batch_no: String,
    pub opened_at: String,
    pub expiry_date: String,
    pub total_qty: i64,
    pub remaining_qty: i64,
    pub status: String,
}

fn map_mother(r: &sqlx::sqlite::SqliteRow) -> MotherRow {
    MotherRow {
        id: r.get("id"),
        store_id: r.get("store_id"),
        store_name: r.get("store_name"),
        drug_id: r.get("drug_id"),
        drug_name: r.get("drug_name"),
        spec: r.get("spec"),
        usage_text: r.get("usage_text"),
        unit: r.get("unit"),
        batch_no: r.get("batch_no"),
        opened_at: r.get("opened_at"),
        expiry_date: r.get("expiry_date"),
        total_qty: r.get("total_qty"),
        remaining_qty: r.get("remaining_qty"),
        status: r.get("status"),
    }
}

const MOTHER_SQL: &str = r#"
SELECT m.id, m.store_id, s.name AS store_name, m.drug_id, d.name AS drug_name,
       d.spec, d.usage_text, d.unit, m.batch_no, m.opened_at, m.expiry_date,
       m.total_qty, m.remaining_qty, m.status
FROM mother_packages m
JOIN stores s ON s.id = m.store_id
JOIN drugs d ON d.id = m.drug_id
"#;

// ---------- 元数据 ----------

async fn meta(State(st): State<AppState>) -> ApiResult<impl IntoResponse> {
    let stores = sqlx::query("SELECT id, name FROM stores ORDER BY id")
        .fetch_all(&st.pool)
        .await?
        .iter()
        .map(|r| serde_json::json!({"id": r.get::<i64,_>("id"), "name": r.get::<String,_>("name")}))
        .collect::<Vec<_>>();
    let drugs = sqlx::query("SELECT id, name, spec, usage_text, unit FROM drugs ORDER BY id")
        .fetch_all(&st.pool)
        .await?
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.get::<i64,_>("id"), "name": r.get::<String,_>("name"),
                "spec": r.get::<String,_>("spec"), "usage_text": r.get::<String,_>("usage_text"),
                "unit": r.get::<String,_>("unit"),
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({"stores": stores, "drugs": drugs})))
}

// ---------- 母包装 ----------

async fn list_mothers(State(st): State<AppState>) -> ApiResult<impl IntoResponse> {
    let rows = sqlx::query(&format!("{MOTHER_SQL} ORDER BY m.id DESC"))
        .fetch_all(&st.pool)
        .await?;
    let out: Vec<MotherRow> = rows.iter().map(map_mother).collect();
    Ok(Json(out))
}

async fn get_mother(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<impl IntoResponse> {
    let row = sqlx::query(&format!("{MOTHER_SQL} WHERE m.id = ?"))
        .bind(id)
        .fetch_optional(&st.pool)
        .await?
        .ok_or_else(|| bad("母包装不存在"))?;
    Ok(Json(map_mother(&row)))
}

#[derive(Deserialize)]
pub struct CreateMother {
    pub store_id: i64,
    pub drug_id: i64,
    pub batch_no: String,
    pub expiry_date: String,
    pub total_qty: i64,
}

async fn create_mother(
    State(st): State<AppState>,
    Json(body): Json<CreateMother>,
) -> ApiResult<impl IntoResponse> {
    if body.total_qty <= 0 {
        return Err(bad("数量必须为正数"));
    }
    if body.batch_no.trim().is_empty() {
        return Err(bad("批号不能为空"));
    }
    let opened_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let res = sqlx::query(
        "INSERT INTO mother_packages (store_id, drug_id, batch_no, opened_at, expiry_date, total_qty, remaining_qty, status)
         VALUES (?,?,?,?,?,?,?, 'ACTIVE')",
    )
    .bind(body.store_id)
    .bind(body.drug_id)
    .bind(body.batch_no.trim())
    .bind(&opened_at)
    .bind(&body.expiry_date)
    .bind(body.total_qty)
    .bind(body.total_qty)
    .execute(&st.pool)
    .await?;
    Ok(Json(serde_json::json!({"id": res.last_insert_rowid(), "opened_at": opened_at})))
}

// ---------- 拆零（核心：原子扣量 + 打印失败不计售出） ----------

#[derive(Deserialize)]
pub struct CreateRepack {
    pub mother_id: i64,
    pub qty: i64,
}

#[derive(Serialize)]
pub struct RepackResp {
    pub order_id: i64,
    pub status: String,
    pub qty: i64,
    pub remaining_after: i64,
    pub label: serde_json::Value,
}

async fn create_repack(
    State(st): State<AppState>,
    Json(body): Json<CreateRepack>,
) -> ApiResult<impl IntoResponse> {
    if body.qty <= 0 {
        return Err(bad("拆零数量必须为正数"));
    }

    // 1) 事务内原子扣量：remaining_qty >= qty 才允许更新，杜绝并发扣成负数
    let mut tx = st.pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE mother_packages
         SET remaining_qty = remaining_qty - ?,
             status = CASE WHEN remaining_qty - ? = 0 THEN 'DEPLETED' ELSE status END
         WHERE id = ? AND remaining_qty >= ? AND status = 'ACTIVE'",
    )
    .bind(body.qty)
    .bind(body.qty)
    .bind(body.mother_id)
    .bind(body.qty)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        let exists = sqlx::query("SELECT status, remaining_qty FROM mother_packages WHERE id = ?")
            .bind(body.mother_id)
            .fetch_optional(&mut *tx)
            .await?;
        return Err(match exists {
            None => bad("母包装不存在"),
            Some(r) if r.get::<String, _>("status") == "RECALLED" => bad("该批次已召回，禁止拆零销售"),
            Some(r) => bad(format!("库存不足：剩余 {}，本次需要 {}", r.get::<i64, _>("remaining_qty"), body.qty)),
        });
    }
    let m = sqlx::query(&format!("{MOTHER_SQL} WHERE m.id = ?"))
        .bind(body.mother_id)
        .fetch_one(&mut *tx)
        .await?;
    let mother = map_mother(&m);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let res = sqlx::query(
        "INSERT INTO repack_orders (mother_id, store_id, qty, status, drug_name, batch_no, usage_text, expiry_date, created_at)
         VALUES (?,?,?, 'PENDING_PRINT', ?,?,?,?,?)",
    )
    .bind(body.mother_id)
    .bind(mother.store_id)
    .bind(body.qty)
    .bind(&mother.drug_name)
    .bind(&mother.batch_no)
    .bind(&mother.usage_text)
    .bind(&mother.expiry_date)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    let order_id = res.last_insert_rowid();
    tx.commit().await?;

    let label = serde_json::json!({
        "order_id": order_id,
        "drug_name": mother.drug_name,
        "spec": mother.spec,
        "batch_no": mother.batch_no,
        "usage_text": mother.usage_text,
        "expiry_date": mother.expiry_date,
        "qty": body.qty,
        "unit": mother.unit,
        "store": mother.store_name,
    });

    // 2) 打印标签（本地模拟打印机）
    match st.printer.print(&label).await {
        Ok(()) => {
            // 3) 打印成功 → 记为已售出，并通知收银 mock
            sqlx::query("UPDATE repack_orders SET status = 'SOLD', printed_at = ? WHERE id = ?")
                .bind(&now)
                .bind(order_id)
                .execute(&st.pool)
                .await?;
            st.pos.record_sale(order_id, body.qty, &mother.drug_name).await;
            Ok(Json(RepackResp {
                order_id,
                status: "SOLD".into(),
                qty: body.qty,
                remaining_after: mother.remaining_qty,
                label,
            }))
        }
        Err(e) => {
            // 打印失败 → 不能记为已售出：订单标记 PRINT_FAILED，数量退回母包装
            let mut tx = st.pool.begin().await?;
            sqlx::query("UPDATE repack_orders SET status = 'PRINT_FAILED' WHERE id = ?")
                .bind(order_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "UPDATE mother_packages SET remaining_qty = remaining_qty + ?, status = 'ACTIVE' WHERE id = ?",
            )
            .bind(body.qty)
            .bind(body.mother_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            Err(bad(format!("标签打印失败，本次拆零未售出（{e}）")))
        }
    }
}

async fn list_repacks(State(st): State<AppState>) -> ApiResult<impl IntoResponse> {
    let rows = sqlx::query(
        "SELECT r.id, r.mother_id, r.store_id, s.name AS store_name, r.qty, r.status,
                r.drug_name, r.batch_no, r.usage_text, r.expiry_date, r.created_at, r.printed_at
         FROM repack_orders r JOIN stores s ON s.id = r.store_id ORDER BY r.id DESC",
    )
    .fetch_all(&st.pool)
    .await?;
    let out: Vec<_> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.get::<i64,_>("id"), "mother_id": r.get::<i64,_>("mother_id"),
                "store_id": r.get::<i64,_>("store_id"), "store_name": r.get::<String,_>("store_name"),
                "qty": r.get::<i64,_>("qty"), "status": r.get::<String,_>("status"),
                "drug_name": r.get::<String,_>("drug_name"), "batch_no": r.get::<String,_>("batch_no"),
                "usage_text": r.get::<String,_>("usage_text"), "expiry_date": r.get::<String,_>("expiry_date"),
                "created_at": r.get::<String,_>("created_at"), "printed_at": r.get::<Option<String>,_>("printed_at"),
            })
        })
        .collect();
    Ok(Json(out))
}

// ---------- 退药（进入待处理库存，不回母盒） ----------

#[derive(Deserialize)]
pub struct CreateReturn {
    pub store_id: i64,
    pub drug_id: i64,
    pub repack_order_id: Option<i64>,
    pub qty: i64,
}

async fn create_return(
    State(st): State<AppState>,
    Json(body): Json<CreateReturn>,
) -> ApiResult<impl IntoResponse> {
    if body.qty <= 0 {
        return Err(bad("退药数量必须为正数"));
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    // 注意：不触碰 mother_packages.remaining_qty —— 退回散装药隔离在待处理库存
    let res = sqlx::query(
        "INSERT INTO returns (store_id, drug_id, repack_order_id, qty, status, created_at)
         VALUES (?,?,?,?, 'PENDING_DISPOSAL', ?)",
    )
    .bind(body.store_id)
    .bind(body.drug_id)
    .bind(body.repack_order_id)
    .bind(body.qty)
    .bind(&now)
    .execute(&st.pool)
    .await?;
    Ok(Json(serde_json::json!({"id": res.last_insert_rowid(), "status": "PENDING_DISPOSAL"})))
}

async fn list_returns(State(st): State<AppState>) -> ApiResult<impl IntoResponse> {
    let rows = sqlx::query(
        "SELECT r.id, r.store_id, s.name AS store_name, r.drug_id, d.name AS drug_name,
                r.repack_order_id, r.qty, r.status, r.created_at
         FROM returns r JOIN stores s ON s.id = r.store_id JOIN drugs d ON d.id = r.drug_id
         ORDER BY r.id DESC",
    )
    .fetch_all(&st.pool)
    .await?;
    let out: Vec<_> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.get::<i64,_>("id"), "store_id": r.get::<i64,_>("store_id"),
                "store_name": r.get::<String,_>("store_name"),
                "drug_id": r.get::<i64,_>("drug_id"), "drug_name": r.get::<String,_>("drug_name"),
                "repack_order_id": r.get::<Option<i64>,_>("repack_order_id"),
                "qty": r.get::<i64,_>("qty"), "status": r.get::<String,_>("status"),
                "created_at": r.get::<String,_>("created_at"),
            })
        })
        .collect();
    Ok(Json(out))
}

// ---------- 调拨（仅未拆零整包） ----------

#[derive(Deserialize)]
pub struct CreateTransfer {
    pub mother_id: i64,
    pub to_store_id: i64,
}

async fn create_transfer(
    State(st): State<AppState>,
    Json(body): Json<CreateTransfer>,
) -> ApiResult<impl IntoResponse> {
    let mut tx = st.pool.begin().await?;
    let m = sqlx::query("SELECT store_id, total_qty, remaining_qty, status FROM mother_packages WHERE id = ?")
        .bind(body.mother_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| bad("母包装不存在"))?;
    let from_store: i64 = m.get("store_id");
    if from_store == body.to_store_id {
        return Err(bad("调出与调入门店不能相同"));
    }
    if m.get::<String, _>("status") != "ACTIVE" {
        return Err(bad("仅状态正常的整包可以调拨"));
    }
    if m.get::<i64, _>("remaining_qty") != m.get::<i64, _>("total_qty") {
        return Err(bad("只允许调拨未拆零的整包药品"));
    }
    let repacked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM repack_orders WHERE mother_id = ? AND status = 'SOLD'",
    )
    .bind(body.mother_id)
    .fetch_one(&mut *tx)
    .await?;
    if repacked > 0 {
        return Err(bad("该包装已发生拆零销售，禁止调拨"));
    }
    sqlx::query("UPDATE mother_packages SET store_id = ? WHERE id = ?")
        .bind(body.to_store_id)
        .bind(body.mother_id)
        .execute(&mut *tx)
        .await?;
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let res = sqlx::query(
        "INSERT INTO transfers (mother_id, from_store_id, to_store_id, qty, created_at) VALUES (?,?,?,?,?)",
    )
    .bind(body.mother_id)
    .bind(from_store)
    .bind(body.to_store_id)
    .bind(m.get::<i64, _>("total_qty"))
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"id": res.last_insert_rowid()})))
}

async fn list_transfers(State(st): State<AppState>) -> ApiResult<impl IntoResponse> {
    let rows = sqlx::query(
        "SELECT t.id, t.mother_id, t.qty, t.created_at,
                f.name AS from_store, tt.name AS to_store
         FROM transfers t JOIN stores f ON f.id = t.from_store_id JOIN stores tt ON tt.id = t.to_store_id
         ORDER BY t.id DESC",
    )
    .fetch_all(&st.pool)
    .await?;
    let out: Vec<_> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.get::<i64,_>("id"), "mother_id": r.get::<i64,_>("mother_id"),
                "qty": r.get::<i64,_>("qty"), "created_at": r.get::<String,_>("created_at"),
                "from_store": r.get::<String,_>("from_store"), "to_store": r.get::<String,_>("to_store"),
            })
        })
        .collect();
    Ok(Json(out))
}

// ---------- 批次召回 ----------

#[derive(Deserialize)]
pub struct RecallQuery {
    pub drug_id: i64,
    pub batch_no: String,
}

async fn recall_lookup(
    State(st): State<AppState>,
    Query(q): Query<RecallQuery>,
) -> ApiResult<impl IntoResponse> {
    let mothers = sqlx::query(&format!("{MOTHER_SQL} WHERE m.drug_id = ? AND m.batch_no = ?"))
        .bind(q.drug_id)
        .bind(q.batch_no.trim())
        .fetch_all(&st.pool)
        .await?;
    let mothers: Vec<MotherRow> = mothers.iter().map(map_mother).collect();
    let mother_ids: Vec<i64> = mothers.iter().map(|m| m.id).collect();
    let mut sales = Vec::new();
    for mid in &mother_ids {
        let rows = sqlx::query(
            "SELECT r.id, r.mother_id, r.qty, r.status, r.created_at, r.printed_at, s.name AS store_name
             FROM repack_orders r JOIN stores s ON s.id = r.store_id
             WHERE r.mother_id = ? ORDER BY r.id",
        )
        .bind(mid)
        .fetch_all(&st.pool)
        .await?;
        for r in &rows {
            sales.push(serde_json::json!({
                "id": r.get::<i64,_>("id"), "mother_id": r.get::<i64,_>("mother_id"),
                "qty": r.get::<i64,_>("qty"), "status": r.get::<String,_>("status"),
                "store_name": r.get::<String,_>("store_name"),
                "created_at": r.get::<String,_>("created_at"),
                "printed_at": r.get::<Option<String>,_>("printed_at"),
            }));
        }
    }
    Ok(Json(serde_json::json!({"mothers": mothers, "repack_records": sales})))
}

async fn execute_recall(
    State(st): State<AppState>,
    Json(q): Json<RecallQuery>,
) -> ApiResult<impl IntoResponse> {
    let res = sqlx::query(
        "UPDATE mother_packages SET status = 'RECALLED'
         WHERE drug_id = ? AND batch_no = ? AND status != 'RECALLED'",
    )
    .bind(q.drug_id)
    .bind(q.batch_no.trim())
    .execute(&st.pool)
    .await?;
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query("INSERT INTO recalls (drug_id, batch_no, created_at) VALUES (?,?,?)")
        .bind(q.drug_id)
        .bind(q.batch_no.trim())
        .bind(&now)
        .execute(&st.pool)
        .await?;
    Ok(Json(serde_json::json!({"recalled_mothers": res.rows_affected()})))
}

async fn list_recalls(State(st): State<AppState>) -> ApiResult<impl IntoResponse> {
    let rows = sqlx::query(
        "SELECT r.id, r.batch_no, r.created_at, d.name AS drug_name
         FROM recalls r JOIN drugs d ON d.id = r.drug_id ORDER BY r.id DESC",
    )
    .fetch_all(&st.pool)
    .await?;
    let out: Vec<_> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.get::<i64,_>("id"), "batch_no": r.get::<String,_>("batch_no"),
                "drug_name": r.get::<String,_>("drug_name"), "created_at": r.get::<String,_>("created_at"),
            })
        })
        .collect();
    Ok(Json(out))
}

// ---------- 模拟设备端点 ----------

#[derive(Deserialize)]
pub struct PrinterCfg {
    pub fail: bool,
}

async fn set_printer_fail(State(st): State<AppState>, Json(c): Json<PrinterCfg>) -> impl IntoResponse {
    st.printer.set_fail(c.fail);
    Json(serde_json::json!({"fail": c.fail}))
}

async fn printer_state(State(st): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({"fail": st.printer.will_fail()}))
}

async fn printer_jobs(State(st): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!(st.printer.jobs()))
}

async fn pos_sales(State(st): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!(st.pos.sales()))
}

// 让编译器保留 AtomicBool/Mutex 导入（mock 模块使用）
#[allow(dead_code)]
fn _assert_sync() {
    let _ = AtomicBool::new(false).load(Ordering::Relaxed);
    let _ = Mutex::new(());
}
