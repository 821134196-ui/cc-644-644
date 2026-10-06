<script>
  import { createEventDispatcher } from 'svelte'
  import { api } from './api.js'
  export let meta = { stores: [], drugs: [] }
  export let mothers = []
  const dispatch = createEventDispatcher()

  let form = { store_id: 1, drug_id: 1, batch_no: '', expiry_date: '', total_qty: 1 }
  let msg = ''
  let msgOk = false

  async function submit() {
    try {
      await api.post('/api/mothers', { ...form, store_id: Number(form.store_id), drug_id: Number(form.drug_id), total_qty: Number(form.total_qty) })
      msgOk = true
      msg = '母包装已登记'
      form.batch_no = ''
      dispatch('done')
    } catch (e) {
      msgOk = false
      msg = e.message
    }
  }

  const statusText = { ACTIVE: '正常', RECALLED: '已召回', DEPLETED: '已售罄' }
</script>

<div class="card">
  <h2>登记母包装</h2>
  <div class="row">
    <label>门店
      <select bind:value={form.store_id}>
        {#each meta.stores as s}<option value={s.id}>{s.name}</option>{/each}
      </select>
    </label>
    <label>药品
      <select bind:value={form.drug_id}>
        {#each meta.drugs as d}<option value={d.id}>{d.name}（{d.spec}）</option>{/each}
      </select>
    </label>
    <label>批号 <input bind:value={form.batch_no} placeholder="如 AMX20250901" /></label>
    <label>开封效期 <input type="date" bind:value={form.expiry_date} /></label>
    <label>数量 <input type="number" min="1" bind:value={form.total_qty} /></label>
    <button class="primary" on:click={submit}>登记</button>
  </div>
  {#if msg}<p class={msgOk ? 'msg-ok' : 'msg-err'}>{msg}</p>{/if}
</div>

<div class="card">
  <h2>母包装库存</h2>
  <table>
    <thead>
      <tr><th>ID</th><th>门店</th><th>药品</th><th>批号</th><th>开封时间</th><th>效期</th><th>总量</th><th>余量</th><th>状态</th></tr>
    </thead>
    <tbody>
      {#each mothers as m}
        <tr>
          <td>{m.id}</td><td>{m.store_name}</td><td>{m.drug_name}</td><td>{m.batch_no}</td>
          <td>{m.opened_at}</td><td>{m.expiry_date}</td>
          <td>{m.total_qty}{m.unit}</td><td>{m.remaining_qty}{m.unit}</td>
          <td>{statusText[m.status] || m.status}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .row { display: flex; gap: 12px; align-items: flex-end; flex-wrap: wrap; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: #555; }
</style>
