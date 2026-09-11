<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { thisMonth, shiftMonth, monthLabel } from '$lib/fmt';
  import MonthTable from '$lib/MonthTable.svelte';

  let monat = $state(thisMonth());
  let month = $state<any>(null);
  let error = $state('');

  async function load() {
    error = '';
    try {
      month = await api.get(`/calc/month?monat=${monat}`);
    } catch (e) {
      error = errMsg(e);
    }
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
{#if month}<MonthTable {month} />{/if}
