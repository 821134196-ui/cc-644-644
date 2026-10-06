<script>
  import { onMount } from 'svelte'
  import { api } from './api.js'
  export let meta = { stores: [], drugs: [] }

  let drugId = 1
  let batchNo = ''
  let result = null
  let recalls = []
  let msg = ''
  let msgOk = false

  async function loadRecalls() {
    recalls = await api.get('/api/recalls')
  }
  onMount(loadRecalls)

  async function lookup() {
    msg = ''
    try {
      result = await api.get(`/api/recalls/lookup?drug_id=${drugId}&batch_no=${encodeURIComponent(batchNo)}`)
    } catch (e) {
      result = null
      msgOk = false
      msg = e.message
    }
  }

  async function execute() {
    try {
      const r = await api.post('/api/recalls', { drug_id: Number(drugId), batch_no: batchNo })
      msgOk = true
      msg = `已召回 ${r.recalled_mothers} 个母包装，该批次禁止继续拆零销售`
      await Promise.all([lookup(), loadRecalls()])
    } catch (e) {
      msgOk = false
      msg = e.message
    }
  }
</script>

<div class="card">
  <h2>批次召回</h2>
  <div class="row">
    <label>药品
      <select bind:value={drugId}>
        {#each meta.drugs as d}<option value={d.id}>{d.name}</option>{/each}
      </select>
    </label>
    <label>批号 <input bind:value={batchNo} placeholder="输入批号" /></label>
    <button class="primary" disabled={!batchNo} on:click={lookup}>查询追溯</button>
    {#if result && result.mothers.length > 0}
      <button class="primary danger" on:click={execute}>执行召回</button>
    {/if}
  </div>
  {#if msg}<p class={msgOk ? 'msg-ok' : 'msg-err'}>{msg}</p>{/if}
</div>

{#if result}
  <div class="card">
    <h3>相关母包装（{result.mothers.length}）</h3>
    <table>
      <thead><tr><th>ID</th><th>门店</th><th>批号</th><th>总量</th><th>余量</th><th>状态</th></tr></thead>
      <tbody>
        {#each result.mothers as m}
          <tr><td>{m.id}</td><td>{m.store_name}</td><td>{m.batch_no}</td><td>{m.total_qty}</td><td>{m.remaining_qty}</td><td>{m.status}</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
  <div class="card">
    <h3>全部拆零销售记录（{result.repack_records.length}）</h3>
    <table>
      <thead><tr><th>拆零单</th><th>母包装</th><th>门店</th><th>数量</th><th>状态</th><th>时间</th></tr></thead>
      <tbody>
        {#each result.repack_records as r}
          <tr><td>#{r.id}</td><td>#{r.mother_id}</td><td>{r.store_name}</td><td>{r.qty}</td><td>{r.status}</td><td>{r.created_at}</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

<div class="card">
  <h3>召回历史</h3>
  <table>
    <thead><tr><th>ID</th><th>药品</th><th>批号</th><th>召回时间</th></tr></thead>
    <tbody>
      {#each recalls as r}
        <tr><td>{r.id}</td><td>{r.drug_name}</td><td>{r.batch_no}</td><td>{r.created_at}</td></tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .row { display: flex; gap: 12px; align-items: flex-end; flex-wrap: wrap; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: #555; }
  button.danger { background: #b91c1c; }
</style>
