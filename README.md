# 连锁药店拆零药品标签与剩余量核算系统

整盒药拆零销售时，药师可追溯到每份药来自哪一盒母包装，并实时核对母盒剩余量。

## 技术栈

- 后端：Rust + Axum + SQLx（SQLite）
- 前端：Svelte + Vite（构建产物由后端直接托管）
- 打印机与收银接口：进程内本地模拟（`/api/mock/*`）

## 启动

```bash
./start.sh          # 构建前端并启动后端，访问 http://127.0.0.1:3000
```

开发模式（前端热更新）：`cd web && npm run dev`（Vite 已代理 `/api` 到 3000 端口）。

## 验证

```bash
cd server && cargo test
```

5 个集成测试覆盖交付要求：

| 测试 | 验证的规则 |
|---|---|
| `concurrent_repack_never_oversells` | 20 个并发请求抢 50 份库存：售出总量恰好 50、余量归零不为负、超卖全部被拒 |
| `print_failure_is_not_sold` | 打印机故障时拆零单标记 `PRINT_FAILED`、不计售出、数量退回母盒、收银无记录 |
| `returns_go_to_pending_stock_only` | 退药进入待处理库存（`PENDING_DISPOSAL`），母盒余量不变 |
| `batch_recall_traces_and_blocks` | 按药品+批号查到全部母包装与拆零销售记录；召回后禁止再拆零，其他批次不受影响 |
| `transfer_only_unopened_packs` | 只有未拆零整包可跨店调拨，已拆零的被拒绝 |

## 业务规则实现要点

- **并发扣量**：扣减在事务内用一条条件更新完成
  `UPDATE ... SET remaining_qty = remaining_qty - ? WHERE id = ? AND remaining_qty >= ? AND status = 'ACTIVE'`，
  影响行数为 0 即拒绝；数据库另有 `CHECK (remaining_qty >= 0)` 兜底，不可能扣出负数。
- **打印失败不计售出**：先扣量并生成 `PENDING_PRINT` 订单 → 调用打印机 mock → 成功才置 `SOLD` 并通知收银 mock；失败则置 `PRINT_FAILED` 并在事务内把数量退回母盒。
- **退药隔离**：退药只写入 `returns` 表（待处理库存），不触碰母盒余量。
- **调拨限制**：仅 `remaining_qty = total_qty` 且无已售出拆零单的整包可调拨。
- **批次召回**：`GET /api/recalls/lookup` 按药品+批号追溯母包装与全部拆零记录；`POST /api/recalls` 将相关母包装置为 `RECALLED`，拆零接口拒绝召回批次。

## 主要接口

| 方法 | 路径 | 说明 |
|---|---|---|
| GET/POST | `/api/mothers` | 母包装列表 / 登记（批次、开封效期、数量） |
| GET/POST | `/api/repacks` | 拆零记录 / 创建拆零（打印标签并售出） |
| GET/POST | `/api/returns` | 待处理库存 / 退药登记 |
| GET/POST | `/api/transfers` | 调拨记录 / 整包调拨 |
| GET | `/api/recalls/lookup` | 批次追溯查询 |
| GET/POST | `/api/recalls` | 召回历史 / 执行召回 |
| POST | `/api/mock/printer/config` | 模拟打印机故障开关 `{"fail": true}` |
| GET | `/api/mock/printer/jobs`、`/api/mock/pos/sales` | 查看模拟打印与收银记录 |

数据库文件默认为 `server/repack.db`（可用 `DATABASE_URL` 覆盖），首次启动自动建表并写入演示数据。
