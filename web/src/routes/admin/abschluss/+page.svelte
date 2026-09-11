<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { thisMonth, shiftMonth, monthLabel, hm, dateDe } from '$lib/fmt';

  let monat = $state(shiftMonth(thisMonth(), -1));
  let data = $state<any>(null);
  let exports = $state<any[]>([]);
  let preview = $state<any>(null);
  let periods = $state<any[]>([]);
  let error = $state('');
  let msg = $state('');
  let busy = $state(false);

  async function load() {
    error = '';
    try {
      [data, exports, periods] = await Promise.all([
        api.get<any>(`/reports/month-status?monat=${monat}`),
        api.get<any[]>(`/export/runs?monat=${monat}`),
        api.get<any[]>(`/export/periods?monat=${monat}`)
      ]);
      preview = null;
    } catch (e) { error = errMsg(e); }
  }
  onMount(load);

  async function close(id: number) {
    busy = true; error = msg = '';
    try { await api.post(`/reports/close`, { employee_id: id, monat }); await load(); }
    catch (e) { error = errMsg(e); } finally { busy = false; }
  }
  async function reopen(id: number) {
    const grund = prompt('Begründung für das Aufheben des Abschlusses:');
    if (!grund) return;
    try { await api.post(`/reports/reopen`, { employee_id: id, monat, grund }); await load(); } catch (e) { error = errMsg(e); }
  }
  async function closeAll() {
    if (!confirm('Alle offenen Monate ohne Blocker abschließen und PDFs erzeugen?')) return;
    busy = true; error = msg = '';
    try { const r: any = await api.post(`/reports/close-all`, { monat }); msg = `${r.geschlossen} Monate abgeschlossen.`; await load(); }
    catch (e) { error = errMsg(e); } finally { busy = false; }
  }
  async function exportCsv() {
    busy = true; error = msg = '';
    try { const r: any = await api.post(`/export/bmd`, { monat }); msg = `Export erstellt: ${r.datei} (${r.zeilen} Zeilen)`; await load(); }
    catch (e) { error = errMsg(e); } finally { busy = false; }
  }
  async function loadPreview() {
    error = '';
    try { preview = await api.get(`/export/preview?monat=${monat}`); } catch (e) { error = errMsg(e); }
  }
  async function closePeriod(p: any) {
    const v = prompt(`Minuten in Topf ${p.topf} übertragen (Vorschlag ${p.vorschlag_min}):`, String(p.vorschlag_min));
    if (v === null) return;
    error = msg = '';
    try {
      await api.post('/export/period-close', { employee_id: p.employee_id, datum: p.periode_ende, minuten: Number(v), topf: p.topf });
      msg = 'Periode abgeschlossen.'; await load();
    } catch (e) { error = errMsg(e); }
  }
  const blockers = (r: any) => r.blocker as string[];
</script>

<div class="page-head">
  <h1>Monatsabschluss &amp; BMD-Export</h1>
  <div class="monthnav">
    <button onclick={() => { monat = shiftMonth(monat, -1); load(); }}>‹</button>
    <span>{monthLabel(monat)}</span>
    <button onclick={() => { monat = shiftMonth(monat, 1); load(); }}>›</button>
  </div>
