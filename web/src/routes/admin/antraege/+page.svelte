<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { dateDe, STATUS_LABEL, PUNCH_LABELS } from '$lib/fmt';

  let status = $state('beantragt');
  let list = $state<any[]>([]);
  let corrections = $state<any[]>([]);
  let error = $state('');

  async function load() {
    try {
      [list, corrections] = await Promise.all([
        api.get<any[]>(`/admin/absences?status=${status}`),
        api.get<any[]>(`/admin/punch-requests?status=${status}`)
      ]);
    } catch (e) { error = errMsg(e); }
  }
  async function decideCorrection(id: number, s: 'genehmigt' | 'abgelehnt') {
    const kommentar = s === 'abgelehnt' ? prompt('Begründung für die Ablehnung:') ?? '' : '';
    if (s === 'abgelehnt' && !kommentar) return;
    error = '';
    try { await api.post(`/punch-requests/${id}/decide`, { status: s, kommentar }); await load(); }
    catch (e) { error = errMsg(e); }
  }
  onMount(load);

  async function decide(id: number, s: 'genehmigt' | 'abgelehnt') {
    const kommentar = s === 'abgelehnt' ? prompt('Begründung für die Ablehnung:') ?? '' : '';
    if (s === 'abgelehnt' && !kommentar) return;
    error = '';
    try { await api.post(`/absences/${id}/decide`, { status: s, kommentar }); await load(); }
    catch (e) { error = errMsg(e); }
  }
  async function storno(id: number) {
    const grund = prompt('Begründung für das Storno:');
    if (!grund) return;
    try { await api.post(`/absences/${id}/storno`, { grund }); await load(); }
    catch (e) { error = errMsg(e); }
  }
</script>

<div class="page-head">
  <h1>Anträge</h1>
  <select style="width:auto" bind:value={status} onchange={load}>
    <option value="beantragt">offen</option>
    <option value="genehmigt">genehmigt</option>
    <option value="abgelehnt">abgelehnt</option>
    <option value="storniert">storniert</option>
    <option value="alle">alle</option>
  </select>
</div>
{#if error}<div class="alert err">{error}</div>{/if}
<div class="card tight table-wrap">
  <table>
    <thead><tr><th>Mitarbeiter</th><th>Art</th><th>Von</th><th>Bis</th><th>Einheit</th><th>Kommentar</th><th>Status</th><th></th></tr></thead>
    <tbody>
      {#each list as a}
        <tr>
          <td>{a.name} <span class="muted small">({a.personalnr})</span></td>
          <td>{a.label}</td><td class="mono">{dateDe(a.von)}</td><td class="mono">{dateDe(a.bis)}</td>
          <td>{a.einheit === 'tag' ? 'Tage' : a.einheit === 'halber_tag' ? 'halber Tag' : `${a.wert} h`}</td>
          <td class="small">{a.kommentar ?? ''}</td>
          <td><span class="badge" class:ok={a.status === 'genehmigt'} class:warn={a.status === 'beantragt'} class:err={a.status === 'abgelehnt'}>{STATUS_LABEL[a.status]}</span></td>
          <td class="right" style="white-space:nowrap">
            {#if a.status === 'beantragt'}
              <button class="primary small" style="font-weight:500" onclick={() => decide(a.id, 'genehmigt')}>Genehmigen</button>
              <button class="small" onclick={() => decide(a.id, 'abgelehnt')}>Ablehnen</button>
            {:else if a.status === 'genehmigt'}
              <button class="small danger" onclick={() => storno(a.id)}>Stornieren</button>
            {/if}
          </td>
        </tr>
      {:else}
        <tr><td colspan="8" class="muted">Keine Einträge.</td></tr>
      {/each}
    </tbody>
  </table>
</div>

<h2>Korrekturanträge zu Stempelungen</h2>
<div class="card tight table-wrap">
  <table>
    <thead><tr><th>Mitarbeiter</th><th>Datum</th><th>Antrag</th><th>Begründung</th><th>Status</th><th></th></tr></thead>
    <tbody>
      {#each corrections as c}
        <tr>
          <td>{c.name} <span class="muted small">({c.personalnr})</span></td>
          <td class="mono">{dateDe(c.datum)}</td>
          <td>{#if c.typ === 'einfuegen'}{PUNCH_LABELS[c.art]} {c.zeit} nachtragen{:else}Streichen: {c.punch?.zeit ?? ''} {PUNCH_LABELS[c.punch?.art] ?? ''}{/if}</td>
          <td class="small">{c.begruendung}</td>
          <td><span class="badge" class:ok={c.status === 'genehmigt'} class:warn={c.status === 'beantragt'} class:err={c.status === 'abgelehnt'}>{STATUS_LABEL[c.status]}</span></td>
          <td class="right" style="white-space:nowrap">
            {#if c.status === 'beantragt'}
              <button class="primary small" style="font-weight:500" onclick={() => decideCorrection(c.id, 'genehmigt')}>Genehmigen</button>
              <button class="small" onclick={() => decideCorrection(c.id, 'abgelehnt')}>Ablehnen</button>
            {/if}
          </td>
        </tr>
      {:else}
        <tr><td colspan="6" class="muted">Keine Einträge.</td></tr>
      {/each}
    </tbody>
  </table>
</div>
