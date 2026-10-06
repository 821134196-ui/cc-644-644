use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Store {
    pub id: i64,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Drug {
    pub id: i64,
    pub name: String,
    pub spec: String,
    pub unit_pack: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ParentPack {
    pub id: i64,
    pub store_id: i64,
    pub drug_id: i64,
    pub drug_name: String,
    pub spec: String,
    pub unit_pack: String,
    pub batch_no: String,
    pub opened: i64,
    pub opened_at: Option<String>,
    pub opened_expiry: Option<String>,
    pub pack_expiry: String,
    pub initial_qty: i64,
    pub remaining_qty: i64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SplitSale {
    pub id: i64,
    pub store_id: i64,
    pub parent_pack_id: i64,
    pub drug_id: i64,
    pub drug_name: String,
    pub spec: String,
    pub unit_pack: String,
    pub batch_no: String,
    pub qty: i64,
    pub dosage: String,
    /// pending_print(已扣量待打印) / sold(已售出) / voided(打印或收银失败作废)
    pub status: String,
    pub label_json: String,
    pub fail_reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReturnedItem {
    pub id: i64,
    pub store_id: i64,
    pub drug_id: i64,
    pub drug_name: String,
    pub batch_no: String,
    pub qty: i64,
    pub reason: String,
    pub status: String,
    pub resolution: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transfer {
    pub id: i64,
    pub parent_pack_id: i64,
    pub drug_id: i64,
    pub drug_name: String,
    pub batch_no: String,
    pub qty: i64,
    pub from_store_id: i64,
    pub from_store_name: String,
    pub to_store_id: i64,
    pub to_store_name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Recall {
    pub id: i64,
    pub drug_id: i64,
    pub drug_name: String,
    pub batch_no: String,
    pub reason: String,
    pub created_at: String,
}

/* ---------- 请求体 ---------- */

#[derive(Debug, Deserialize)]
pub struct CreateStoreReq {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDrugReq {
    pub name: String,
    #[serde(default)]
    pub spec: String,
    #[serde(default = "default_unit")]
    pub unit_pack: String,
}

fn default_unit() -> String {
    "片".into()
}

#[derive(Debug, Deserialize)]
pub struct RegisterPackReq {
    pub drug_id: i64,
    pub batch_no: String,
    pub pack_expiry: String,
    #[serde(default)]
    pub opened_expiry: Option<String>,
    pub initial_qty: i64,
    /// 登记时是否已开封(同步登记开封效期)
    #[serde(default)]
    pub opened: bool,
}

#[derive(Debug, Deserialize)]
pub struct SaleReq {
    pub parent_pack_id: i64,
    pub qty: i64,
    #[serde(default)]
    pub dosage: String,
    /// 测试用: 强制本次 peripheral 失败 "printer" | "cashier"
    #[serde(default)]
    pub sim_fail: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReturnReq {
    pub drug_id: i64,
    #[serde(default)]
    pub batch_no: String,
    pub qty: i64,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveReturnReq {
    #[serde(default)]
    pub resolution: String,
}

#[derive(Debug, Deserialize)]
pub struct TransferReq {
    pub parent_pack_id: i64,
    pub to_store_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct RecallReq {
    pub drug_id: i64,
    pub batch_no: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct SimModeReq {
    /// none | printer | cashier
    pub mode: String,
}

/* ---------- 标签 ---------- */

/// 拆零标签快照: 药名 / 批号 / 用法 / 有效期, 以及追溯信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub trace_code: String,
    pub sale_id: i64,
    pub parent_pack_id: i64,
    pub store_name: String,
    pub drug_name: String,
    pub spec: String,
    pub batch_no: String,
    pub qty: i64,
    pub unit_pack: String,
    pub dosage: String,
    /// 标签标注的给病人使用的有效期(开封效期优先, 否则包装有效期)
    pub expiry: String,
    pub pack_expiry: String,
    pub opened_expiry: Option<String>,
    pub packed_at: String,
}
