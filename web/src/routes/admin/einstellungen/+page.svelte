<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { loadBranding } from '$lib/stores';

  let s = $state<any>(null);
  let error = $state('');
  let msg = $state('');

  onMount(async () => { try { s = await api.get('/settings'); } catch (e) { error = errMsg(e); } });

  async function save(e: Event) {
    e.preventDefault();
    error = msg = '';
    try {
      s = await api.put('/settings', s);
      await loadBranding();
      msg = 'Gespeichert.';
    } catch (err) { error = errMsg(err); }
  }
  function logo(e: Event) {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    if (file.size > 300 * 1024) { error = 'Logo maximal 300 KB'; return; }
    const r = new FileReader();
    r.onload = () => { s.logo_data_url = r.result as string; };
    r.readAsDataURL(file);
  }
</script>

<div class="page-head"><h1>Einstellungen</h1></div>
{#if error}<div class="alert err">{error}</div>{/if}
{#if msg}<div class="alert ok">{msg}</div>{/if}
{#if s}
  <form onsubmit={save}>
    <div class="card">
      <div class="card-title">Branding</div>
      <div class="form-grid">
        <div class="field"><label for="fn">Firmenname</label><input id="fn" bind:value={s.firmenname} /></div>
        <div class="field"><label for="pc">Primärfarbe</label><input id="pc" type="color" bind:value={s.primaerfarbe} /></div>
        <div class="field"><label for="fz">Fußzeile (UI und PDF)</label><input id="fz" bind:value={s.fusszeile} /></div>
        <div class="field"><label for="td">Terminal-Darstellung</label><select id="td" bind:value={s.terminal_dunkel}><option value={true}>Dunkel</option><option value={false}>Hell</option></select></div>
        <div class="field"><label for="lg">Logo (PNG oder JPG, max. 300 KB)</label><input id="lg" type="file" accept="image/*" onchange={logo} />
          {#if s.logo_data_url}<div class="row" style="margin-top:.4rem"><img src={s.logo_data_url} alt="" style="max-height:40px" /><button type="button" onclick={() => (s.logo_data_url = null)}>Entfernen</button></div>{/if}</div>
      </div>
    </div>
    <div class="card">
      <div class="card-title">BMD-Schnittstelle</div>
      <div class="form-grid">
        <div class="field"><label for="bf">BMD-Firmennummer</label><input id="bf" bind:value={s.bmd_firmennr} /></div>
        <div class="field"><label for="bz">Zeichensatz der CSV</label><select id="bz" bind:value={s.bmd_zeichensatz}><option value="windows-1252">Windows-1252</option><option value="utf-8">UTF-8</option></select></div>
        <div class="field"><label for="tv">Gutstundentopf Vollzeit (NLZ)</label><input id="tv" type="number" bind:value={s.topf_vollzeit} /></div>
        <div class="field"><label for="tt">Gutstundentopf Teilzeit (NLZ)</label><input id="tt" type="number" bind:value={s.topf_teilzeit} /></div>
        <div class="field"><label for="tf">Gutstundentopf Feiertag (NLZ)</label><input id="tf" type="number" bind:value={s.topf_feiertag} /></div>
        <div class="field"><label for="ab">Diverse-NLZ-Nummer für Absonderung</label><input id="ab" bind:value={s.bmd_absonderung_divnlz} /></div>
      </div>
      <label class="check"><input type="checkbox" bind:checked={s.bmd_krank_exportieren} />Krankenstände zum Abgleich mit exportieren (302/303/314). Standard: aus, da über ÖGK-Import.</label>
    </div>
    <div class="card">
      <div class="card-title">Arbeitszeit</div>
      <div class="form-grid">
        <div class="field"><label for="kv">KV-Normalarbeitszeit (Wochenstunden)</label><input id="kv" type="number" step="0.5" bind:value={s.kv_wochenstunden} /></div>
        <div class="field"><label for="ps">Pause erforderlich ab (Minuten Arbeit)</label><input id="ps" type="number" bind:value={s.pause_schwelle_min} /></div>
        <div class="field"><label for="pd">Mindestpause (Minuten)</label><input id="pd" type="number" bind:value={s.pause_dauer_min} /></div>
      </div>
      <p class="small muted">§ 11 AZG: über 6 Stunden Arbeit mindestens 30 Minuten Pause. Der automatische Pausenabzug wird je Mitarbeiter im Wochenmodell aktiviert und ist nur zulässig, wenn die Pausenlage betrieblich festgelegt ist.</p>
    </div>
    <div class="card">
      <div class="card-title">Urlaub</div>
      <label class="check"><input type="checkbox" bind:checked={s.urlaub_halbe_tage} />Halbe Urlaubstage erlauben</label>
      <label class="check"><input type="checkbox" bind:checked={s.urlaub_stunden} />Stundenweisen Urlaub erlauben</label>
      <label class="check"><input type="checkbox" bind:checked={s.urlaub_verfall_auto} />Verfall automatisch berechnen (§ 4 Abs 5 UrlG: zwei Jahre nach Ende des Urlaubsjahres, Verbrauch vom ältesten Anspruch)</label>
      <div class="field" style="margin-top:.6rem"><label for="uh">Hinweistext bei halben Tagen / Stunden</label><textarea id="uh" rows="3" bind:value={s.urlaub_hinweis}></textarea></div>
    </div>
    <div class="card">
      <div class="card-title">PDF-Bericht</div>
      <div class="form-grid">
        <div class="field"><label for="u1">Unterschriftsfeld 1</label><input id="u1" bind:value={s.unterschrift_1} /></div>
        <div class="field"><label for="u2">Unterschriftsfeld 2</label><input id="u2" bind:value={s.unterschrift_2} /></div>
      </div>
    </div>
    <div class="row" style="justify-content:flex-end"><button class="primary">Speichern</button></div>
  </form>
  <div class="card">
    <div class="card-title">System</div>
    <p class="small muted">Erzeugt eine konsistente Kopie der Datenbank. PDFs und CSV-Exporte liegen zusätzlich im Datenverzeichnis unter <code>exports/</code>; für ein vollständiges Backup das Skript <code>deploy/backup.sh</code> verwenden.</p>
    <a class="btn" href="/api/admin/backup">Datenbank-Backup herunterladen</a>
  </div>
{/if}
