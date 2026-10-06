# 连锁药店拆零药品标签与剩余量核算系统

药师把整盒药拆成小包装出售时，用本系统登记母包装、打印拆零标签、核算母盒余量，并支持
退药隔离、门店整包调拨与批次召回追溯。

- **后端**：Rust + Axum + SQLx + SQLite（WAL）
- **前端**：Svelte 5 + Vite
- **外设**：打印机、收银机均为进程内本地模拟（可注入故障）
- **启动**：一条本地命令 `./run.sh`

## 快速开始

```bash
./run.sh          # 构建前端, Axum 单进程托管, 打开 http://localhost:8080
./run.sh dev      # 开发模式: 后端 :8080 + Vite 热更新 :5173
```

首次启动后点击页面任意操作前，系统会自动播种演示数据（2 家门店、3 种药品、3 个母包装），
也可手动调用 `POST /api/seed`。

## 业务规则与实现要点

| 规则 | 实现 |
| --- | --- |
| 登记母包装批次、开封效期、数量 | `POST /api/packs`；开封后标签有效期取开封效期 |
| 拆零关联母包装，打印药名/批号/用法/有效期标签 | `POST /api/sales`，标签快照存 `split_sales.label_json`，追溯码 `RX-0000123` |
| 柜台显示本次数量与拆后余量，不足不能销售 | 柜台页实时预览；后端余量判定 + 条件 UPDATE 双重拦截 |
| 多人同时操作同一盒不能扣成负数 | SQLite WAL + `BEGIN IMMEDIATE` 串行化写事务，`UPDATE ... WHERE remaining_qty >= qty`，`CHECK (remaining_qty >= 0)` 第三重保险，`SQLITE_BUSY` 自动重试 |
| 打印失败不能记已售出 | 扣量建单后调本地打印机→收银机；任一失败走补偿事务：回补余量、单据置 `voided`（永不 `sold`） |
| 退药入待处理库存，不增母盒余量 | `returned_stock` 独立表，只允许复核隔离，不回写 `parent_packs` |
| 只调拨未拆零整包 | 校验 `opened=0 AND remaining_qty=initial_qty`，整包随店移动并留调拨记录 |
| 批次召回定位母包装与全部拆零记录 | `POST /api/recalls` 冻结跨门店同批号母包装；`GET /api/recalls/{id}/report` 返回母包装、全部 `sold` 拆零单、待处理退药 |

## 外设模拟

- 柜台页右上可切换：正常 / 模拟打印失败 / 模拟收银失败；也可 `POST /api/sim/mode {"mode":"printer"}`。
- 单笔测试可在销售请求里带 `"sim_fail":"printer"|"cashier"`。
- 已打印标签与收银小票可通过 `/api/sim/jobs`、`/api/sim/receipts` 查看。

## 验证四个关键规则

```bash
cd backend && cargo test
```

- `test_concurrent.rs` — 10 人并发抢同一盒（余量 10、每人 3）：恰 3 单成功、7 单 `INSUFFICIENT_STOCK`，余量恰为 1，永不为负。
- `test_print_fail.rs` — 打印失败 / 收银失败：返回 409、单据 `voided`、余量回补、无小票/标签作业，恢复后可正常销售。
- `test_return_isolation.rs` — 退药进入 `pending` 待处理库存，母盒余量前后不变。
- `test_recall.rs` — 召回跨门店定位全部母包装（含已调拨）与全部已售拆零记录、冻结库存；已拆零包禁止调拨。

## API 一览

```
GET/POST /api/stores          GET/POST /api/drugs
GET/POST /api/packs           GET  /api/packs/:id
GET/POST /api/sales           GET/POST /api/returns
POST     /api/returns/:id/resolve
GET/POST /api/transfers
GET/POST /api/recalls         GET  /api/recalls/:id/report
POST     /api/sim/mode        GET  /api/sim/{status,jobs,receipts}
POST     /api/seed
```

柜台终端用请求头 `x-store-id` 选择当前门店（默认 1）。数据存于仓库根 `data/pharmacy.db`（WAL 模式）。
