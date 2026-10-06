<script>
  import { get, post } from '../lib/api.js';

  let { drugs } = $props();

  let drugId = $state('');
  let batchNo = $state('');
  let reason = $state('');
  let error = $state('');
  let report = $state(null);
  let recalls = $state([]);
  let loading = $state(false);

  $effect(() => {
    if (!drugId && drugs.length) drugId = drugs[0].id;
  });

  async function loadRecalls() {
    recalls = await get('/api/recalls');
  }

  async function submit() {
    error = '';
    report = null;
    loading = true;
    try {
      const r = await post('/api/recalls', {
        drug_id: Number(drugId),
        batch_no: batchNo.trim(),
        reason: reason.trim(),
      });
      batchNo = '';
      reason = '';
      await loadRecalls();
      await openReport(r.id);
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  async function openReport(id) {
    report = await get(`/api/recalls/${id}/report`);
  }

  loadRecalls();
</script>

<div class="panel">
  <h2>批次召回登记</h2>
  <div class="grid2">
    <div>
      <label class="field">
        <span>药品</span>
        <select bind:value={drugId}>
          {#each drugs as d}<option value={d.id}>{d.name} {d.spec}</option>{/each}
        </select>
      </label>
      <label class="field">
        <span>召回批号</span>
        <input bind:value={batchNo} placeholder="如 BLF20260915" />
      </label>
      <label class="field">
        <span>召回原因</span>
        <input bind:value={reason} placeholder="如：溶出度不合格" />
      </label>
      {#if error}<div class="alert err">{error}</div>{/if}
      <button class="btn danger" onclick={submit} disabled={loading}>
        {loading ? '处理中…' : '发起召回并冻结相关母包装'}
      </button>
    </div>
    <div>
      <h3>历史召回</h3>
      <table>
        <thead><tr><th>#</th><th>药品</th><th>批号</th><th>原因</th><th></th></tr></thead>
        <tbody>
          {#each recalls as r}
            <tr>
              <td>{r.id}</td><td>{r.drug_name}</td>
              <td class="mono">{r.batch_no}</td><td class="muted">{r.reason}</td>
              <td><button class="btn small ghost" onclick={() => openReport(r.id)}>查看报告</button></td>
            </tr>
          {/each}
          {#if recalls.length === 0}<tr><td colspan="5" class="muted">暂无召回记录</td></tr>{/if}
        </tbody>
      </table>
    </div>
  </div>
</div>

{#if report}
  <div class="panel">
    <h2>
      召回报告：{report.recall.drug_name} 批号
      <span class="mono">{report.recall.batch_no}</span>
    </h2>
    <div class="stat-line">
      <div class="stat"><div class="cap">涉及母包装</div><div class="val">{report.parent_packs.length} 盒</div></div>
      <div class="stat"><div class="cap">已售出拆零笔数</div><div class="val">{report.sold_splits.length} 笔</div></div>
      <div class="stat"><div class="cap">已售出数量</div><div class="val">{report.total_sold_qty} 份</div></div>
      <div class="stat"><div class="cap">待处理退药</div><div class="val">{report.returned_pending.length} 批</div></div>
    </div>

    <h3>相关母包装（跨全门店，含已调拨；均已冻结）</h3>
    <table>
      <thead><tr><th>#</th><th>所在门店</th><th>批号</th><th>余量/初始</th><th>状态</th></tr></thead>
      <tbody>
        {#each report.parent_packs as p}
          <tr>
            <td>{p.id}</td>
            <td>{p.store_id}</td>
            <td class="mono">{p.batch_no}</td>
            <td>{p.remaining_qty}/{p.initial_qty} {p.unit_pack}</td>
            <td><span class="badge bad">已召回冻结</span></td>
          </tr>
        {/each}
      </tbody>
    </table>

    <h3>全部已售出拆零记录（可逐笔追溯到母包装与患者用法标签）</h3>
    <table>
      <thead><tr><th>追溯码</th><th>母包装</th><th>门店</th><th>数量</th><th>用法</th><th>售出时间</th></tr></thead>
      <tbody>
        {#each report.sold_splits as s}
          <tr>
            <td class="mono">{s.label_json ? JSON.parse(s.label_json).trace_code : '—'}</td>
            <td>#{s.parent_pack_id}</td>
            <td>{s.store_id}</td>
            <td>{s.qty} {s.unit_pack}</td>
            <td>{s.dosage}</td>
            <td class="muted">{s.created_at}</td>
          </tr>
        {/each}
        {#if report.sold_splits.length === 0}
          <tr><td colspan="6" class="muted">该批次尚无已售出拆零记录（打印/收银失败的作废单不计入）</td></tr>
        {/if}
      </tbody>
    </table>

    {#if report.returned_pending.length > 0}
      <h3>待处理退药中的同批次散装药</h3>
      <table>
        <thead><tr><th>#</th><th>门店</th><th>数量</th><th>原因</th></tr></thead>
        <tbody>
          {#each report.returned_pending as r}
            <tr><td>{r.id}</td><td>{r.store_id}</td><td>{r.qty}</td><td class="muted">{r.reason}</td></tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
{/if}
