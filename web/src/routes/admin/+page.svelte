<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { hm } from '$lib/fmt';
  import TeamCalendar from '$lib/TeamCalendar.svelte';
  let rows = $state<any[]>([]);
  let error = $state('');
  let clockDiff = $state<number | null>(null);
  onMount(async () => {
    try {
      const [r, t]: any[] = await Promise.all([api.get('/calc/overview'), api.get('/time')]);
      rows = r;
      clockDiff = Math.round((Date.now() - new Date(t.utc).getTime()) / 1000);
    } catch (e) { error = errMsg(e); }
  });
  const offen = $derived(rows.reduce((s, r) => s + r.offene_antraege, 0));
  const anwesend = $derived(rows.filter((r) => r.zustand !== 'draussen').length);
  const today = new Date().toLocaleDateString('de-AT', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
</script>

<div class="page-head">
  <div><h1>Übersicht heute</h1></div>
  <div class="row"><span class="page-date">{today}</span><a class="btn" href="/api/reports/vacation-overview/pdf" target="_blank">Urlaubsübersicht PDF</a></div>
</div>
{#if error}<div class="alert err">{error}</div>{/if}
{#if clockDiff !== null && Math.abs(clockDiff) > 120}
  <div class="alert warn">Die Serveruhr weicht um {Math.round(Math.abs(clockDiff) / 60)} Minuten von diesem Gerät ab. Stempelzeiten kommen vom Server, bitte die Uhr des Servers prüfen.</div>
{/if}
{#if offen}<div class="alert info"><span><a href="/admin/antraege">{offen} offene Anträge</a> warten auf Entscheidung.</span></div>{/if}
<div class="card tight table-wrap">
  <table>
    <thead><tr><th class="right">Nr.</th><th>Name</th><th>Status</th><th class="right">Ist heute</th><th class="right">Soll</th><th class="right">Saldo</th><th>Hinweise</th><th></th></tr></thead>
    <tbody>
      {#each rows as r}
        <tr>
          <td class="right muted">{r.personalnr}</td>
          <td><a href={`/admin/mitarbeiter/${r.id}`} style="font-weight:500">{r.name}</a>{#if r.rolle === 'admin'} <span class="badge">Admin</span>{/if}</td>
          <td>
            {#if r.abwesenheit}<span class="badge info">{r.abwesenheit}</span>
            {:else if r.zustand === 'arbeitet'}<span class="badge ok"><span class="dot"></span>anwesend</span>
            {:else if r.zustand === 'pause'}<span class="badge warn">Pause</span>
            {:else}<span class="badge">abwesend</span>{/if}
          </td>
          <td class="right">{hm(r.heute_ist_min)}</td>
          <td class="right">{hm(r.heute_soll_min)}</td>
          <td class="right" style="font-weight:500" class:pos={r.saldo_min > 0} class:neg={r.saldo_min < 0}>{hm(r.saldo_min, true)}</td>
          <td>{#if r.warnungen_heute}<span class="badge warn">{r.warnungen_heute} Hinweise</span>{/if}{#if r.offene_antraege}<span class="badge">{r.offene_antraege} Anträge</span>{/if}</td>
          <td class="right"><a class="btn small" href={`/admin/mitarbeiter/${r.id}`}>Öffnen</a></td>
        </tr>
      {/each}
    </tbody>
  </table>
  <div class="table-foot"><span>{rows.length} Mitarbeitende · {anwesend} anwesend</span></div>
</div>

<TeamCalendar />
