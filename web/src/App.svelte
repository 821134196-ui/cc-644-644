<script>
  import { onMount } from 'svelte'
  import { api } from './api.js'
  import Counter from './Counter.svelte'
  import Mothers from './Mothers.svelte'
  import Returns from './Returns.svelte'
  import Transfers from './Transfers.svelte'
  import Recalls from './Recalls.svelte'

  let tab = 'counter'
  let meta = { stores: [], drugs: [] }
  let mothers = []
  let error = ''

  async function reload() {
    try {
      ;[meta, mothers] = await Promise.all([api.get('/api/meta'), api.get('/api/mothers')])
      error = ''
    } catch (e) {
      error = e.message
    }
  }
  onMount(reload)

  const tabs = [
    ['counter', '柜台拆零'],
    ['mothers', '母包装登记'],
    ['returns', '退药 / 待处理库存'],
    ['transfers', '门店调拨'],
    ['recalls', '批次召回']
  ]
</script>

<header>
  <h1>连锁药店拆零管理系统</h1>
  <nav>
    {#each tabs as [key, label]}
      <button class:active={tab === key} on:click={() => (tab = key)}>{label}</button>
    {/each}
  </nav>
</header>

{#if error}<p class="error">{error}</p>{/if}

<main>
  {#if tab === 'counter'}
    <Counter {mothers} on:done={reload} />
  {:else if tab === 'mothers'}
    <Mothers {meta} {mothers} on:done={reload} />
  {:else if tab === 'returns'}
    <Returns {meta} />
  {:else if tab === 'transfers'}
    <Transfers {meta} {mothers} on:done={reload} />
  {:else if tab === 'recalls'}
    <Recalls {meta} />
  {/if}
</main>

<style>
  :global(body) { font-family: system-ui, 'PingFang SC', 'Microsoft YaHei', sans-serif; margin: 0; background: #f4f6f8; color: #222; }
  header { background: #14532d; color: #fff; padding: 12px 24px; }
  header h1 { font-size: 18px; margin: 0 0 10px; }
  nav { display: flex; gap: 8px; flex-wrap: wrap; }
  nav button { background: transparent; color: #d1fae5; border: 1px solid #4ade80; border-radius: 6px; padding: 6px 14px; cursor: pointer; }
  nav button.active, nav button:hover { background: #16a34a; color: #fff; }
  main { padding: 20px 24px; max-width: 1100px; margin: 0 auto; }
  .error { color: #b91c1c; padding: 0 24px; }
  :global(table) { border-collapse: collapse; width: 100%; background: #fff; }
  :global(th, td) { border: 1px solid #e5e7eb; padding: 8px 10px; text-align: left; font-size: 14px; }
  :global(th) { background: #f0fdf4; }
  :global(button.primary) { background: #16a34a; color: #fff; border: none; border-radius: 6px; padding: 8px 18px; cursor: pointer; font-size: 14px; }
  :global(button.primary:disabled) { background: #9ca3af; cursor: not-allowed; }
  :global(input, select) { padding: 6px 8px; border: 1px solid #d1d5db; border-radius: 6px; font-size: 14px; }
  :global(.card) { background: #fff; border-radius: 8px; padding: 16px 20px; margin-bottom: 16px; box-shadow: 0 1px 3px rgba(0,0,0,.08); }
  :global(.msg-ok) { color: #15803d; }
  :global(.msg-err) { color: #b91c1c; }
</style>
