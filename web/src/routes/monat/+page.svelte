<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { thisMonth, shiftMonth, monthLabel, dateDe, STATUS_LABEL, requestText } from '$lib/fmt';
  import MonthTable from '$lib/MonthTable.svelte';
  import CorrectionDialog from '$lib/CorrectionDialog.svelte';

  let monat = $state(thisMonth());
  let month = $state<any>(null);
  let requests = $state<any[]>([]);
  let error = $state('');
  let msg = $state('');
  let dialog: CorrectionDialog;
  let narrow = $state(false);
  let view = $state<'table' | 'cards'>('table');

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
  const pendingDays = $derived(new Set(requests.filter((r) => r.status === 'beantragt').map((r) => r.datum as string)));
  onMount(() => {
    const mq = window.matchMedia('(max-width: 640px)');
    const apply = () => { narrow = mq.matches; view = mq.matches ? 'cards' : 'table'; };
    apply();
    mq.addEventListener('change', apply);
    load();
    return () => mq.removeEventListener('change', apply);
  });
  function go(delta: number) {
    monat = shiftMonth(monat, delta);
    load();
  }
</script>

<div class="page-head">
  <div class="row" style="gap:16px">
    <h1>Monatsübersicht</h1>
    <div class="monthnav">
      <button onclick={() => go(-1)} aria-label="Vormonat">‹</button>
      <span>{monthLabel(monat)}</span>
      <button onclick={() => go(1)} aria-label="Folgemonat">›</button>
    </div>
    {#if month}{#if month.geschlossen}<span class="badge ok">abgeschlossen</span>{:else}<span class="badge">offen</span>{/if}{/if}
  </div>
  {#if month?.geschlossen?.pdf}
    <a class="btn" href={`/api/reports/month/${month.employee.id}/${monat}/pdf`} target="_blank">Monatsbericht PDF</a>
  {:else if month}
    <a class="btn" href={`/api/reports/month/${month.employee.id}/${monat}/pdf?vorschau=1`} target="_blank">Vorschau PDF</a>
  {/if}
</div>
{#if error}<div class="alert err">{error}</div>{/if}
{#if msg}<div class="alert ok">{msg}</div>{/if}

{#if narrow}
  <div class="segment" style="margin-bottom:14px">
    <button class:active={view === 'cards'} onclick={() => (view = 'cards')}>Karten</button>
    <button class:active={view === 'table'} onclick={() => (view = 'table')}>Tabelle</button>
  </div>
{/if}
{#if month}<MonthTable {month} {view} pending={pendingDays} onrequest={(d) => dialog.open(d)} />{/if}
<CorrectionDialog bind:this={dialog} onsaved={() => { msg = 'Korrekturantrag gesendet.'; load(); }} />

{#if requests.length}
  <div class="card" style="margin-top:20px">
    <div class="card-title">Meine Korrekturanträge</div>
    <div class="rowlist">
      {#each requests as r}
        <div>
          <span style="width:90px;flex-shrink:0">{dateDe(r.datum)}</span>
          <span style="flex:1">{requestText(r)}</span>
          <span class="muted small" style="flex:1">{r.begruendung}{#if r.entscheidung_kommentar} — {r.entscheidung_kommentar}{/if}</span>
          <span class="badge" class:ok={r.status === 'genehmigt'} class:warn={r.status === 'beantragt'} class:err={r.status === 'abgelehnt'}>{STATUS_LABEL[r.status]}</span>
          {#if r.status === 'beantragt'}<button class="small" onclick={() => withdraw(r.id)}>Zurückziehen</button>{/if}
        </div>
      {/each}
    </div>
  </div>
{/if}
