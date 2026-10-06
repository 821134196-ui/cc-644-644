<script>
  import { onMount } from 'svelte';
  import { get, post } from '../lib/api.js';

  let { stores, drugs, storeId } = $props();

  let packs = $state([]);
  let selectedId = $state(null);
  let qty = $state(1);
  let dosage = $state('口服 一次1片 一日3次');
  let loading = $state(false);
  let error = $state('');
  let result = $state(null); // 最近一次成功销售
  let recentSales = $state([]);
  let simMode = $state('none');
  let simStatus = $state(null);

  const selectedPack = $derived(packs.find((p) => p.id === Number(selectedId)));
  const remainingAfterPreview = $derived(
    selectedPack ? selectedPack.remaining_qty - Math.max(1, Number(qty) || 0) : null
  );
  const insufficient = $derived(
    selectedPack && remainingAfterPreview !== null && remainingAfterPreview < 0
  );
  const drugMap = $derived(new Map(drugs.map((d) => [d.id, d])));

  async function loadPacks() {
    packs = await get(`/api/packs?store_id=${storeId}`);
    if (!selectedId && packs.length) selectedId = packs[0].id;
    if (selectedId) await loadSales();
  }

  async function loadSales() {
    if (!selectedId) return;
    recentSales = await get(`/api/sales?parent_pack_id=${selectedId}`);
  }

  async function loadSim() {
    simStatus = await get('/api/sim/status');
    simMode = simStatus.fail_mode;
  }

  async function onSelect() {
    result = null;
    error = '';
    await loadSales();
  }

  async function sell() {
    error = '';
    result = null;
    if (!selectedPack) return;
    loading = true;
    try {
      // 提交前再算一次柜台余量
      result = await post('/api/sales', {
        parent_pack_id: selectedPack.id,
        qty: Math.max(1, Number(qty) || 0),
        dosage,
      });
      qty = 1;
      await Promise.all([loadPacks(), loadSim()]);
    } catch (e) {
      error = e.message;
      // 失败(含打印失败)也要刷新余量与单据列表
      await Promise.all([loadPacks(), loadSim()]);
    } finally {
      loading = false;
    }
  }

  async function setSim(mode) {
    await post('/api/sim/mode', { mode });
    await loadSim();
  }

  function statusBadge(s) {
    if (s === 'sold') return ['ok', '已售出'];
    if (s === 'voided') return ['bad', '已作废'];
    return ['warn', '待打印'];
  }
  function packBadge(p) {
    if (p.status === 'recalled') return ['bad', '已召回冻结'];
    if (p.status === 'transferred_out') return ['warn', '已调出'];
    if (p.remaining_qty === 0) return ['muted', '已拆零完'];
    if (p.opened) return ['info', '已开封'];
    return ['ok', '未开封整包'];
  }

  onMount(async () => {
    await Promise.all([loadPacks(), loadSim()]);
  });
</script>

