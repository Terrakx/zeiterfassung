<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { dateDe } from '$lib/fmt';

  let jahr = $state(new Date().getFullYear());
  let list = $state<any[]>([]);
  let error = $state('');
  let datum = $state('');
  let name = $state('');

  async function load() {
    try { list = await api.get(`/holidays?jahr=${jahr}`); } catch (e) { error = errMsg(e); }
  }
  onMount(load);
  async function add(e: Event) {
    e.preventDefault();
    try { await api.post('/holidays', { datum, name }); datum = name = ''; await load(); } catch (err) { error = errMsg(err); }
  }
  async function del(d: string) {
    if (!confirm('Betrieblichen Feiertag entfernen?')) return;
    try { await api.del(`/holidays/${d}`); await load(); } catch (err) { error = errMsg(err); }
  }
</script>

<div class="page-head">
  <h1>Feiertage</h1>
  <div class="monthnav"><button onclick={() => { jahr--; load(); }}>‹</button><span style="min-width:90px">{jahr}</span><button onclick={() => { jahr++; load(); }}>›</button></div>
</div>
{#if error}<div class="alert err">{error}</div>{/if}
<div class="grid cols-2">
  <div class="card tight table-wrap" style="margin:0">
    <table>
      <thead><tr><th>Datum</th><th>Name</th><th></th></tr></thead>
      <tbody>
        {#each list as h}
          <tr><td class="mono">{dateDe(h.datum)}</td><td>{h.name}{#if h.betrieblich} <span class="badge info">betrieblich</span>{/if}</td>
            <td class="right">{#if h.betrieblich}<button class="small" onclick={() => del(h.datum)}>Entfernen</button>{/if}</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
  <div class="card" style="margin:0">
    <div class="card-title">Betrieblichen freien Tag hinzufügen</div>
    <p class="small muted">Gesetzliche Feiertage nach § 7 ARG werden automatisch berechnet. Hier können Landesfeiertage (z. B. Josefitag) oder betriebliche freie Tage ergänzt werden. Diese Tage gelten wie Feiertage (Soll entfällt, bezahlt).</p>
    <form onsubmit={add}>
      <div class="field"><label for="d">Datum</label><input id="d" type="date" bind:value={datum} required /></div>
      <div class="field"><label for="n">Bezeichnung</label><input id="n" bind:value={name} required /></div>
      <button class="primary">Hinzufügen</button>
    </form>
  </div>
</div>
