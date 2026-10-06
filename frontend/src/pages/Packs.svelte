<script>
  import { onMount } from 'svelte';
  import { get, post } from '../lib/api.js';

  let { stores, drugs, storeId } = $props();

  let packs = $state([]);
  let drugId = $state('');
  let batchNo = $state('');
  let packExpiry = $state('');
  let openedExpiry = $state('');
  let initialQty = $state(24);
  let opened = $state(false);
  let error = $state('');
  let ok = $state('');
  let loading = $state(false);
  let filterStatus = $state('');

  $effect(() => {
    if (!drugId && drugs.length) drugId = drugs[0].id;
  });

  async function load() {
    const qs = new URLSearchParams({ store_id: String(storeId) });
    if (filterStatus) qs.set('status', filterStatus);
    packs = await get(`/api/packs?${qs}`);
  }

  async function submit() {
    error = '';
    ok = '';
    loading = true;
    try {
      const body = {
        drug_id: Number(drugId),
        batch_no: batchNo.trim(),
        pack_expiry: packExpiry,
        opened_expiry: opened ? openedExpiry || null : null,
        initial_qty: Number(initialQty),
        opened,
      };
      const p = await post('/api/packs', body);
      ok = `已登记母包装 #${p.id}：${p.drug_name} 批号 ${p.batch_no}，数量 ${p.initial_qty} ${p.unit_pack}`;
      batchNo = '';
      openedExpiry = '';
      await load();
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  function badge(p) {
    if (p.status === 'recalled') return ['bad', '已召回冻结'];
    if (p.status === 'transferred_out') return ['warn', '已调出'];
    if (p.remaining_qty === 0) return ['muted', '已拆零完'];
    if (p.opened) return ['info', '已开封'];
    return ['ok', '未开封整包'];
  }

  onMount(load);
</script>

<div class="grid2">
  <div class="panel">
    <h2>登记母包装（批次 / 开封效期 / 数量）</h2>
    <div class="muted" style="margin-bottom:10px">登记门店：{stores.find((s) => s.id === storeId)?.name}</div>
    <label class="field">
      <span>药品</span>
      <select bind:value={drugId}>
        {#each drugs as d}
          <option value={d.id}>{d.name} {d.spec}（按{d.unit_pack}拆零）</option>
        {/each}
      </select>
    </label>
    <label class="field">
      <span>批号</span>
      <input bind:value={batchNo} placeholder="如 AMX20260801" />
    </label>
    <label class="field">
      <span>包装有效期</span>
      <input type="date" bind:value={packExpiry} />
    </label>
    <label class="field">
      <span>初始数量（整盒最小单位总数）</span>
      <input type="number" min="1" bind:value={initialQty} />
    </label>
    <label class="field">
      <span><input type="checkbox" style="width:auto" bind:checked={opened} /> 登记时已开封（需填写开封效期）</span>
    </label>
    {#if opened}
      <label class="field">
        <span>开封效期（标签向患者展示的有效期以此为准）</span>
        <input type="date" bind:value={openedExpiry} />
      </label>
    {/if}
    {#if error}<div class="alert err">{error}</div>{/if}
    {#if ok}<div class="alert ok">{ok}</div>{/if}
    <button class="btn" onclick={submit} disabled={loading}>{loading ? '提交中…' : '登记母包装'}</button>
  </div>

  <div class="panel">
    <h2>本门店母包装库存</h2>
    <div style="margin-bottom:10px">
      <button class="pill-btn" class:on={filterStatus === ''} onclick={() => { filterStatus = ''; load(); }}>全部</button>
      <button class="pill-btn" class:on={filterStatus === 'in_stock'} onclick={() => { filterStatus = 'in_stock'; load(); }}>在库</button>
      <button class="pill-btn" class:on={filterStatus === 'recalled'} onclick={() => { filterStatus = 'recalled'; load(); }}>已召回</button>
    </div>
    <table>
      <thead><tr><th>#</th><th>药品 / 批号</th><th>效期</th><th>余量</th><th>状态</th></tr></thead>
      <tbody>
        {#each packs as p}
          {@const [cls, text] = badge(p)}
          <tr>
            <td>{p.id}</td>
            <td>{p.drug_name}<div class="muted mono">{p.batch_no} · {p.spec}</div></td>
            <td>
              {#if p.opened_expiry}
                <span class="badge warn">开封效期 {p.opened_expiry}</span>
              {/if}
              <div class:muted={!!p.opened_expiry}>{p.pack_expiry}</div>
            </td>
            <td><b>{p.remaining_qty}</b> / {p.initial_qty} {p.unit_pack}</td>
            <td><span class="badge {cls}">{text}</span></td>
          </tr>
        {/each}
        {#if packs.length === 0}<tr><td colspan="5" class="muted">无母包装</td></tr>{/if}
      </tbody>
    </table>
  </div>
</div>
