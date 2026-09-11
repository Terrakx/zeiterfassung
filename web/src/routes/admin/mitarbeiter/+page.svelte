<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api, errMsg } from '$lib/api';
  import { dateDe, todayIso } from '$lib/fmt';

  let list = $state<any[]>([]);
  let error = $state('');
  let dlg: HTMLDialogElement;
  let f = $state({
    personalnr: '', vorname: '', nachname: '', username: '', rolle: 'mitarbeiter', eintritt: todayIso(),
    urlaubsanspruch_tage: 25, urlaubsjahr_beginn_mm_dd: '01-01', durchrechnung_monate: 3, durchrechnung_start: todayIso(),
    passwort: '', pin: '', wochenmodell: [8, 8, 8, 8, 8, 0, 0]
  });

  async function load() {
    try { list = await api.get('/employees'); } catch (e) { error = errMsg(e); }
  }
  onMount(load);

  async function create(e: Event) {
    e.preventDefault();
    error = '';
    try {
      const r: any = await api.post('/employees', { ...f, wochenmodell: f.wochenmodell.map(Number) });
      dlg.close();
      goto(`/admin/mitarbeiter/${r.employee.id}`);
    } catch (err) { error = errMsg(err); }
  }
</script>

<div class="row" style="justify-content:space-between">
  <h1 style="margin:0">Mitarbeiter</h1>
  <button class="primary" onclick={() => dlg.showModal()}>Neu anlegen</button>
</div>
{#if error}<div class="alert err" style="margin-top:1rem">{error}</div>{/if}
<div class="card table-wrap" style="padding:0;margin-top:1rem">
  <table>
    <thead><tr><th>Nr</th><th>Name</th><th>Benutzer</th><th>Rolle</th><th>Eintritt</th><th>Austritt</th><th>Urlaub/Jahr</th><th>Status</th></tr></thead>
    <tbody>
      {#each list as e}
        <tr>
          <td class="mono">{e.personalnr}</td>
          <td><a href={`/admin/mitarbeiter/${e.id}`}>{e.nachname} {e.vorname}</a></td>
          <td>{e.username}</td><td>{e.rolle}</td>
          <td class="mono">{dateDe(e.eintritt)}</td><td class="mono">{dateDe(e.austritt)}</td>
          <td>{e.urlaubsanspruch_tage}</td>
          <td>{#if e.aktiv}<span class="badge ok">aktiv</span>{:else}<span class="badge">inaktiv</span>{/if}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<dialog bind:this={dlg}>
  <h2 style="margin-top:0">Mitarbeiter anlegen</h2>
  <form onsubmit={create}>
    <div class="form-grid">
      <div class="field"><label for="pn">Personalnummer (BMD)</label><input id="pn" bind:value={f.personalnr} required inputmode="numeric" /></div>
      <div class="field"><label for="un">Benutzername</label><input id="un" bind:value={f.username} required /></div>
      <div class="field"><label for="vn">Vorname</label><input id="vn" bind:value={f.vorname} required /></div>
      <div class="field"><label for="nn">Nachname</label><input id="nn" bind:value={f.nachname} required /></div>
      <div class="field"><label for="ei">Eintritt</label><input id="ei" type="date" bind:value={f.eintritt} required /></div>
      <div class="field"><label for="ds">Zeiterfassung ab</label><input id="ds" type="date" bind:value={f.durchrechnung_start} required /></div>
      <div class="field"><label for="ro">Rolle</label><select id="ro" bind:value={f.rolle}><option value="mitarbeiter">Mitarbeiter</option><option value="admin">Admin</option></select></div>
      <div class="field"><label for="ua">Urlaubsanspruch Tage/Jahr</label><input id="ua" type="number" step="0.5" bind:value={f.urlaubsanspruch_tage} /></div>
      <div class="field"><label for="uj">Urlaubsjahr beginnt (MM-TT)</label><input id="uj" bind:value={f.urlaubsjahr_beginn_mm_dd} pattern="[0-1][0-9]-[0-3][0-9]" /></div>
      <div class="field"><label for="dm">Durchrechnung Monate</label><input id="dm" type="number" min="1" max="12" bind:value={f.durchrechnung_monate} /></div>
      <div class="field"><label for="pw">Passwort (leer = kein Portal-Login)</label><input id="pw" type="text" bind:value={f.passwort} autocomplete="off" /></div>
      <div class="field"><label for="pi">Terminal-PIN (4 bis 8 Ziffern)</label><input id="pi" type="text" bind:value={f.pin} inputmode="numeric" autocomplete="off" /></div>
    </div>
    <label>Wochenmodell Stunden Mo–So</label>
    <div class="row" style="margin-bottom:1rem">
      {#each ['Mo','Di','Mi','Do','Fr','Sa','So'] as d, i}
        <div style="width:64px"><label for={'w'+i} style="text-align:center">{d}</label><input id={'w'+i} type="number" step="0.25" min="0" max="24" bind:value={f.wochenmodell[i]} /></div>
      {/each}
    </div>
    <div class="row">
      <button class="primary">Anlegen</button>
      <button type="button" onclick={() => dlg.close()}>Abbrechen</button>
    </div>
  </form>
</dialog>
