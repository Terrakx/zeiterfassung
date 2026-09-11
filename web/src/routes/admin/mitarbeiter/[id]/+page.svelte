<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { api, errMsg } from '$lib/api';
  import { dateDe, hm, thisMonth, shiftMonth, monthLabel, STATUS_LABEL, todayIso, days } from '$lib/fmt';
  import MonthTable from '$lib/MonthTable.svelte';
  import AbsenceForm from '$lib/AbsenceForm.svelte';

  const id = Number(page.params.id);
  let tab = $state<'stamm' | 'zeiten' | 'abwesenheit' | 'urlaub' | 'gutstunden'>('stamm');
  let emp = $state<any>(null);
  let schedules = $state<any[]>([]);
  let error = $state('');
  let msg = $state('');

  // Zeiten
  let monat = $state(thisMonth());
  let month = $state<any>(null);
  let np = $state({ zeit: '', art: 'kommen', kommentar: '' });

  // Abwesenheiten
  let absences = $state<any[]>([]);
  // Urlaub / Gutstunden
  let vacation = $state<any>(null);
  let ve = $state({ urlaubsjahr: '', art: 'korrektur', tage: 0, grund: '' });
  let credit = $state<any>(null);
  let ce = $state({ datum: todayIso(), topf: 307, minuten: 0, art: 'korrektur', grund: '' });
  let secret = $state('');

  // Wochenmodell
  let ns = $state({ gueltig_ab: todayIso(), stunden: [8, 8, 8, 8, 8, 0, 0], pause_auto: false, gleitzeit: false, gleitzeit_von: '', gleitzeit_bis: '', kernzeit_von: '', kernzeit_bis: '', uebertrag_max_plus_min: null as number | null, uebertrag_max_minus_min: null as number | null });

  async function load() {
    error = '';
    try {
      const r: any = await api.get(`/employees/${id}`);
      emp = r.employee; schedules = r.schedules;
    } catch (e) { error = errMsg(e); }
  }
  async function loadMonth() { try { month = await api.get(`/employees/${id}/month?monat=${monat}`); } catch (e) { error = errMsg(e); } }
  async function loadAbs() {
    const y = new Date().getFullYear();
    try { absences = await api.get(`/employees/${id}/absences?von=${y - 1}-01-01&bis=${y + 1}-12-31`); } catch (e) { error = errMsg(e); }
  }
  async function loadVac() { try { vacation = await api.get(`/employees/${id}/vacation`); ve.urlaubsjahr = vacation.urlaubsjahr_von; } catch (e) { error = errMsg(e); } }
  async function loadCredit() { try { credit = await api.get(`/employees/${id}/credit`); ce.topf = credit.standard_topf; } catch (e) { error = errMsg(e); } }
  onMount(load);
  $effect(() => {
    if (tab === 'zeiten') loadMonth();
    if (tab === 'abwesenheit') loadAbs();
    if (tab === 'urlaub') loadVac();
    if (tab === 'gutstunden') loadCredit();
  });

  async function saveStamm(e: Event) {
    e.preventDefault(); error = msg = '';
    try { await api.put(`/employees/${id}`, emp); msg = 'Gespeichert.'; await load(); } catch (err) { error = errMsg(err); }
  }
  async function toggleActive() {
    if (!confirm(emp.aktiv ? 'Mitarbeiter deaktivieren? Login und Stempeln werden gesperrt.' : 'Mitarbeiter wieder aktivieren?')) return;
    try { await api.del(`/employees/${id}`); await load(); } catch (err) { error = errMsg(err); }
  }
  async function resetSecret(kind: 'password' | 'pin') {
    if (!secret) return;
    try { await api.post(`/employees/${id}/${kind}`, { wert: secret }); msg = kind === 'pin' ? 'PIN gesetzt.' : 'Passwort gesetzt.'; secret = ''; } catch (err) { error = errMsg(err); }
  }
  async function addSchedule(e: Event) {
    e.preventDefault(); error = '';
    try { schedules = await api.post(`/employees/${id}/schedules`, { ...ns, stunden: ns.stunden.map(Number) }); msg = 'Wochenmodell gespeichert.'; } catch (err) { error = errMsg(err); }
  }
  async function delSchedule(sid: number) {
    if (!confirm('Wochenmodell löschen?')) return;
    try { schedules = await api.del(`/employees/${id}/schedules/${sid}`); } catch (err) { error = errMsg(err); }
  }
  async function insertPunch(e: Event) {
    e.preventDefault(); error = '';
    try { await api.post(`/employees/${id}/punches`, np); np.zeit = ''; np.kommentar = ''; await loadMonth(); } catch (err) { error = errMsg(err); }
  }
  async function stornoPunch(pid: number) {
    const grund = prompt('Begründung für das Storno:');
    if (!grund) return;
    try { await api.post(`/punches/${pid}/storno`, { grund }); await loadMonth(); } catch (err) { error = errMsg(err); }
  }
  async function stornoAbs(aid: number) {
    const grund = prompt('Begründung für das Storno:');
    if (!grund) return;
    try { await api.post(`/absences/${aid}/storno`, { grund }); await loadAbs(); } catch (err) { error = errMsg(err); }
  }
  async function addVacEntry(e: Event) {
    e.preventDefault(); error = '';
    try { await api.post(`/employees/${id}/vacation`, ve); ve.grund = ''; ve.tage = 0; await loadVac(); } catch (err) { error = errMsg(err); }
  }
  async function addCreditEntry(e: Event) {
    e.preventDefault(); error = '';
    try { await api.post(`/employees/${id}/credit`, ce); ce.grund = ''; ce.minuten = 0; await loadCredit(); } catch (err) { error = errMsg(err); }
  }
  const wm = (s: any) => [s.mo_min, s.di_min, s.mi_min, s.do_min, s.fr_min, s.sa_min, s.so_min].map((m: number) => (m / 60).toString().replace('.', ',')).join(' / ');
