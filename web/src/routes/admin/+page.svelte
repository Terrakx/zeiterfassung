<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { hm } from '$lib/fmt';
  let rows = $state<any[]>([]);
  let error = $state('');
  onMount(async () => {
    try { rows = await api.get('/calc/overview'); } catch (e) { error = errMsg(e); }
  });
  const offen = $derived(rows.reduce((s, r) => s + r.offene_antraege, 0));
</script>

<h1>Übersicht heute</h1>
{#if error}<div class="alert err">{error}</div>{/if}
{#if offen}<div class="alert info"><a href="/admin/antraege">{offen} offene Anträge</a></div>{/if}
<div class="card table-wrap" style="padding:0">
  <table>
    <thead><tr><th>Nr</th><th>Name</th><th>Status</th><th class="right">Ist heute</th><th class="right">Soll</th><th class="right">Saldo</th><th>Hinweise</th></tr></thead>
    <tbody>
      {#each rows as r}
        <tr>
          <td class="mono">{r.personalnr}</td>
          <td><a href={`/admin/mitarbeiter/${r.id}`}>{r.name}</a>{#if r.rolle === 'admin'} <span class="badge">Admin</span>{/if}</td>
          <td>
            {#if r.abwesenheit}<span class="badge info">{r.abwesenheit}</span>
            {:else if r.zustand === 'arbeitet'}<span class="badge ok">anwesend</span>
            {:else if r.zustand === 'pause'}<span class="badge warn">Pause</span>
            {:else}<span class="badge">abwesend</span>{/if}
          </td>
          <td class="right mono">{hm(r.heute_ist_min)}</td>
          <td class="right mono">{hm(r.heute_soll_min)}</td>
          <td class="right mono" class:pos={r.saldo_min > 0} class:neg={r.saldo_min < 0}>{hm(r.saldo_min, true)}</td>
          <td>{#if r.warnungen_heute}<span class="badge warn">{r.warnungen_heute}</span>{/if}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>
