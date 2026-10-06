<script>
  import { onMount } from 'svelte'
  import { api } from './api.js'
  export let meta = { stores: [], drugs: [] }

  let form = { store_id: 1, drug_id: 1, repack_order_id: '', qty: 1 }
  let returns = []
  let msg = ''
  let msgOk = false

  async function load() {
    returns = await api.get('/api/returns')
  }
  onMount(load)

  async function submit() {
    try {
      await api.post('/api/returns', {
        store_id: Number(form.store_id),
        drug_id: Number(form.drug_id),
        repack_order_id: form.repack_order_id ? Number(form.repack_order_id) : null,
        qty: Number(form.qty)
      })
      msgOk = true
      msg = '已放入待处理库存（不回充母盒余量）'
      await load()
    } catch (e) {
      msgOk = false
      msg = e.message
    }
  }
</script>

<div class="card">
  <h2>退药登记</h2>
  <p class="hint">退回的散装药进入<b>待处理库存</b>等待处置，不会增加母盒余量。</p>
  <div class="row">
    <label>门店
      <select bind:value={form.store_id}>
        {#each meta.stores as s}<option value={s.id}>{s.name}</option>{/each}
      </select>
    </label>
    <label>药品
      <select bind:value={form.drug_id}>
        {#each meta.drugs as d}<option value={d.id}>{d.name}</option>{/each}
      </select>
    </label>
    <label>关联拆零单号（可选） <input bind:value={form.repack_order_id} placeholder="留空则不关联" /></label>
    <label>数量 <input type="number" min="1" bind:value={form.qty} /></label>
    <button class="primary" on:click={submit}>登记退药</button>
  </div>
  {#if msg}<p class={msgOk ? 'msg-ok' : 'msg-err'}>{msg}</p>{/if}
</div>

<div class="card">
  <h2>待处理库存</h2>
  <table>
    <thead><tr><th>ID</th><th>门店</th><th>药品</th><th>关联拆零单</th><th>数量</th><th>状态</th><th>时间</th></tr></thead>
    <tbody>
      {#each returns as r}
        <tr>
          <td>{r.id}</td><td>{r.store_name}</td><td>{r.drug_name}</td>
          <td>{r.repack_order_id ?? '—'}</td><td>{r.qty}</td>
          <td>{r.status === 'PENDING_DISPOSAL' ? '待处置' : '已处置'}</td><td>{r.created_at}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .row { display: flex; gap: 12px; align-items: flex-end; flex-wrap: wrap; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: #555; }
  .hint { color: #92400e; background: #fffbeb; padding: 8px 12px; border-radius: 6px; }
</style>