</script>

{#if emp}
  <div class="row" style="justify-content:space-between">
    <h1 style="margin:0">{emp.vorname} {emp.nachname} <span class="muted small">Nr. {emp.personalnr}</span> {#if !emp.aktiv}<span class="badge">inaktiv</span>{/if}</h1>
    <a href="/admin/mitarbeiter">← Liste</a>
  </div>
  <div class="tabs" style="margin-top:.8rem">
    <a href={'#'} class:active={tab === 'stamm'} onclick={(e) => { e.preventDefault(); tab = 'stamm'; }}>Stammdaten</a>
    <a href={'#'} class:active={tab === 'zeiten'} onclick={(e) => { e.preventDefault(); tab = 'zeiten'; }}>Zeiten</a>
    <a href={'#'} class:active={tab === 'abwesenheit'} onclick={(e) => { e.preventDefault(); tab = 'abwesenheit'; }}>Abwesenheiten</a>
    <a href={'#'} class:active={tab === 'urlaub'} onclick={(e) => { e.preventDefault(); tab = 'urlaub'; }}>Urlaub</a>
    <a href={'#'} class:active={tab === 'gutstunden'} onclick={(e) => { e.preventDefault(); tab = 'gutstunden'; }}>Gutstunden</a>
  </div>
  {#if error}<div class="alert err">{error}</div>{/if}
  {#if msg}<div class="alert ok">{msg}</div>{/if}

  {#if tab === 'stamm'}
    <div class="grid cols-2">
      <form class="card" onsubmit={saveStamm}>
        <h3 style="margin-top:0">Stammdaten</h3>
        <div class="form-grid">
          <div class="field"><label for="pn">Personalnummer (BMD)</label><input id="pn" bind:value={emp.personalnr} required /></div>
          <div class="field"><label for="un">Benutzername</label><input id="un" bind:value={emp.username} required /></div>
          <div class="field"><label for="vn">Vorname</label><input id="vn" bind:value={emp.vorname} required /></div>
          <div class="field"><label for="nn">Nachname</label><input id="nn" bind:value={emp.nachname} required /></div>
          <div class="field"><label for="ro">Rolle</label><select id="ro" bind:value={emp.rolle}><option value="mitarbeiter">Mitarbeiter</option><option value="admin">Admin</option></select></div>
          <div class="field"><label for="ei">Eintritt</label><input id="ei" type="date" bind:value={emp.eintritt} required /></div>
          <div class="field"><label for="au">Austritt</label><input id="au" type="date" bind:value={emp.austritt} /></div>
          <div class="field"><label for="ds">Zeiterfassung ab / 1. Durchrechnungsperiode</label><input id="ds" type="date" bind:value={emp.durchrechnung_start} required /></div>
          <div class="field"><label for="dm">Durchrechnung (Monate)</label><input id="dm" type="number" min="1" max="12" bind:value={emp.durchrechnung_monate} /></div>
          <div class="field"><label for="ua">Urlaubsanspruch Tage/Jahr</label><input id="ua" type="number" step="0.5" bind:value={emp.urlaubsanspruch_tage} /></div>
          <div class="field"><label for="uj">Urlaubsjahr beginnt (MM-TT)</label><input id="uj" bind:value={emp.urlaubsjahr_beginn_mm_dd} /></div>
          <div class="field"><label for="gt">Gutstundentopf (leer = automatisch 307/311)</label><input id="gt" type="number" bind:value={emp.gutstunden_topf} /></div>
        </div>
        <div class="row">
          <button class="primary">Speichern</button>
          <button type="button" class:danger={emp.aktiv} onclick={toggleActive}>{emp.aktiv ? 'Deaktivieren' : 'Aktivieren'}</button>
        </div>
      </form>
      <div>
        <div class="card">
          <h3 style="margin-top:0">Zugang</h3>
          <div class="field"><label for="sec">Neues Passwort oder neue PIN</label><input id="sec" bind:value={secret} autocomplete="off" /></div>
          <div class="row">
            <button onclick={() => resetSecret('password')}>Passwort setzen</button>
            <button onclick={() => resetSecret('pin')}>PIN setzen</button>
          </div>
        </div>
        <div class="card">
          <h3 style="margin-top:0">Wochenmodelle</h3>
          <table>
            <thead><tr><th>Gültig ab</th><th>Mo / Di / Mi / Do / Fr / Sa / So</th><th>Optionen</th><th></th></tr></thead>
            <tbody>
              {#each schedules as s}
                <tr><td class="mono">{dateDe(s.gueltig_ab)}</td><td class="mono">{wm(s)}</td>
                  <td class="small">{#if s.pause_auto}<span class="badge">Pause auto</span>{/if} {#if s.gleitzeit}<span class="badge">Gleitzeit {s.gleitzeit_von}–{s.gleitzeit_bis}</span>{/if}</td>
                  <td><button class="small" onclick={() => delSchedule(s.id)}>×</button></td></tr>
              {/each}
            </tbody>
          </table>
          <form onsubmit={addSchedule} style="margin-top:1rem">
            <div class="row">
              <div style="width:150px"><label for="ga">Gültig ab</label><input id="ga" type="date" bind:value={ns.gueltig_ab} required /></div>
              {#each ['Mo','Di','Mi','Do','Fr','Sa','So'] as d, i}
                <div style="width:60px"><label for={'s'+i}>{d}</label><input id={'s'+i} type="number" step="0.25" min="0" max="24" bind:value={ns.stunden[i]} /></div>
              {/each}
            </div>
            <div class="row" style="margin-top:.6rem">
              <label style="margin:0"><input type="checkbox" bind:checked={ns.pause_auto} />Pause automatisch abziehen</label>
              <label style="margin:0"><input type="checkbox" bind:checked={ns.gleitzeit} />Gleitzeit</label>
            </div>
            {#if ns.gleitzeit}
              <div class="form-grid" style="margin-top:.6rem">
                <div class="field"><label for="gv">Gleitzeitrahmen von</label><input id="gv" type="time" bind:value={ns.gleitzeit_von} /></div>
                <div class="field"><label for="gb">bis</label><input id="gb" type="time" bind:value={ns.gleitzeit_bis} /></div>
                <div class="field"><label for="kv">Kernzeit von</label><input id="kv" type="time" bind:value={ns.kernzeit_von} /></div>
                <div class="field"><label for="kb">bis</label><input id="kb" type="time" bind:value={ns.kernzeit_bis} /></div>
                <div class="field"><label for="up">Übertrag max. Plus (Minuten)</label><input id="up" type="number" bind:value={ns.uebertrag_max_plus_min} /></div>
                <div class="field"><label for="um">Übertrag max. Minus (Minuten)</label><input id="um" type="number" bind:value={ns.uebertrag_max_minus_min} /></div>
              </div>
            {/if}
            <button class="primary" style="margin-top:.6rem">Wochenmodell ab Datum setzen</button>
          </form>
        </div>
      </div>
    </div>

  {:else if tab === 'zeiten'}
    <div class="row" style="justify-content:space-between;margin-bottom:1rem">
      <div class="row">
        <button onclick={() => { monat = shiftMonth(monat, -1); loadMonth(); }}>‹</button>
        <strong style="min-width:150px;text-align:center">{monthLabel(monat)}</strong>
        <button onclick={() => { monat = shiftMonth(monat, 1); loadMonth(); }}>›</button>
        {#if month?.geschlossen}<span class="badge ok">abgeschlossen</span>{/if}
      </div>
      <form class="row" onsubmit={insertPunch}>
        <input type="datetime-local" bind:value={np.zeit} required style="width:auto" />
        <select bind:value={np.art} style="width:auto"><option value="kommen">Kommen</option><option value="gehen">Gehen</option><option value="pause_start">Pause Beginn</option><option value="pause_ende">Pause Ende</option></select>
        <input bind:value={np.kommentar} placeholder="Begründung" style="width:180px" required />
        <button class="primary">Stempelung einfügen</button>
      </form>
    </div>
    {#if month}<MonthTable {month} admin onstorno={stornoPunch} />{/if}

  {:else if tab === 'abwesenheit'}
    <div class="grid cols-2">
      <div class="card"><h3 style="margin-top:0">Abwesenheit buchen</h3><AbsenceForm employeeId={id} onsaved={loadAbs} /></div>
      <div class="card table-wrap" style="padding:0">
        <table>
          <thead><tr><th>Art</th><th>Von</th><th>Bis</th><th>Status</th><th></th></tr></thead>
          <tbody>
            {#each absences as a}
              <tr><td>{a.label}{#if a.einheit !== 'tag'} <span class="muted small">({a.einheit === 'halber_tag' ? '½ Tag' : a.wert + ' h'})</span>{/if}</td>
                <td class="mono">{dateDe(a.von)}</td><td class="mono">{dateDe(a.bis)}</td>
                <td><span class="badge">{STATUS_LABEL[a.status]}</span></td>
                <td>{#if a.status === 'genehmigt' || a.status === 'beantragt'}<button class="small" onclick={() => stornoAbs(a.id)}>Storno</button>{/if}</td></tr>
            {:else}<tr><td colspan="5" class="muted">Keine Abwesenheiten.</td></tr>{/each}
          </tbody>
        </table>
      </div>
    </div>

  {:else if tab === 'urlaub' && vacation}
    <div class="grid cols-2">
      <div class="card">
        <h3 style="margin-top:0">Urlaubsjahr {dateDe(vacation.urlaubsjahr_von)} bis {dateDe(vacation.urlaubsjahr_bis)}</h3>
        <table><tbody>
          <tr><td>Anspruch{#if vacation.anspruch_aliquot} <span class="muted small">(aliquot berechnet)</span>{/if}</td><td class="right">{days(vacation.anspruch)}</td></tr>
          <tr><td>Übertrag aus Vorjahr</td><td class="right">{days(vacation.uebertrag)}</td></tr>
          <tr><td>Korrekturen / Verfall</td><td class="right">{days(vacation.korrektur)}</td></tr>
          <tr><td>Verbraucht (genehmigt, gesamtes Jahr)</td><td class="right">−{days(vacation.verbrauch)}</td></tr>
          <tr><th>Rest</th><th class="right">{days(vacation.rest)} Tage</th></tr>
        </tbody></table>
        <h3>Buchungen im Urlaubsjahr</h3>
        <table><tbody>
          {#each vacation.buchungen as b}<tr><td class="mono">{dateDe(b.von)} – {dateDe(b.bis)}</td><td>{b.label}</td><td class="right">{days(b.tage)}</td></tr>{/each}
          {#each vacation.eintraege as e}<tr><td class="muted">{e.art}</td><td class="small">{e.grund}</td><td class="right">{days(e.tage)}</td></tr>{/each}
        </tbody></table>
      </div>
      <form class="card" onsubmit={addVacEntry}>
        <h3 style="margin-top:0">Eintrag hinzufügen</h3>
        <p class="small muted">Anspruch übersteuert die automatische Berechnung, Übertrag den durchgerechneten Vorjahresrest. Korrektur und Verfall werden addiert (Verfall negativ eintragen).</p>
        <div class="form-grid">
          <div class="field"><label for="vj">Urlaubsjahr (Startdatum)</label><input id="vj" type="date" bind:value={ve.urlaubsjahr} required /></div>
          <div class="field"><label for="va">Art</label><select id="va" bind:value={ve.art}><option value="korrektur">Korrektur</option><option value="uebertrag">Übertrag</option><option value="anspruch">Anspruch</option><option value="verfall">Verfall</option></select></div>
          <div class="field"><label for="vt">Tage</label><input id="vt" type="number" step="0.5" bind:value={ve.tage} required /></div>
        </div>
        <div class="field"><label for="vg">Begründung</label><input id="vg" bind:value={ve.grund} required /></div>
        <button class="primary">Speichern</button>
      </form>
    </div>

  {:else if tab === 'gutstunden' && credit}
    <div class="grid cols-2">
      <div class="card">
        <h3 style="margin-top:0">Gleitzeitsaldo: <span class="mono" class:pos={credit.saldo_min > 0} class:neg={credit.saldo_min < 0}>{hm(credit.saldo_min, true)}</span></h3>
        <table><thead><tr><th>Topf</th><th class="right">Aufbau</th><th class="right">Abbau</th><th class="right">Saldo</th></tr></thead><tbody>
          {#each credit.toepfe as t}<tr><td>{t.topf}{#if t.topf === credit.standard_topf} <span class="badge">Standard</span>{/if}</td><td class="right mono">{hm(t.aufbau_min)}</td><td class="right mono">{hm(t.abbau_min)}</td><td class="right mono">{hm(t.saldo_min, true)}</td></tr>{/each}
        </tbody></table>
        <h3>Buchungen</h3>
        <table><tbody>
          {#each credit.eintraege as e}<tr><td class="mono">{dateDe(e.datum)}</td><td>{e.topf === 0 ? 'Saldo' : 'Topf ' + e.topf}</td><td>{e.art}</td><td class="small">{e.grund}</td><td class="right mono">{hm(e.minuten, true)}</td></tr>{:else}<tr><td class="muted">Keine Buchungen.</td></tr>{/each}
        </tbody></table>
      </div>
      <form class="card" onsubmit={addCreditEntry}>
        <h3 style="margin-top:0">Buchung hinzufügen</h3>
        <p class="small muted">Art „saldo“ mit Topf 0 setzt den Gleitzeitsaldo (z. B. Anfangswert bei Systemstart). Korrektur, Auszahlung (negativ) und Übertrag wirken auf den Gutstundentopf.</p>
        <div class="form-grid">
          <div class="field"><label for="cd">Datum</label><input id="cd" type="date" bind:value={ce.datum} required /></div>
          <div class="field"><label for="ca">Art</label><select id="ca" bind:value={ce.art} onchange={() => { if (ce.art === 'saldo') ce.topf = 0; else if (ce.topf === 0) ce.topf = credit.standard_topf; }}><option value="korrektur">Korrektur</option><option value="auszahlung">Auszahlung</option><option value="uebertrag">Übertrag</option><option value="saldo">Gleitzeitsaldo</option></select></div>
          <div class="field"><label for="ct">Topf</label><input id="ct" type="number" bind:value={ce.topf} required /></div>
          <div class="field"><label for="cm">Minuten (+/−)</label><input id="cm" type="number" bind:value={ce.minuten} required /></div>
        </div>
        <div class="field"><label for="cg">Begründung</label><input id="cg" bind:value={ce.grund} required /></div>
        <button class="primary">Speichern</button>
      </form>
    </div>
  {/if}
{/if}