<div class="panel">
  <h2>🧾 柜台拆零销售 — {stores.find((s) => s.id === storeId)?.name}</h2>

  <div class="grid2">
    <div>
      <label class="field">
        <span>选择母包装（本门店在库，按批号追溯）</span>
        <select bind:value={selectedId} onchange={onSelect}>
          {#each packs as p (p.id)}
            <option value={p.id}>
              #{p.id} {p.drug_name} {p.spec}｜批号 {p.batch_no}｜余 {p.remaining_qty}/{p.initial_qty}
              {p.unit_pack}{p.status !== 'in_stock' ? `｜${p.status}` : ''}
            </option>
          {/each}
        </select>
      </label>

      {#if selectedPack}
        <div class="stat-line">
          <div class="stat">
            <div class="cap">母盒当前余量</div>
            <div class="val">{selectedPack.remaining_qty} {selectedPack.unit_pack}</div>
          </div>
          <div class="stat">
            <div class="cap">本次拆零数量</div>
            <div class="val">{Math.max(1, Number(qty) || 0)} {selectedPack.unit_pack}</div>
          </div>
          <div class="stat">
            <div class="cap">拆后余量（柜台预览）</div>
            <div class="val" class:neg={remainingAfterPreview < 0}>{remainingAfterPreview}</div>
          </div>
        </div>

        <label class="field">
          <span>拆零数量（最小单位：{selectedPack.unit_pack}）</span>
          <input type="number" min="1" max={selectedPack.remaining_qty} bind:value={qty} />
        </label>
        <label class="field">
          <span>用法（打印到标签）</span>
          <input bind:value={dosage} />
        </label>

        {#if insufficient}
          <div class="alert err">
            库存不足：本次申请 {Math.max(1, Number(qty) || 0)} {selectedPack.unit_pack}，
            母盒仅剩 {selectedPack.remaining_qty} {selectedPack.unit_pack}，不能销售。
          </div>
        {/if}
        {#if selectedPack.status === 'recalled'}
          <div class="alert err">该批次已被召回，母包装已冻结，不能销售。</div>
        {/if}

        <button class="btn" onclick={sell} disabled={loading || insufficient || selectedPack.status !== 'in_stock'}>
          {loading ? '处理中…' : '拆零销售并打印标签'}
        </button>
        <span class="muted" style="margin-left:10px">
          流程：扣量 → 打印标签 → 收银；打印或收银失败则自动回补余量，单据作废。
        </span>
      {/if}
    </div>

    <div>
      <h3>本地外设模拟（打印机 / 收银机）</h3>
      <div class="muted" style="margin-bottom:6px">
        当前故障注入：
        {#if simMode === 'none'}
          <span class="badge ok">全部正常</span>
        {:else if simMode === 'printer'}
          <span class="badge bad">打印机故障</span>
        {:else}
          <span class="badge bad">收银机故障</span>
        {/if}
      </div>
      <button class="pill-btn" class:on={simMode === 'none'} onclick={() => setSim('none')}>正常</button>
      <button class="pill-btn" class:on={simMode === 'printer'} onclick={() => setSim('printer')}>模拟打印失败</button>
      <button class="pill-btn" class:on={simMode === 'cashier'} onclick={() => setSim('cashier')}>模拟收银失败</button>
      {#if simStatus}
        <p class="muted" style="margin-top:8px">
          已打印标签 {simStatus.printed} 张 ｜ 已收银小票 {simStatus.receipted} 张
        </p>
      {/if}

      {#if error}
        <div class="alert err">⛔ {error}</div>
      {/if}
      {#if result}
        <div class="alert ok">
          ✅ 销售成立（{result.trace_code}）：本次拆出 <b>{result.qty}</b> {result.label.unit_pack}，
          母盒拆后余量 <b>{result.remaining_after}</b> {result.label.unit_pack}。
        </div>
        <div class="label-card">
          <div class="l-head">
            <div class="l-name">{result.label.drug_name}</div>
            <div class="mono">{result.label.trace_code}</div>
          </div>
          <div class="muted">{result.label.spec} ｜ 拆零 {result.label.qty} {result.label.unit_pack}</div>
          <div class="l-row"><span>批号：<b>{result.label.batch_no}</b></span></div>
          <div class="l-row"><span>用法：<b>{result.label.dosage || '遵医嘱'}</b></span></div>
          <div class="l-row"><span>有效期至：<b>{result.label.expiry}</b></span></div>
          <div class="l-foot">
            母包装 #{result.label.parent_pack_id} ｜ {result.label.store_name} ｜
            分装时间 {result.label.packed_at}
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>

{#if selectedPack}
  <div class="panel">
    <h2>该母盒拆零单据</h2>
    <table>
      <thead>
        <tr><th>单号</th><th>药品 / 批号</th><th>数量</th><th>用法</th><th>状态</th><th>失败原因</th><th>时间</th></tr>
      </thead>
      <tbody>
        {#each recentSales as s}
          {@const [cls, text] = statusBadge(s.status)}
          <tr>
            <td class="mono">{s.label_json ? JSON.parse(s.label_json).trace_code : '—'}</td>
            <td>{s.drug_name}<div class="muted">{s.batch_no}</div></td>
            <td>{s.qty} {s.unit_pack}</td>
            <td>{s.dosage}</td>
            <td><span class="badge {cls}">{text}</span></td>
            <td class="muted">{s.fail_reason || '—'}</td>
            <td class="muted">{s.created_at}</td>
          </tr>
        {/each}
        {#if recentSales.length === 0}
          <tr><td colspan="7" class="muted">暂无拆零记录</td></tr>
        {/if}
      </tbody>
    </table>
  </div>

  <div class="panel">
    <h2>本门店全部母包装</h2>
    <table>
      <thead>
        <tr><th>#</th><th>药品</th><th>批号</th><th>开封效期 / 包装效期</th><th>余量</th><th>状态</th></tr>
      </thead>
      <tbody>
        {#each packs as p}
          {@const [cls, text] = packBadge(p)}
          <tr class="clickable" onclick={() => { selectedId = p.id; onSelect(); }}>
            <td>{p.id}</td>
            <td>{p.drug_name}<div class="muted">{p.spec}</div></td>
            <td class="mono">{p.batch_no}</td>
            <td>{p.opened_expiry ? `${p.opened_expiry}（开封效期）` : p.pack_expiry}</td>
            <td><b>{p.remaining_qty}</b> / {p.initial_qty} {p.unit_pack}</td>
            <td><span class="badge {cls}">{text}</span></td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}
