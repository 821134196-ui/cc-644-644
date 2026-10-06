<script>
  import { onMount } from 'svelte';
  import { get, post, getStoreId, setStoreId } from './lib/api.js';
  import Counter from './pages/Counter.svelte';
  import Packs from './pages/Packs.svelte';
  import Returns from './pages/Returns.svelte';
  import Transfers from './pages/Transfers.svelte';
  import Recall from './pages/Recall.svelte';

  let tab = $state(localStorage.getItem('tab') || 'counter');
  let stores = $state([]);
  let drugs = $state([]);
  let storeId = $state(getStoreId());
  let bootError = $state('');

  function switchTab(t) {
    tab = t;
    localStorage.setItem('tab', t);
  }

  async function loadBase() {
    try {
      [stores, drugs] = await Promise.all([get('/api/stores'), get('/api/drugs')]);
      if (stores.length === 0) {
        await post('/api/seed', {});
        [stores, drugs] = await Promise.all([get('/api/stores'), get('/api/drugs')]);
      }
      if (!stores.find((s) => s.id === storeId)) {
        storeId = stores[0].id;
        setStoreId(storeId);
      }
    } catch (e) {
      bootError = `无法连接后端: ${e.message}(请确认 Axum 已启动于 8080 端口)`;
    }
  }

  function changeStore(e) {
    storeId = Number(e.target.value);
    setStoreId(storeId);
    // 强制重建当前页面以刷新门店相关数据
    tab = tab;
  }

  onMount(loadBase);

  const tabs = [
    ['counter', '柜台拆零'],
    ['packs', '母包装登记 / 库存'],
    ['returns', '退药待处理'],
    ['transfers', '门店调拨'],
    ['recall', '批次召回'],
  ];
</script>

<header class="topbar">
  <h1>💊 连锁药店拆零药品管理系统</h1>
  <nav class="tabs">
    {#each tabs as [key, label]}
      <button class={tab === key ? 'active' : ''} onclick={() => switchTab(key)}>{label}</button>
    {/each}
  </nav>
  <div class="store-switch">
    <span>当前门店</span>
    <select value={storeId} onchange={changeStore}>
      {#each stores as s}
        <option value={s.id}>{s.name}（{s.code}）</option>
      {/each}
    </select>
  </div>
</header>

<main>
  {#if bootError}
    <div class="alert err">{bootError}</div>
  {:else}
    {#key storeId}
      {#if tab === 'counter'}
        <Counter {stores} {drugs} {storeId} />
      {:else if tab === 'packs'}
        <Packs {stores} {drugs} {storeId} onChanged={loadBase} />
      {:else if tab === 'returns'}
        <Returns {stores} {drugs} {storeId} />
      {:else if tab === 'transfers'}
        <Transfers {stores} {drugs} {storeId} />
      {:else if tab === 'recall'}
        <Recall {stores} {drugs} {storeId} />
      {/if}
    {/key}
  {/if}
</main>