</div>
{#if error}<div class="alert err">{error}</div>{/if}
{#if msg}<div class="alert ok">{msg}</div>{/if}

{#if data}
  <div class="row" style="margin-bottom:20px">
    <button class="primary" onclick={closeAll} disabled={busy}>Alle abschließen + PDFs</button>
    <a class="btn" href={`/api/reports/month/${monat}/zip`} target="_blank">PDFs als ZIP</a>
    <button onclick={loadPreview}>CSV-Vorschau</button>
    <button onclick={exportCsv} disabled={busy}>BMD-CSV erzeugen (Abrechnungsmonat {data.abrechnungsmonat})</button>
  </div>
  <div class="card tight table-wrap">
    <table>
      <thead><tr><th>Mitarbeiter</th><th class="right">Soll</th><th class="right">Ist</th><th class="right">Diff</th><th class="right">Saldo Ende</th><th>Prüfung</th><th>Status</th><th></th></tr></thead>
      <tbody>
        {#each data.rows as r}
          <tr>
            <td><a href={`/admin/mitarbeiter/${r.employee_id}`} style="font-weight:500">{r.name}</a></td>
            <td class="right mono">{hm(r.soll_min)}</td><td class="right mono">{hm(r.ist_min)}</td>
            <td class="right mono" class:pos={r.diff_min > 0} class:neg={r.diff_min < 0}>{hm(r.diff_min, true)}</td>
            <td class="right mono">{hm(r.saldo_ende_min, true)}</td>
            <td class="small">
              {#each blockers(r) as b}<span class="badge err">{b}</span> {/each}
              {#if r.warnungen}<span class="badge warn">{r.warnungen} Hinweise</span>{/if}
              {#if !blockers(r).length && !r.warnungen}<span class="badge ok">ok</span>{/if}
            </td>
            <td>{#if r.geschlossen}<span class="badge ok">abgeschlossen {dateDe(r.geschlossen.slice(0, 10))}</span>{:else}<span class="badge">offen</span>{/if}</td>
            <td class="right" style="white-space:nowrap">
              {#if r.geschlossen}
                <a class="btn small" href={`/api/reports/month/${r.employee_id}/${monat}/pdf`} target="_blank">PDF</a>
                <button class="small" onclick={() => reopen(r.employee_id)}>Aufheben</button>
              {:else}
                <a class="btn small" href={`/api/reports/month/${r.employee_id}/${monat}/pdf?vorschau=1`} target="_blank">Vorschau</a>
                <button class="primary small" style="font-weight:500" onclick={() => close(r.employee_id)} disabled={busy || blockers(r).length > 0}>Abschließen</button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if periods.length}
    <h2>Durchrechnungsperioden, die in diesem Monat enden</h2>
    <div class="card tight table-wrap">
      <table>
        <thead><tr><th>Mitarbeiter</th><th>Periodenende</th><th class="right">Saldo</th><th class="right">Übertragsgrenze</th><th class="right">Vorschlag → Topf</th><th>Hinweise</th><th></th></tr></thead>
        <tbody>
          {#each periods as p}
            <tr>
              <td>{p.name}</td><td class="mono">{dateDe(p.periode_ende)}</td>
              <td class="right mono" class:pos={p.saldo_min > 0} class:neg={p.saldo_min < 0}>{hm(p.saldo_min, true)}</td>
              <td class="right mono">{p.uebertrag_max_plus_min != null ? hm(p.uebertrag_max_plus_min) : '–'}</td>
              <td class="right mono">{hm(p.vorschlag_min)} → {p.topf}</td>
              <td class="small">{#each p.hinweise as h}<span class="badge warn">{h}</span> {/each}</td>
              <td class="right">{#if p.erledigt_min != null}<span class="badge ok">übertragen {hm(p.erledigt_min)}</span>{:else}<button class="primary small" style="font-weight:500" onclick={() => closePeriod(p)}>Periode abschließen</button>{/if}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="small muted">Beim Periodenabschluss wird der Plus-Saldo über der Übertragsgrenze in den Gutstundentopf übertragen und erscheint im BMD-Export als Aufbau. Ein Minus-Saldo bleibt im Gleitzeitsaldo.</p>
  {/if}

  {#if preview}
    <h2>CSV-Vorschau ({preview.art}, Verbuchungsart {preview.verbuchungsart})</h2>
    <div class="card tight table-wrap">
      <table>
        <thead><tr><th>MA</th><th>Mitarbeiter</th><th>NLZ_K</th><th>Von</th><th>Bis</th><th class="right">NLZ_VER</th><th class="right">MENGE</th><th>DivNLZ</th><th>Beschreibung</th></tr></thead>
        <tbody>
          {#each preview.rows as r}
            <tr><td class="mono">{r.ma}</td><td>{r.mitarbeiter}</td><td class="mono">{r.nlz_k}</td><td class="mono">{r.nlz_v}</td><td class="mono">{r.nlz_b}</td><td class="right mono">{r.nlz_ver}</td><td class="right mono">{r.menge}</td><td>{r.divnlz}</td><td class="small">{r.beschreibung}</td></tr>
          {:else}<tr><td colspan="9" class="muted">Keine Zeilen für diesen Monat.</td></tr>{/each}
        </tbody>
      </table>
    </div>
  {/if}

  <h2>Exporte für {monthLabel(monat)}</h2>
  <div class="card tight table-wrap">
    <table>
      <thead><tr><th>Erstellt</th><th>Art</th><th>Datei</th><th class="right">Zeilen</th><th>SHA-256</th><th></th></tr></thead>
      <tbody>
        {#each exports as x}
          <tr><td class="muted">{x.created_at.slice(0, 16).replace('T', ' ')}</td><td>{x.art}</td><td>{x.datei}</td><td class="right">{x.zeilen}</td><td class="small muted">{x.sha256.slice(0, 12)}…</td>
            <td class="right" style="white-space:nowrap"><a class="btn small" href={`/api/export/runs/${x.id}/download`}>Download</a> <a class="btn small" href={`/api/export/runs/${x.id}/preview`} target="_blank">Ansehen</a></td></tr>
        {:else}<tr><td colspan="6" class="muted">Noch kein Export.</td></tr>{/each}
      </tbody>
    </table>
  </div>
{/if}
