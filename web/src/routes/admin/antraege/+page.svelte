<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { dateDe, STATUS_LABEL } from '$lib/fmt';

  let status = $state('beantragt');
  let list = $state<any[]>([]);
  let error = $state('');

  async function load() {
    try { list = await api.get(`/admin/absences?status=${status}`); } catch (e) { error = errMsg(e); }
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

<div class="row" style="justify-content:space-between">
  <h1 style="margin:0">Anträge</h1>
  <select style="width:auto" bind:value={status} onchange={load}>
    <option value="beantragt">offen</option>
    <option value="genehmigt">genehmigt</option>
    <option value="abgelehnt">abgelehnt</option>
    <option value="storniert">storniert</option>
    <option value="alle">alle</option>
  </select>
</div>
{#if error}<div class="alert err" style="margin-top:1rem">{error}</div>{/if}
<div class="card table-wrap" style="padding:0;margin-top:1rem">
  <table>
    <thead><tr><th>Mitarbeiter</th><th>Art</th><th>Von</th><th>Bis</th><th>Einheit</th><th>Kommentar</th><th>Status</th><th></th></tr></thead>
    <tbody>
      {#each list as a}
        <tr>
          <td>{a.name} <span class="muted small">({a.personalnr})</span></td>
          <td>{a.label}</td><td class="mono">{dateDe(a.von)}</td><td class="mono">{dateDe(a.bis)}</td>
          <td>{a.einheit === 'tag' ? 'Tage' : a.einheit === 'halber_tag' ? 'halber Tag' : `${a.wert} h`}</td>
          <td class="small">{a.kommentar ?? ''}</td>
          <td><span class="badge">{STATUS_LABEL[a.status]}</span></td>
          <td class="row" style="flex-wrap:nowrap">
            {#if a.status === 'beantragt'}
              <button class="primary" onclick={() => decide(a.id, 'genehmigt')}>Genehmigen</button>
              <button onclick={() => decide(a.id, 'abgelehnt')}>Ablehnen</button>
            {:else if a.status === 'genehmigt'}
              <button class="danger" onclick={() => storno(a.id)}>Stornieren</button>
            {/if}
          </td>
        </tr>
      {:else}
        <tr><td colspan="8" class="muted">Keine Einträge.</td></tr>
      {/each}
    </tbody>
  </table>
</div>
