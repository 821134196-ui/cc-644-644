use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::models::Label;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FailMode {
    #[default]
    None,
    /// 模拟打印机故障(卡纸/离线)
    Printer,
    /// 模拟收银机故障(钱箱/支付终端)
    Cashier,
}

/// 本地打印机 / 收银接口模拟器。
///
/// - 成功时把标签 / 小票写入各自的作业队列(可在前端查看)。
/// - 通过 `set_fail_mode` 注入故障, 用于验证"打印失败不记已售"。
/// - 作业编号递增, 代表真实设备的打印任务流水号。
#[derive(Clone)]
pub struct Peripherals {
    inner: Arc<RwLock<Inner>>,
    seq: Arc<AtomicUsize>,
}

#[derive(Default)]
struct Inner {
    fail_mode: FailMode,
    print_jobs: Vec<PrintJob>,
    receipts: Vec<Receipt>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PrintJob {
    pub seq: usize,
    pub label: Label,
    pub printed_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Receipt {
    pub seq: usize,
    pub trace_code: String,
    pub sale_id: i64,
    pub amount_cents: i64,
    pub paid_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SimStatus {
    pub fail_mode: String,
    pub printed: usize,
    pub receipted: usize,
}

impl Default for Peripherals {
    fn default() -> Self {
        Self::new()
    }
}

impl Peripherals {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner::default())),
            seq: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn set_fail_mode(&self, mode: FailMode) {
        self.inner.write().unwrap().fail_mode = mode;
    }

    pub fn status(&self) -> SimStatus {
        let g = self.inner.read().unwrap();
        SimStatus {
            fail_mode: match g.fail_mode {
                FailMode::None => "none",
                FailMode::Printer => "printer",
                FailMode::Cashier => "cashier",
            }
            .into(),
            printed: g.print_jobs.len(),
            receipted: g.receipts.len(),
        }
    }

    pub fn jobs(&self) -> Vec<PrintJob> {
        self.inner.read().unwrap().print_jobs.clone()
    }

    pub fn receipts(&self) -> Vec<Receipt> {
        self.inner.read().unwrap().receipts.clone()
    }

    /// 打印拆零标签。设备处于 printer 故障模式时返回 Err。
    pub fn print_label(&self, label: &Label, now: &str) -> Result<usize, String> {
        let mut g = self.inner.write().unwrap();
        if g.fail_mode == FailMode::Printer {
            return Err("打印机离线: 标签未能输出".into());
        }
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        g.print_jobs.push(PrintJob {
            seq,
            label: label.clone(),
            printed_at: now.into(),
        });
        Ok(seq)
    }

    /// 收银记账。设备处于 cashier 故障模式时返回 Err。
    pub fn charge(&self, label: &Label, sale_id: i64, now: &str) -> Result<usize, String> {
        let mut g = self.inner.write().unwrap();
        if g.fail_mode == FailMode::Cashier {
            return Err("收银终端无应答: 收款未完成".into());
        }
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        // 本地模拟不做定价体系: 每最小单位 1 元
        let amount_cents = label.qty * 100;
        g.receipts.push(Receipt {
            seq,
            trace_code: label.trace_code.clone(),
            sale_id,
            amount_cents,
            paid_at: now.into(),
        });
        Ok(seq)
    }
}
