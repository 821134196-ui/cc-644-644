//! 本地模拟设备：标签打印机与收银接口。
//! 打印机可通过 /api/mock/printer/config 切换为故障模式，用于验证
//! “打印失败的拆零单不能记为已售出”。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Default)]
pub struct MockPrinter {
    fail: AtomicBool,
    jobs: Mutex<Vec<serde_json::Value>>,
}

#[derive(serde::Serialize, Clone)]
pub struct PrintJob {
    pub seq: usize,
    pub ok: bool,
    pub label: serde_json::Value,
}

impl MockPrinter {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_fail(&self, fail: bool) {
        self.fail.store(fail, Ordering::SeqCst);
    }
    pub fn will_fail(&self) -> bool {
        self.fail.load(Ordering::SeqCst)
    }
    pub async fn print(&self, label: &serde_json::Value) -> Result<(), String> {
        let ok = !self.will_fail();
        let mut jobs = self.jobs.lock().unwrap();
        let seq = jobs.len() + 1;
        jobs.push(serde_json::json!({ "seq": seq, "ok": ok, "label": label }));
        if ok {
            Ok(())
        } else {
            Err("模拟打印机故障：缺纸".to_string())
        }
    }
    pub fn jobs(&self) -> Vec<serde_json::Value> {
        self.jobs.lock().unwrap().clone()
    }
}

#[derive(serde::Serialize, Clone)]
pub struct PosSale {
    pub seq: usize,
    pub repack_order_id: i64,
    pub qty: i64,
    pub drug_name: String,
}

#[derive(Default)]
pub struct MockPos {
    sales: Mutex<Vec<PosSale>>,
}

impl MockPos {
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn record_sale(&self, repack_order_id: i64, qty: i64, drug_name: &str) {
        let mut sales = self.sales.lock().unwrap();
        let seq = sales.len() + 1;
        sales.push(PosSale { seq, repack_order_id, qty, drug_name: drug_name.to_string() });
    }
    pub fn sales(&self) -> Vec<PosSale> {
        self.sales.lock().unwrap().clone()
    }
}
