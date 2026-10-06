<script>
  import { onMount } from 'svelte';
  import { get, post } from '../lib/api.js';

  let { stores, drugs, storeId } = $props();

  let items = $state([]);
  let drugId = $state('');
  let batchNo = $state('');
  let qty = $state(1);
  let reason = $state('');
  let error = $state('');
  let ok = $state('');

  $effect(() => {
    if (!drugId && drugs.length) drugId = drugs[0].id;
  });

  async function load() {
    items = await get(`/api/returns?status=pending`);
  }

  async function submit() {
    error = '';
    ok = '';
    try {
      const r = await post('/api/returns', {
        drug_id: Number(drugId),
        batch_no: batchNo.trim(),
        qty: Number(qty),
        reason: reason.trim(),
      });
      ok = `退药已进入待处理库存 #${r.id}（${r.drug_name} × ${r.qty}），不会增加任何母盒余量。`;
      batchNo = '';
      reason = '';
      qty = 1;
      await load();
    } catch (e) {
      error = e.message;
    }
  }

  async function resolve(id) {
    await post(`/api/returns/${id}/resolve`, { resolution: '药师复核后隔离，不上架' });
    await load();
  }

  function drugName(id) {
    return drugs.find((d) => d.id === id)?.name || `#${id}`;
  }
  function storeName(id) {
    return stores.find((s) => s.id === id)?.name || `门店${id}`;
  }

  onMount(load);
</script>

<div class="grid2">
  <div class="panel">
    <h2>登记顾客退回的散装药</h2>
    <div class="alert warn">
      退回的散装药只进入<b>待处理库存</b>等待质量复核，<b>不回增母包装余量、不允许重新上架销售</b>。
    </div>
    <label class="field">
      <span>药品</span>
      <select bind:value={drugId}>
        {#each drugs as d}<option value={d.id}>{d.name} {d.spec}</option>{/each}
      </select>
    </label>
    <label class="field">
      <span>批号（顾客无法辨认可留空）</span>
      <input bind:value={batchNo} placeholder="可留空" />
    </label>
    <label class="field">
      <span>退回数量</span>
      <input type="number" min="1" bind:value={qty} />
    </label>
    <label class="field">
      <span>退回原因</span>
      <textarea rows="2" bind:value={reason} placeholder="如：用药后不适 / 包装破损"></textarea>
    </label>
    {#if error}<div class="alert err">{error}</div>{/if}
    {#if ok}<div class="alert ok">{ok}</div>{/if}
    <button class="btn" onclick={submit}>收入待处理库存</button>
  </div>

  <div class="panel">
    <h2>待处理退药（{items.length}）</h2>
    <table>
      <thead><tr><th>#</th><th>门店</th><th>药品</th><th>批号</th><th>数量</th><th>原因</th><th>操作</th></tr></thead>
      <tbody>
        {#each items as r}
          <tr>
            <td>{r.id}</td>
            <td>{storeName(r.store_id)}</td>
            <td>{drugName(r.drug_id)}</td>
            <td class="mono">{r.batch_no || '未知'}</td>
            <td>{r.qty}</td>
            <td class="muted">{r.reason}</td>
            <td><button class="btn small ghost" onclick={() => resolve(r.id)}>复核隔离</button></td>
          </tr>
        {/each}
        {#if items.length === 0}<tr><td colspan="7" class="muted">没有待处理退药</td></tr>{/if}
      </tbody>
    </table>
  </div>
</div>
