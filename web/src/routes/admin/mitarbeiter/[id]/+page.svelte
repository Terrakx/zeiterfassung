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
  let orig = $state<any>(null);
  let schedules = $state<any[]>([]);
  let summary = $state<{ saldo: number; rest: number } | null>(null);
  let error = $state('');
  let msg = $state('');

  let monat = $state(thisMonth());
  let month = $state<any>(null);
  let np = $state({ zeit: '', art: 'kommen', kommentar: '' });
  let absences = $state<any[]>([]);
  let vacation = $state<any>(null);
  let ve = $state({ urlaubsjahr: '', art: 'korrektur', tage: 0, grund: '' });
  let credit = $state<any>(null);
  let ce = $state({ datum: todayIso(), topf: 307, minuten: 0, art: 'korrektur', grund: '' });
  let secretDlg = $state<HTMLDialogElement>();
  let secretKind = $state<'password' | 'pin'>('password');
  let secret = $state('');
  let showSchedule = $state(false);
  let ns = $state({ gueltig_ab: todayIso(), stunden: [8, 8, 8, 8, 8, 0, 0], pause_auto: false, gleitzeit: false, gleitzeit_von: '', gleitzeit_bis: '', kernzeit_von: '', kernzeit_bis: '', uebertrag_max_plus_min: null as number | null, uebertrag_max_minus_min: null as number | null });

  async function load() {
    error = '';
    try {
      const r: any = await api.get(`/employees/${id}`);
      emp = r.employee; orig = structuredClone($state.snapshot(r.employee)); schedules = r.schedules;
      const [c, v]: any[] = await Promise.all([api.get(`/employees/${id}/credit`), api.get(`/employees/${id}/vacation`)]);
      summary = { saldo: c.saldo_min, rest: v.rest };
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
  function setTab(e: Event, t: typeof tab) { e.preventDefault(); tab = t; msg = ''; }

  async function saveStamm(e: Event) {
    e.preventDefault(); error = msg = '';
    try { await api.put(`/employees/${id}`, emp); msg = 'Gespeichert.'; await load(); } catch (err) { error = errMsg(err); }
  }
  function discard() { emp = structuredClone($state.snapshot(orig)); }
  async function toggleActive() {
    if (!confirm(emp.aktiv ? 'Mitarbeiter deaktivieren? Login und Stempeln werden gesperrt.' : 'Mitarbeiter wieder aktivieren?')) return;
    try { await api.del(`/employees/${id}`); await load(); } catch (err) { error = errMsg(err); }
  }
  function openSecret(kind: 'password' | 'pin') { secretKind = kind; secret = ''; secretDlg?.showModal(); }
  async function saveSecret(e: Event) {
    e.preventDefault(); error = msg = '';
    try { await api.post(`/employees/${id}/${secretKind}`, { wert: secret }); msg = secretKind === 'pin' ? 'PIN gesetzt.' : 'Passwort gesetzt.'; secretDlg?.close(); await load(); } catch (err) { error = errMsg(err); }
  }
  async function addSchedule(e: Event) {
    e.preventDefault(); error = '';
    try { schedules = await api.post(`/employees/${id}/schedules`, { ...ns, stunden: ns.stunden.map(Number) }); msg = 'Wochenmodell gespeichert.'; showSchedule = false; } catch (err) { error = errMsg(err); }
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
    try { await api.post(`/employees/${id}/vacation`, ve); ve.grund = ''; ve.tage = 0; await loadVac(); await load(); } catch (err) { error = errMsg(err); }
  }
  async function addCreditEntry(e: Event) {
    e.preventDefault(); error = '';
    try { await api.post(`/employees/${id}/credit`, ce); ce.grund = ''; ce.minuten = 0; await loadCredit(); await load(); } catch (err) { error = errMsg(err); }
  }
  const mins = (s: any) => [s.mo_min, s.di_min, s.mi_min, s.do_min, s.fr_min, s.sa_min, s.so_min] as number[];
  const weekly = (s: any) => mins(s).reduce((a, b) => a + b, 0) / 60;
  const current = $derived(schedules.length ? schedules[schedules.length - 1] : null);
  const modelLabel = $derived(current ? (weekly(current) >= 38.5 ? `Vollzeit ${days(weekly(current))} h` : `Teilzeit ${days(weekly(current))} h`) : '');
  const chips = (s: any) => {
    const c: string[] = [];
    if (s.pause_auto) c.push('Pausenabzug automatisch');
    if (s.gleitzeit) c.push(`Gleitzeit${s.gleitzeit_von ? ' ' + s.gleitzeit_von + '–' + s.gleitzeit_bis : ''}`);
    if (s.kernzeit_von) c.push(`Kernzeit ${s.kernzeit_von}–${s.kernzeit_bis}`);
    if (s.uebertrag_max_plus_min != null || s.uebertrag_max_minus_min != null) c.push(`Übertrag max. +${hm(s.uebertrag_max_plus_min ?? 0)} / −${hm(s.uebertrag_max_minus_min ?? 0)}`);
    return c;
  };
  const tabs = [['stamm', 'Stammdaten'], ['zeiten', 'Zeiten'], ['abwesenheit', 'Abwesenheiten'], ['urlaub', 'Urlaub'], ['gutstunden', 'Gutstunden']] as const;
</script>

{#if emp}
  <div class="breadcrumb"><a href="/admin/mitarbeiter">Mitarbeiter</a><span>›</span><span>{emp.vorname} {emp.nachname}</span></div>
  <div class="page-head" style="align-items:flex-end">
    <div class="row" style="gap:14px">
      <h1>{emp.vorname} {emp.nachname}</h1>
      <span class="small muted">Nr. {emp.personalnr}{#if modelLabel}{" · " + modelLabel}{/if}</span>
      {#if emp.aktiv}<span class="badge ok">aktiv</span>{:else}<span class="badge">inaktiv</span>{/if}
    </div>
    {#if summary}
      <div class="row small muted" style="gap:16px">
        <span>Saldo <strong class:pos={summary.saldo > 0} class:neg={summary.saldo < 0}>{hm(summary.saldo, true)}</strong></span>
        <span>Resturlaub <strong style="color:var(--ink)">{days(summary.rest)} Tage</strong></span>
      </div>
    {/if}
  </div>
  <div class="tabs">
    {#each tabs as [key, label]}<a href={'#' + key} class:active={tab === key} onclick={(e) => setTab(e, key)}>{label}</a>{/each}
  </div>
  {#if error}<div class="alert err">{error}</div>{/if}
  {#if msg}<div class="alert ok">{msg}</div>{/if}

  {#if tab === 'stamm'}
    <div style="display:grid;grid-template-columns:minmax(0,1.3fr) minmax(0,1fr);gap:20px;align-items:start" class="detail-grid">
      <form class="card" style="margin:0" onsubmit={saveStamm}>
        <div class="card-title">Stammdaten</div>
        <div class="form-grid" style="grid-template-columns:1fr 1fr">
          <div class="field"><label for="pn">Personalnummer</label><input id="pn" bind:value={emp.personalnr} required /><span class="help">BMD-Mitarbeiternummer</span></div>
          <div class="field"><label for="un">Benutzername</label><input id="un" bind:value={emp.username} required /></div>
          <div class="field"><label for="vn">Vorname</label><input id="vn" bind:value={emp.vorname} required /></div>
          <div class="field"><label for="nn">Nachname</label><input id="nn" bind:value={emp.nachname} required /></div>
          <div class="field"><label for="ro">Rolle</label><select id="ro" bind:value={emp.rolle}><option value="mitarbeiter">Mitarbeiter</option><option value="admin">Admin</option></select></div>
          <div class="field"><label for="ds">Zeiterfassung ab</label><input id="ds" type="date" bind:value={emp.durchrechnung_start} required /><span class="help">Beginn der Saldoberechnung und der ersten Durchrechnungsperiode</span></div>
          <div class="field"><label for="ei">Eintritt</label><input id="ei" type="date" bind:value={emp.eintritt} required /></div>
          <div class="field"><label for="au">Austritt</label><input id="au" type="date" bind:value={emp.austritt} /><span class="help">Leer lassen, wenn unbefristet</span></div>
          <div class="field"><label for="dm">Durchrechnung</label><input id="dm" type="number" min="1" max="12" bind:value={emp.durchrechnung_monate} /><span class="help">Monate je Periode</span></div>
          <div class="field"><label for="ua">Urlaubsanspruch</label><input id="ua" type="number" step="0.5" bind:value={emp.urlaubsanspruch_tage} /><span class="help">Tage pro Jahr</span></div>
          <div class="field"><label for="uj">Beginn Urlaubsjahr</label><input id="uj" bind:value={emp.urlaubsjahr_beginn_mm_dd} placeholder="MM-TT" /></div>
          <div class="field"><label for="gt">Gutstundentopf</label><input id="gt" type="number" bind:value={emp.gutstunden_topf} placeholder="automatisch" /><span class="help">Leer = automatisch (Vollzeit 307, Teilzeit 311)</span></div>
        </div>
        <div class="form-footer">
          <button type="button" class="link danger" onclick={toggleActive}>{emp.aktiv ? 'Deaktivieren' : 'Aktivieren'}</button>
          <div class="row"><button type="button" onclick={discard}>Verwerfen</button><button class="primary">Speichern</button></div>
        </div>
      </form>
      <div>
        <div class="card">
          <div class="card-title">Zugang</div>
          <div class="rowlist">
            <div><div><div>Passwort</div><div class="xs muted">Benutzer {emp.username}</div></div><button class="small" onclick={() => openSecret('password')}>Zurücksetzen</button></div>
            <div><div><div>Terminal-PIN</div><div class="xs" style="color:{emp.hat_pin === false ? 'var(--warn)' : 'var(--ok)'}">{emp.hat_pin === false ? 'nicht gesetzt' : 'gesetzt'}</div></div><button class="small" onclick={() => openSecret('pin')}>Neu setzen</button></div>
          </div>
        </div>
        <div class="card">
          <div class="row between" style="margin-bottom:14px"><div class="card-title" style="margin:0">Wochenmodelle</div><button class="link" style="font-size:12px" onclick={() => (showSchedule = !showSchedule)}>+ Neues Modell</button></div>
          <table class="inline-table">
            <thead><tr><th>Gültig ab</th>{#each ['Mo','Di','Mi','Do','Fr','Sa','So'] as d}<th class="right">{d}</th>{/each}<th class="right">Σ</th><th></th></tr></thead>
            <tbody>
              {#each [...schedules].reverse() as s, i}
                <tr class:muted-row={i > 0}>
                  <td>{dateDe(s.gueltig_ab)}</td>
                  {#each mins(s) as m}<td class="right">{m ? hm(m) : '–'}</td>{/each}
                  <td class="right" style="font-weight:600">{days(weekly(s))}</td>
                  <td class="right">{#if schedules.length > 1}<button class="link small" style="font-size:12px" onclick={() => delSchedule(s.id)}>×</button>{/if}</td>
                </tr>
              {/each}
            </tbody>
          </table>
          {#if current && chips(current).length}
            <div class="row" style="gap:6px;padding-top:10px;border-top:1px solid var(--border-row);margin-top:8px">{#each chips(current) as c}<span class="chip">{c}</span>{/each}</div>
          {/if}
          {#if showSchedule}
            <form onsubmit={addSchedule} style="margin-top:16px;padding-top:16px;border-top:1px solid var(--border-row)">
              <div class="field"><label for="ga">Gültig ab</label><input id="ga" type="date" bind:value={ns.gueltig_ab} required style="max-width:180px" /></div>
              <div class="field">
                <span class="xs muted" style="display:block;margin-bottom:6px;font-weight:500;color:var(--ink-2);font-size:13px">Stunden je Tag</span>
                <div style="display:grid;grid-template-columns:repeat(7,1fr);gap:6px">
                  {#each ['Mo','Di','Mi','Do','Fr','Sa','So'] as d, i}
                    <div><span class="xs muted" style="display:block;text-align:center;margin-bottom:4px">{d}</span><input type="number" step="0.25" min="0" max="24" bind:value={ns.stunden[i]} style="padding:0 6px;text-align:center" aria-label={d} /></div>
                  {/each}
                </div>
              </div>
              <label class="check"><input type="checkbox" bind:checked={ns.pause_auto} />Pause automatisch abziehen (nur bei betrieblich festgelegter Pausenlage)</label>
              <label class="check"><input type="checkbox" bind:checked={ns.gleitzeit} />Gleitzeit</label>
              {#if ns.gleitzeit}
                <div class="form-grid" style="margin-top:10px">
                  <div class="field"><label for="gv">Gleitzeitrahmen von</label><input id="gv" type="time" bind:value={ns.gleitzeit_von} /></div>
                  <div class="field"><label for="gb">bis</label><input id="gb" type="time" bind:value={ns.gleitzeit_bis} /></div>
                  <div class="field"><label for="kv">Kernzeit von</label><input id="kv" type="time" bind:value={ns.kernzeit_von} /></div>
                  <div class="field"><label for="kb">bis</label><input id="kb" type="time" bind:value={ns.kernzeit_bis} /></div>
                  <div class="field"><label for="up">Übertrag max. Plus (Minuten)</label><input id="up" type="number" bind:value={ns.uebertrag_max_plus_min} /></div>
                  <div class="field"><label for="um">Übertrag max. Minus (Minuten)</label><input id="um" type="number" bind:value={ns.uebertrag_max_minus_min} /></div>
                </div>
              {/if}
              <div class="row" style="justify-content:flex-end;margin-top:12px"><button type="button" onclick={() => (showSchedule = false)}>Abbrechen</button><button class="primary">Modell speichern</button></div>
            </form>
          {/if}
        </div>
      </div>
    </div>

  {:else if tab === 'zeiten'}
    <div class="page-head">
      <div class="row">
        <div class="monthnav"><button onclick={() => { monat = shiftMonth(monat, -1); loadMonth(); }}>‹</button><span>{monthLabel(monat)}</span><button onclick={() => { monat = shiftMonth(monat, 1); loadMonth(); }}>›</button></div>
        {#if month?.geschlossen}<span class="badge ok">abgeschlossen</span>{:else if month}<span class="badge">offen</span>{/if}
      </div>
      <form class="row" onsubmit={insertPunch}>
        <input type="datetime-local" bind:value={np.zeit} required style="width:auto" />
        <select bind:value={np.art} style="width:auto"><option value="kommen">Kommen</option><option value="gehen">Gehen</option><option value="pause_start">Pause Beginn</option><option value="pause_ende">Pause Ende</option></select>
        <input bind:value={np.kommentar} placeholder="Begründung" style="width:200px" required />
        <button class="primary">Stempelung einfügen</button>
      </form>
    </div>
    {#if month}<MonthTable {month} admin onstorno={stornoPunch} />{/if}

  {:else if tab === 'abwesenheit'}
    <div class="grid cols-2">
      <div class="card" style="margin:0"><div class="card-title">Abwesenheit buchen</div><AbsenceForm employeeId={id} onsaved={(m) => { msg = m; loadAbs(); }} /></div>
      <div class="card tight table-wrap" style="margin:0">
        <table>
          <thead><tr><th>Art</th><th>Von</th><th>Bis</th><th>Status</th><th></th></tr></thead>
          <tbody>
            {#each absences as a}
              <tr><td>{a.label}{#if a.einheit !== 'tag'} <span class="muted small">({a.einheit === 'halber_tag' ? '½ Tag' : a.wert + ' h'})</span>{/if}</td>
                <td>{dateDe(a.von)}</td><td>{dateDe(a.bis)}</td>
                <td><span class="badge" class:ok={a.status === 'genehmigt'} class:warn={a.status === 'beantragt'} class:err={a.status === 'abgelehnt'}>{STATUS_LABEL[a.status]}</span></td>
                <td class="right">{#if a.status === 'genehmigt' || a.status === 'beantragt'}<button class="small" onclick={() => stornoAbs(a.id)}>Storno</button>{/if}</td></tr>
            {:else}<tr><td colspan="5" class="muted">Keine Abwesenheiten.</td></tr>{/each}
          </tbody>
        </table>
      </div>
    </div>

  {:else if tab === 'urlaub' && vacation}
    <div class="grid cols-2">
      <div class="card" style="margin:0">
        <div class="card-title">Urlaubsjahr {dateDe(vacation.urlaubsjahr_von)} bis {dateDe(vacation.urlaubsjahr_bis)}</div>
        <table class="inline-table"><tbody>
          <tr><td>Anspruch{#if vacation.anspruch_aliquot} <span class="muted small">(aliquot)</span>{/if}</td><td class="right">{days(vacation.anspruch)}</td></tr>
          <tr><td>Übertrag aus Vorjahren</td><td class="right">{days(vacation.uebertrag)}</td></tr>
          <tr><td>Korrekturen</td><td class="right">{days(vacation.korrektur)}</td></tr>
          {#if vacation.verfall_manuell || vacation.verfall_auto}<tr><td>Verfall (manuell {days(vacation.verfall_manuell)}, automatisch {days(vacation.verfall_auto)})</td><td class="right neg">{days(vacation.verfall_manuell + vacation.verfall_auto)}</td></tr>{/if}
          <tr><td>Verbraucht (genehmigt, gesamtes Jahr)</td><td class="right">−{days(vacation.verbrauch)}</td></tr>
          <tr><td style="font-weight:600">Rest</td><td class="right" style="font-weight:600">{days(vacation.rest)} Tage</td></tr>
        </tbody></table>
        <p class="small muted" style="margin:12px 0 0">Offene Ansprüche: {#each vacation.offene_ansprueche as b, i}{i ? ', ' : ''}{days(b.tage)} aus {b.aus_urlaubsjahr.slice(0, 4)}{/each}</p>
        {#if vacation.naechster_verfall}<p class="small" style="color:var(--warn);margin:4px 0 0">{days(vacation.naechster_verfall.tage)} Tage verfallen am {dateDe(vacation.naechster_verfall.am)}</p>{/if}
        <div class="row" style="margin-top:14px"><a class="btn" href={`/api/reports/vacation/${id}/pdf`} target="_blank">Urlaubskartei PDF</a></div>
        <h2>Buchungen im Urlaubsjahr</h2>
        <table class="inline-table"><tbody>
          {#each vacation.buchungen as b}<tr><td>{dateDe(b.von)} – {dateDe(b.bis)}</td><td>{b.label}</td><td class="right">{days(b.tage)}</td></tr>{/each}
          {#each vacation.eintraege as e}<tr><td class="muted">{e.art}</td><td class="small">{e.grund}</td><td class="right">{days(e.tage)}</td></tr>{/each}
          {#if !vacation.buchungen.length && !vacation.eintraege.length}<tr><td class="muted" colspan="3">Keine Buchungen.</td></tr>{/if}
        </tbody></table>
      </div>
      <form class="card" style="margin:0" onsubmit={addVacEntry}>
        <div class="card-title">Eintrag hinzufügen</div>
        <p class="small muted">Anspruch übersteuert die automatische Berechnung, Übertrag den durchgerechneten Vorjahresrest. Korrektur und Verfall werden addiert (Verfall negativ eintragen).</p>
        <div class="form-grid">
          <div class="field"><label for="vj">Urlaubsjahr (Startdatum)</label><input id="vj" type="date" bind:value={ve.urlaubsjahr} required /></div>
          <div class="field"><label for="va">Art</label><select id="va" bind:value={ve.art}><option value="korrektur">Korrektur</option><option value="uebertrag">Übertrag</option><option value="anspruch">Anspruch</option><option value="verfall">Verfall</option></select></div>
          <div class="field"><label for="vt">Tage</label><input id="vt" type="number" step="0.5" bind:value={ve.tage} required /></div>
        </div>
        <div class="field" style="margin-top:16px"><label for="vg">Begründung</label><input id="vg" bind:value={ve.grund} required /></div>
        <div class="row" style="justify-content:flex-end"><button class="primary">Speichern</button></div>
      </form>
    </div>

  {:else if tab === 'gutstunden' && credit}
    <div class="grid cols-2">
      <div class="card" style="margin:0">
        <div class="card-title">Gleitzeitsaldo <span class:pos={credit.saldo_min > 0} class:neg={credit.saldo_min < 0}>{hm(credit.saldo_min, true)}</span></div>
        <table class="inline-table"><thead><tr><th>Topf</th><th class="right">Aufbau</th><th class="right">Abbau</th><th class="right">Saldo</th></tr></thead><tbody>
          {#each credit.toepfe as t}<tr><td>{t.topf}{#if t.topf === credit.standard_topf} <span class="badge">Standard</span>{/if}</td><td class="right">{hm(t.aufbau_min)}</td><td class="right">{hm(t.abbau_min)}</td><td class="right" style="font-weight:600">{hm(t.saldo_min, true)}</td></tr>{/each}
        </tbody></table>
        <h2>Buchungen</h2>
        <table class="inline-table"><tbody>
          {#each credit.eintraege as e}<tr><td>{dateDe(e.datum)}</td><td>{e.topf === 0 ? 'Saldo' : 'Topf ' + e.topf}</td><td>{e.art}</td><td class="small muted">{e.grund}</td><td class="right">{hm(e.minuten, true)}</td></tr>{:else}<tr><td class="muted">Keine Buchungen.</td></tr>{/each}
        </tbody></table>
      </div>
      <form class="card" style="margin:0" onsubmit={addCreditEntry}>
        <div class="card-title">Buchung hinzufügen</div>
        <p class="small muted">Art „Gleitzeitsaldo“ mit Topf 0 setzt den Saldo (z. B. Anfangswert bei Systemstart). Korrektur, Auszahlung (negativ) und Übertrag wirken auf den Gutstundentopf.</p>
        <div class="form-grid">
          <div class="field"><label for="cd">Datum</label><input id="cd" type="date" bind:value={ce.datum} required /></div>
          <div class="field"><label for="ca">Art</label><select id="ca" bind:value={ce.art} onchange={() => { if (ce.art === 'saldo') ce.topf = 0; else if (ce.topf === 0) ce.topf = credit.standard_topf; }}><option value="korrektur">Korrektur</option><option value="auszahlung">Auszahlung</option><option value="uebertrag">Übertrag</option><option value="saldo">Gleitzeitsaldo</option></select></div>
          <div class="field"><label for="ct">Topf</label><input id="ct" type="number" bind:value={ce.topf} required /></div>
          <div class="field"><label for="cm">Minuten (+/−)</label><input id="cm" type="number" bind:value={ce.minuten} required /></div>
        </div>
        <div class="field" style="margin-top:16px"><label for="cg">Begründung</label><input id="cg" bind:value={ce.grund} required /></div>
        <div class="row" style="justify-content:flex-end"><button class="primary">Speichern</button></div>
      </form>
    </div>
  {/if}

  <dialog bind:this={secretDlg}>
    <h2>{secretKind === 'pin' ? 'Terminal-PIN neu setzen' : 'Passwort zurücksetzen'}</h2>
    <form onsubmit={saveSecret}>
      <div class="field"><label for="sec">{secretKind === 'pin' ? 'Neue PIN (4 bis 8 Ziffern)' : 'Neues Passwort (mind. 8 Zeichen)'}</label><input id="sec" bind:value={secret} autocomplete="off" required inputmode={secretKind === 'pin' ? 'numeric' : undefined} /></div>
      <div class="row" style="justify-content:flex-end"><button type="button" onclick={() => secretDlg?.close()}>Abbrechen</button><button class="primary">Speichern</button></div>
    </form>
  </dialog>
{/if}

<style>
  @media (max-width: 900px) { .detail-grid { grid-template-columns: 1fr !important; } }
</style>
