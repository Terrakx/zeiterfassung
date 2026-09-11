<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { thisMonth, shiftMonth, monthLabel } from '$lib/fmt';
  import MonthTable from '$lib/MonthTable.svelte';
  import CorrectionDialog from '$lib/CorrectionDialog.svelte';
  import { dateDe, PUNCH_LABELS, STATUS_LABEL } from '$lib/fmt';

  let monat = $state(thisMonth());
  let month = $state<any>(null);
  let requests = $state<any[]>([]);
  let error = $state('');
  let msg = $state('');
  let dialog: CorrectionDialog;

  async function load() {
    error = '';
    try {
      const [m, r] = await Promise.all([
        api.get<any>(`/calc/month?monat=${monat}`),
        api.get<any[]>(`/punch-requests?von=${monat}-01&bis=${monat}-31`)
      ]);
      month = m; requests = r;
    } catch (e) {
      error = errMsg(e);
    }
  }
  async function withdraw(id: number) {
    try { await api.post(`/punch-requests/${id}/withdraw`); await load(); } catch (e) { error = errMsg(e); }
  }
  onMount(load);
  function go(delta: number) {
    monat = shiftMonth(monat, delta);
    load();
  }
</script>

<div class="row" style="justify-content:space-between;margin-bottom:1rem">
  <h1 style="margin:0">Monatsübersicht</h1>
  <div class="row">
    <button onclick={() => go(-1)}>‹</button>
    <strong style="min-width:150px;text-align:center">{monthLabel(monat)}</strong>
    <button onclick={() => go(1)}>›</button>
    {#if month?.geschlossen}<span class="badge ok">abgeschlossen</span>{/if}
    {#if month?.geschlossen?.pdf}<a class="btn" href={`/api/reports/month/${month.employee.id}/${monat}/pdf`} target="_blank">PDF</a>{/if}
  </div>
</div>
{#if error}<div class="alert err">{error}</div>{/if}
{#if msg}<div class="alert ok">{msg}</div>{/if}
{#if month}<MonthTable {month} onrequest={(d) => dialog.open(d)} />{/if}
<CorrectionDialog bind:this={dialog} onsaved={() => { msg = 'Korrekturantrag gesendet.'; load(); }} />

{#if requests.length}
  <h2>Korrekturanträge</h2>
  <div class="card table-wrap" style="padding:0">
    <table>
      <thead><tr><th>Datum</th><th>Antrag</th><th>Begründung</th><th>Status</th><th></th></tr></thead>
      <tbody>
        {#each requests as r}
          <tr>
            <td class="mono">{dateDe(r.datum)}</td>
            <td>{r.typ === 'einfuegen' ? `${PUNCH_LABELS[r.art]} ${r.zeit} nachtragen` : 'Stempelung streichen'}</td>
            <td class="small">{r.begruendung}{#if r.entscheidung_kommentar} <span class="muted">— {r.entscheidung_kommentar}</span>{/if}</td>
            <td><span class="badge">{STATUS_LABEL[r.status]}</span></td>
            <td>{#if r.status === 'beantragt'}<button class="small" onclick={() => withdraw(r.id)}>Zurückziehen</button>{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}
