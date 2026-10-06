<script>
  import { onMount } from 'svelte';
  import { get, post } from '../lib/api.js';

  let { stores, drugs, storeId } = $props();

  let packs = $state([]);
  let transfers = $state([]);
  let selectedId = $state(null);
  let toStoreId = $state('');
  let error = $state('');
  let ok = $state('');

  $effect(() => {
    const candidates = stores.filter((s) => s.id !== storeId);
    if ((!toStoreId || !candidates.find((s) => s.id === Number(toStoreId))) && candidates.length) {
      toStoreId = candidates[0].id;
    }
  });

  async function load() {
    packs = await get(`/api/packs?store_id=${storeId}`);
    transfers = await get('/api/transfers');
    if (!selectedId && packs.length) selectedId = packs[0].id;
  }

  const selectedPack = $derived(packs.find((p) => p.id === Number(selectedId)));
  const sealed = $derived(
    selectedPack && selectedPack.opened === 0 && selectedPack.remaining_qty === selectedPack.initial_qty
  );

  async function submit() {
    error = '';
    ok = '';
    try {
      const t = await post('/api/transfers', {
        parent_pack_id: Number(selectedId),
        to_store_id: Number(toStoreId),
      });
      ok = `整包已调拨：${t.drug_name}（批号 ${t.batch_no}，${t.qty} 个最小单位）→ ${t.to_store_name}`;
      await load();
    } catch (e) {
      error = e.message;
    }
  }

  onMount(load);
</script>

<div class="grid2">
  <div class="panel">
    <h2>门店间调拨（仅限未拆零整包）</h2>
    <div class="alert warn">
      规则：只有<b>未开封且余量等于初始数量</b>的整包才能调拨；已拆零、已召回的包装禁止调出。
    </div>
    <label class="field">
      <span>调出门店：{stores.find((s) => s.id === storeId)?.name}</span>
      <select bind:value={selectedId}>
        {#each packs as p}
          <option value={p.id}>
            #{p.id} {p.drug_name}｜批号 {p.batch_no}｜{p.remaining_qty}/{p.initial_qty}
            {p.unit_pack}｜{p.opened === 0 && p.remaining_qty === p.initial_qty ? '整包' : '已拆零'}
          </option>
        {/each}
      </select>
    </label>
    <label class="field">
      <span>调入门店</span>
      <select bind:value={toStoreId}>
        {#each stores.filter((s) => s.id !== storeId) as s}
          <option value={s.id}>{s.name}（{s.code}）</option>
        {/each}
      </select>
    </label>

    {#if selectedPack}
      {#if !sealed}
        <div class="alert err">该母盒已拆零或数量不符（{selectedPack.remaining_qty}/{selectedPack.initial_qty}），不能调拨。</div>
      {:else}
        <div class="alert ok">校验通过：未开封整包，{selectedPack.initial_qty} {selectedPack.unit_pack}，允许调拨。</div>
      {/if}
    {/if}
    {#if error}<div class="alert err">{error}</div>{/if}
    {#if ok}<div class="alert ok">{ok}</div>{/if}
    <button class="btn" onclick={submit} disabled={!sealed || Number(toStoreId) === storeId}>发起整包调拨</button>
  </div>

  <div class="panel">
    <h2>调拨记录</h2>
    <table>
      <thead><tr><th>#</th><th>药品 / 批号</th><th>数量</th><th>调出 → 调入</th><th>时间</th></tr></thead>
      <tbody>
        {#each transfers as t}
          <tr>
            <td>{t.id}</td>
            <td>{t.drug_name}<div class="muted mono">{t.batch_no}</div></td>
            <td>{t.qty}</td>
            <td>{t.from_store_name} → {t.to_store_name}</td>
            <td class="muted">{t.created_at}</td>
          </tr>
        {/each}
        {#if transfers.length === 0}<tr><td colspan="5" class="muted">暂无调拨记录</td></tr>{/if}
      </tbody>
    </table>
  </div>
</div>
