-- 连锁药店拆零药品系统 schema (SQLite)
PRAGMA foreign_keys = ON;

-- 门店
CREATE TABLE IF NOT EXISTS stores (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    code        TEXT NOT NULL UNIQUE,           -- 门店编码, 如 S001
    name        TEXT NOT NULL
);

-- 药品字典
CREATE TABLE IF NOT EXISTS drugs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,                  -- 药品通用名
    spec        TEXT NOT NULL DEFAULT '',       -- 规格, 如 0.25g*24片
    unit_pack   TEXT NOT NULL DEFAULT '片'      -- 最小拆零单位(片/粒/支)
);

-- 母包装(整盒): 药师登记批次、开封效期、数量
CREATE TABLE IF NOT EXISTS parent_packs (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    store_id          INTEGER NOT NULL REFERENCES stores(id),
    drug_id           INTEGER NOT NULL REFERENCES drugs(id),
    batch_no          TEXT NOT NULL,            -- 批号
    opened            INTEGER NOT NULL DEFAULT 0,   -- 是否已开封(拆零) 0/1
    opened_at         TEXT,                     -- 开封时间 ISO8601
    opened_expiry     TEXT,                     -- 开封效期(短于包装有效期时使用)
    pack_expiry       TEXT NOT NULL,            -- 包装有效期
    initial_qty       INTEGER NOT NULL CHECK (initial_qty > 0),
    remaining_qty     INTEGER NOT NULL CHECK (remaining_qty >= 0),
    -- 调拨中/已调出的整包不能再销售; 召回时锁定
    status            TEXT NOT NULL DEFAULT 'in_stock', -- in_stock/transferred_out/recalled
    created_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_parent_batch ON parent_packs(batch_no);
CREATE INDEX IF NOT EXISTS idx_parent_drug_batch ON parent_packs(drug_id, batch_no);

-- 拆零销售单
CREATE TABLE IF NOT EXISTS split_sales (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    store_id          INTEGER NOT NULL REFERENCES stores(id),
    parent_pack_id    INTEGER NOT NULL REFERENCES parent_packs(id),
    drug_id           INTEGER NOT NULL REFERENCES drugs(id),
    qty               INTEGER NOT NULL CHECK (qty > 0),
    dosage            TEXT NOT NULL DEFAULT '', -- 用法, 如 "口服 一次1片 一日3次"
    -- pending_print: 已扣量待打印; sold: 打印+收银成功, 记为已售出; voided: 打印失败作废(余量已回补)
    status            TEXT NOT NULL DEFAULT 'pending_print',
    label_json        TEXT NOT NULL DEFAULT '', -- 标签快照(药名/批号/用法/有效期)
    fail_reason       TEXT NOT NULL DEFAULT '',
    created_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sales_parent ON split_sales(parent_pack_id);
CREATE INDEX IF NOT EXISTS idx_sales_drug ON split_sales(drug_id);

-- 顾客退回的散装药: 独立待处理库存, 不回增母盒余量
CREATE TABLE IF NOT EXISTS returned_stock (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    store_id          INTEGER NOT NULL REFERENCES stores(id),
    drug_id           INTEGER NOT NULL REFERENCES drugs(id),
    batch_no          TEXT NOT NULL DEFAULT '', -- 顾客往往说不清批号, 允许为空
    qty               INTEGER NOT NULL CHECK (qty > 0),
    reason            TEXT NOT NULL DEFAULT '',
    status            TEXT NOT NULL DEFAULT 'pending', -- pending(待处理)/quarantined(销毁等)
    created_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_return_batch ON returned_stock(batch_no);

-- 门店间调拨: 只允许未拆零的整包药品
CREATE TABLE IF NOT EXISTS transfers (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_pack_id    INTEGER NOT NULL REFERENCES parent_packs(id),
    drug_id           INTEGER NOT NULL REFERENCES drugs(id),
    batch_no          TEXT NOT NULL,
    qty               INTEGER NOT NULL,
    from_store_id     INTEGER NOT NULL REFERENCES stores(id),
    to_store_id       INTEGER NOT NULL REFERENCES stores(id),
    created_at        TEXT NOT NULL
);

-- 批次召回登记
CREATE TABLE IF NOT EXISTS recalls (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    drug_id           INTEGER NOT NULL REFERENCES drugs(id),
    batch_no          TEXT NOT NULL,
    reason            TEXT NOT NULL DEFAULT '',
    created_at        TEXT NOT NULL,
    UNIQUE(drug_id, batch_no)
);
CREATE INDEX IF NOT EXISTS idx_recall_batch ON recalls(batch_no);
