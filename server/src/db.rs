use sqlx::SqlitePool;

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS stores (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE IF NOT EXISTS drugs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            spec TEXT NOT NULL,
            usage_text TEXT NOT NULL,
            unit TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS mother_packages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            store_id INTEGER NOT NULL REFERENCES stores(id),
            drug_id INTEGER NOT NULL REFERENCES drugs(id),
            batch_no TEXT NOT NULL,
            opened_at TEXT NOT NULL,
            expiry_date TEXT NOT NULL,
            total_qty INTEGER NOT NULL CHECK (total_qty > 0),
            remaining_qty INTEGER NOT NULL CHECK (remaining_qty >= 0),
            status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','RECALLED','DEPLETED'))
        );
        CREATE INDEX IF NOT EXISTS idx_mother_batch ON mother_packages(drug_id, batch_no);
        CREATE TABLE IF NOT EXISTS repack_orders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            mother_id INTEGER NOT NULL REFERENCES mother_packages(id),
            store_id INTEGER NOT NULL REFERENCES stores(id),
            qty INTEGER NOT NULL CHECK (qty > 0),
            status TEXT NOT NULL CHECK (status IN ('PENDING_PRINT','SOLD','PRINT_FAILED')),
            drug_name TEXT NOT NULL,
            batch_no TEXT NOT NULL,
            usage_text TEXT NOT NULL,
            expiry_date TEXT NOT NULL,
            created_at TEXT NOT NULL,
            printed_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_repack_mother ON repack_orders(mother_id);
        CREATE TABLE IF NOT EXISTS returns (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            store_id INTEGER NOT NULL REFERENCES stores(id),
            drug_id INTEGER NOT NULL REFERENCES drugs(id),
            repack_order_id INTEGER REFERENCES repack_orders(id),
            qty INTEGER NOT NULL CHECK (qty > 0),
            status TEXT NOT NULL DEFAULT 'PENDING_DISPOSAL' CHECK (status IN ('PENDING_DISPOSAL','DISPOSED')),
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS transfers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            mother_id INTEGER NOT NULL REFERENCES mother_packages(id),
            from_store_id INTEGER NOT NULL REFERENCES stores(id),
            to_store_id INTEGER NOT NULL REFERENCES stores(id),
            qty INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS recalls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            drug_id INTEGER NOT NULL REFERENCES drugs(id),
            batch_no TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// 首次启动时写入演示数据
pub async fn seed(pool: &SqlitePool) -> anyhow::Result<()> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stores").fetch_one(pool).await?;
    if n > 0 {
        return Ok(());
    }
    sqlx::query("INSERT INTO stores (name) VALUES ('总店'), ('分店')")
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO drugs (name, spec, usage_text, unit) VALUES
         ('阿莫西林胶囊', '0.25g×24粒/盒', '口服，一次2粒，一日3次', '粒'),
         ('布洛芬片', '0.2g×100片/瓶', '口服，一次1片，必要时服用', '片'),
         ('维生素C片', '100mg×60片/瓶', '口服，一次1片，一日1次', '片')",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO mother_packages (store_id, drug_id, batch_no, opened_at, expiry_date, total_qty, remaining_qty, status)
         VALUES
         (1, 1, 'AMX20250901', '2026-10-01 09:00:00', '2027-09-30', 24, 24, 'ACTIVE'),
         (1, 2, 'BLF20250815', '2026-09-20 10:30:00', '2027-08-31', 100, 100, 'ACTIVE'),
         (2, 3, 'VTC20250701', '2026-10-05 14:00:00', '2027-06-30', 60, 60, 'ACTIVE')",
    )
    .execute(pool)
    .await?;
    Ok(())
}
