<script>
  import { createEventDispatcher } from 'svelte'
  import { api } from './api.js'
  export let mothers = []
  const dispatch = createEventDispatcher()

  let motherId = ''
  let qty = 1
  let msg = ''
  let msgOk = false
  let label = null
  let remainingAfter = null
  let busy = false

  $: mother = mothers.find((m) => m.id === Number(motherId))
  $: after = mother ? mother.remaining_qty - qty : null
  $: insufficient = mother ? qty > mother.remaining_qty : false
  $: blocked = !mother || qty < 1 || insufficient || mother.status !== 'ACTIVE'

  async function submit() {
    busy = true
    msg = ''
    label = null
    try {
      const r = await api.post('/api/repacks', { mother_id: Number(motherId), qty })
      msgOk = true
      msg = `拆零单 #${r.order_id} 已售出并打印`
      label = r.label
      remainingAfter = r.remaining_after
      dispatch('done')
    } catch (e) {
      msgOk = false
      msg = e.message
      dispatch('done')
    } finally {
      busy = false
    }
  }
</script>

<div class="card">
  <h2>柜台拆零</h2>
  <div class="row">
    <label>母包装
      <select bind:value={motherId}>
        <option value="">请选择</option>
        {#each mothers as m}
          <option value={m.id} disabled={m.status !== 'ACTIVE'}>
            #{m.id} {m.drug_name}（批号 {m.batch_no}，余 {m.remaining_qty}{m.unit}，{m.store_name}）{m.status !== 'ACTIVE' ? ' [已' + (m.status === 'RECALLED' ? '召回' : '售罄') + ']' : ''}
          </option>
        {/each}
      </select>
    </label>
    <label>本次拆零数量
      <input type="number" min="1" bind:value={qty} />
    </label>
    <button class="primary" disabled={blocked || busy} on:click={submit}>
      {busy ? '处理中…' : '打印标签并售出'}
    </button>
  </div>

  {#if mother}
    <p class="summary">
      当前余量 <b>{mother.remaining_qty}</b> {mother.unit} →
      拆后余量
      <b class:neg={after < 0}>{after}</b> {mother.unit}
      {#if insufficient}<span class="msg-err">（库存不足，不能销售）</span>{/if}
    </p>
  {/if}
  {#if msg}<p class={msgOk ? 'msg-ok' : 'msg-err'}>{msg}</p>{/if}
</div>

{#if label}
  <div class="card label">
    <h3>已打印标签（拆零单 #{label.order_id}，拆后余量 {remainingAfter}）</h3>
    <div class="ticket">
      <p class="drug">{label.drug_name} {label.spec}</p>
      <p>批号：{label.batch_no}</p>
      <p>用法：{label.usage_text}</p>
      <p>有效期至：{label.expiry_date}</p>
      <p>数量：{label.qty} {label.unit}　门店：{label.store}</p>
    </div>
  </div>
{/if}

<style>
  .row { display: flex; gap: 16px; align-items: flex-end; flex-wrap: wrap; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: #555; }
  select { min-width: 340px; }
  .summary { font-size: 15px; }
  .neg { color: #b91c1c; }
  .ticket { border: 2px dashed #16a34a; border-radius: 8px; padding: 12px 18px; max-width: 380px; background: #f0fdf4; }
  .ticket .drug { font-weight: 700; font-size: 16px; margin: 0 0 6px; }
  .ticket p { margin: 3px 0; font-size: 14px; }
</style>
