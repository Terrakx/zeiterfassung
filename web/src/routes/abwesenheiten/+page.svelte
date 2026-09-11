<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { dateDe, STATUS_LABEL, days } from '$lib/fmt';
  import AbsenceForm from '$lib/AbsenceForm.svelte';

  let list = $state<any[]>([]);
  let account = $state<any>(null);
  let error = $state('');

  async function load() {
    const y = new Date().getFullYear();
    try {
      [list, account] = await Promise.all([
        api.get(`/absences?von=${y - 1}-01-01&bis=${y + 1}-12-31`),
        api.get('/absences/account')
      ]);
    } catch (e) {
      error = errMsg(e);
    }
  }
  onMount(load);

  async function withdraw(id: number) {
    if (!confirm('Antrag zurückziehen?')) return;
    try {
      await api.post(`/absences/${id}/withdraw`);
      await load();
    } catch (e) {
      error = errMsg(e);
    }
  }
  const badge = (s: string) => (s === 'genehmigt' ? 'ok' : s === 'beantragt' ? 'warn' : s === 'abgelehnt' ? 'err' : '');
</script>

<h1>Abwesenheiten</h1>
{#if error}<div class="alert err">{error}</div>{/if}

<div class="grid cols-2">
  <div class="card">
    <h3 style="margin-top:0">Antrag stellen</h3>
    <AbsenceForm onsaved={load} />
  </div>
  {#if account}
    <div class="card">
      <h3 style="margin-top:0">Urlaubskonto {dateDe(account.urlaub.urlaubsjahr_von)} bis {dateDe(account.urlaub.urlaubsjahr_bis)}</h3>
      <table>
        <tbody>
          <tr><td>Anspruch{#if account.urlaub.anspruch_aliquot} <span class="muted small">(aliquot)</span>{/if}</td><td class="right">{days(account.urlaub.anspruch)}</td></tr>
          <tr><td>Übertrag Vorjahr</td><td class="right">{days(account.urlaub.uebertrag)}</td></tr>
          {#if account.urlaub.korrektur}<tr><td>Korrekturen</td><td class="right">{days(account.urlaub.korrektur)}</td></tr>{/if}
          <tr><td>Verbraucht bis heute</td><td class="right">−{days(account.urlaub.verbrauch_bis_stichtag)}</td></tr>
          <tr><td>Geplant (genehmigt)</td><td class="right">−{days(account.urlaub.geplant)}</td></tr>
          <tr><th>Rest</th><th class="right">{days(account.urlaub.rest)} Tage</th></tr>
        </tbody>
      </table>
      <h3>Gutstunden</h3>
      <table>
        <tbody>
          {#each account.gutstunden.toepfe as t}
            <tr><td>Topf {t.topf}</td><td class="right mono">{(t.saldo_min / 60).toFixed(2).replace('.', ',')} h</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<div class="card table-wrap" style="padding:0">
  <table>
    <thead><tr><th>Art</th><th>Von</th><th>Bis</th><th>Einheit</th><th>Status</th><th>Kommentar</th><th></th></tr></thead>
    <tbody>
      {#each list as a}
        <tr>
          <td>{a.label}</td><td class="mono">{dateDe(a.von)}</td><td class="mono">{dateDe(a.bis)}</td>
          <td>{a.einheit === 'tag' ? 'Tage' : a.einheit === 'halber_tag' ? 'halber Tag' : `${a.wert} h`}</td>
          <td><span class="badge {badge(a.status)}">{STATUS_LABEL[a.status]}</span></td>
          <td class="small">{a.kommentar ?? ''}{#if a.entscheidung_kommentar} <span class="muted">— {a.entscheidung_kommentar}</span>{/if}</td>
          <td>{#if a.status === 'beantragt'}<button class="small" onclick={() => withdraw(a.id)}>Zurückziehen</button>{/if}</td>
        </tr>
      {:else}
        <tr><td colspan="7" class="muted">Keine Abwesenheiten.</td></tr>
      {/each}
    </tbody>
  </table>
</div>
