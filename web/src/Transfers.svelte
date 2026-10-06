<script>
  import { onMount } from 'svelte'
  import { createEventDispatcher } from 'svelte'
  import { api } from './api.js'
  export let meta = { stores: [], drugs: [] }
  export let mothers = []
  const dispatch = createEventDispatcher()

  let motherId = ''
  let toStore = 2
  let transfers = []
  let msg = ''
  let msgOk = false

  $: unopened = mothers.filter((m) => m.remaining_qty === m.total_qty && m.status === 'ACTIVE')

  async function load() {
    transfers = await api.get('/api/transfers')
  }
  onMount(load)

  async function submit() {
    try {
      await api.post('/api/transfers', { mother_id: Number(motherId), to_store_id: Number(toStore) })
      msgOk = true
      msg = '调拨完成'
      await load()
      dispatch('done')
    } catch (e) {
      msgOk = false
      msg = e.message
    }
  }
</script>

<div class="card">
  <h2>门店调拨</h2>
  <p class="hint">仅允许调拨<b>未拆零的整包</b>药品；已拆零的包装不能调拨。</p>
  <div class="row">
    <label>整包（未拆零）
      <select bind:value={motherId}>
        <option value="">请选择</option>
        {#each unopened as m}
          <option value={m.id}>#{m.id} {m.drug_name}（批号 {m.batch_no}，{m.total_qty}{m.unit}，{m.store_name}）</option>
        {/each}
      </select>
    </label>
    <label>调入门店
      <select bind:value={toStore}>
        {#each meta.stores as s}<option value={s.id}>{s.name}</option>{/each}
      </select>
    </label>
    <button class="primary" disabled={!motherId} on:click={submit}>调拨</button>
  </div>
  {#if msg}<p class={msgOk ? 'msg-ok' : 'msg-err'}>{msg}</p>{/if}
</div>

<div class="card">
  <h2>调拨记录</h2>
  <table>
    <thead><tr><th>ID</th><th>母包装</th><th>数量</th><th>调出</th><th>调入</th><th>时间</th></tr></thead>
    <tbody>
      {#each transfers as t}
        <tr><td>{t.id}</td><td>#{t.mother_id}</td><td>{t.qty}</td><td>{t.from_store}</td><td>{t.to_store}</td><td>{t.created_at}</td></tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .row { display: flex; gap: 12px; align-items: flex-end; flex-wrap: wrap; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: #555; }
  select { min-width: 320px; }
  .hint { color: #92400e; background: #fffbeb; padding: 8px 12px; border-radius: 6px; }
</style>
