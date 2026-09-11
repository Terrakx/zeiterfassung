<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api, errMsg } from '$lib/api';
  import { dateDe, todayIso } from '$lib/fmt';

  let list = $state<any[]>([]);
  let q = $state('');
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
  const filtered = $derived(list.filter((e) => {
    const s = q.trim().toLowerCase();
    return !s || `${e.vorname} ${e.nachname} ${e.username} ${e.personalnr}`.toLowerCase().includes(s);
  }));
  const aktiv = $derived(list.filter((e) => e.aktiv).length);

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

<div class="page-head">
  <h1>Mitarbeiter</h1>
  <div class="row">
    <input class="search" placeholder="Suchen …" bind:value={q} />
    <button class="primary" onclick={() => dlg.showModal()}>Neu anlegen</button>
  </div>
</div>
{#if error}<div class="alert err">{error}</div>{/if}
<div class="card tight table-wrap">
  <table>
    <thead><tr><th class="right">Nr.</th><th>Name</th><th>Benutzer</th><th>Rolle</th><th>Eintritt</th><th>Austritt</th><th class="right">Urlaub/Jahr</th><th>Aktiv</th><th></th></tr></thead>
    <tbody>
      {#each filtered as e}
        <tr>
          <td class="right muted">{e.personalnr}</td>
          <td><a href={`/admin/mitarbeiter/${e.id}`} style="font-weight:500">{e.vorname} {e.nachname}</a></td>
          <td style="color:var(--ink-2)">{e.username}</td>
          <td>{e.rolle === 'admin' ? 'Admin' : 'Mitarbeiter'}</td>
          <td>{dateDe(e.eintritt)}</td>
          <td class="muted">{e.austritt ? dateDe(e.austritt) : '–'}</td>
          <td class="right">{e.urlaubsanspruch_tage}</td>
          <td>{#if e.aktiv}<span class="badge ok">aktiv</span>{:else}<span class="badge">inaktiv</span>{/if}</td>
          <td class="right"><a class="btn small" href={`/admin/mitarbeiter/${e.id}`}>Öffnen</a></td>
        </tr>
      {:else}
        <tr><td colspan="9" class="muted">Keine Treffer.</td></tr>
      {/each}
    </tbody>
  </table>
  <div class="table-foot"><span>{list.length} Mitarbeitende · {aktiv} aktiv</span></div>
</div>

<dialog bind:this={dlg}>
  <h2>Mitarbeiter anlegen</h2>
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
      <div class="field"><label for="dm">Durchrechnung (Monate)</label><input id="dm" type="number" min="1" max="12" bind:value={f.durchrechnung_monate} /></div>
      <div class="field"><label for="pw">Passwort</label><input id="pw" type="text" bind:value={f.passwort} autocomplete="off" /><span class="help">Leer lassen, wenn kein Portal-Login gewünscht ist.</span></div>
      <div class="field"><label for="pi">Terminal-PIN (4 bis 8 Ziffern)</label><input id="pi" type="text" bind:value={f.pin} inputmode="numeric" autocomplete="off" /></div>
    </div>
    <div class="field" style="margin-top:16px">
      <span style="display:block;margin-bottom:6px;font-weight:500;color:var(--ink-2);font-size:13px">Wochenmodell in Stunden</span>
      <div style="display:grid;grid-template-columns:repeat(7,1fr);gap:6px">
        {#each ['Mo','Di','Mi','Do','Fr','Sa','So'] as d, i}
          <div><span class="xs muted" style="display:block;text-align:center;margin-bottom:4px">{d}</span><input type="number" step="0.25" min="0" max="24" bind:value={f.wochenmodell[i]} style="padding:0 6px;text-align:center" aria-label={d} /></div>
        {/each}
      </div>
    </div>
    <div class="row" style="justify-content:flex-end;margin-top:8px">
      <button type="button" onclick={() => dlg.close()}>Abbrechen</button>
      <button class="primary">Anlegen</button>
    </div>
  </form>
</dialog>
