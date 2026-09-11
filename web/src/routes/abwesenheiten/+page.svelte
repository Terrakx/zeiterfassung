<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { dateDe, STATUS_LABEL, days } from '$lib/fmt';
  import AbsenceForm from '$lib/AbsenceForm.svelte';
  import { user } from '$lib/stores';

  let list = $state<any[]>([]);
  let account = $state<any>(null);
  let error = $state('');

  async function load() {
    const y = new Date().getFullYear();
    try {
      [list, account] = await Promise.all([
        api.get<any[]>(`/absences?von=${y - 1}-01-01&bis=${y + 1}-12-31`),
        api.get<any>('/absences/account')
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

<div class="page-head"><h1>Abwesenheiten</h1></div>
{#if error}<div class="alert err">{error}</div>{/if}

<div class="grid cols-2">
  <div class="card" style="margin:0">
    <div class="card-title">Antrag stellen</div>
    <AbsenceForm onsaved={load} />
  </div>
  {#if account}
    <div class="card" style="margin:0">
      <div class="card-title">Urlaubskonto {dateDe(account.urlaub.urlaubsjahr_von)} bis {dateDe(account.urlaub.urlaubsjahr_bis)}</div>
      <table class="inline-table">
        <tbody>
          <tr><td>Anspruch{#if account.urlaub.anspruch_aliquot} <span class="muted small">(aliquot)</span>{/if}</td><td class="right">{days(account.urlaub.anspruch)}</td></tr>
          <tr><td>Übertrag Vorjahr</td><td class="right">{days(account.urlaub.uebertrag)}</td></tr>
          {#if account.urlaub.korrektur}<tr><td>Korrekturen</td><td class="right">{days(account.urlaub.korrektur)}</td></tr>{/if}
          <tr><td>Verbraucht bis heute</td><td class="right">−{days(account.urlaub.verbrauch_bis_stichtag)}</td></tr>
          <tr><td>Geplant (genehmigt)</td><td class="right">−{days(account.urlaub.geplant)}</td></tr>
          <tr><td style="font-weight:600">Rest</td><td class="right" style="font-weight:600">{days(account.urlaub.rest)} Tage</td></tr>
        </tbody>
      </table>
      {#if account.urlaub.naechster_verfall}
        <p class="small" style="color:var(--warn)">{days(account.urlaub.naechster_verfall.tage)} Tage aus {account.urlaub.naechster_verfall.aus_urlaubsjahr.slice(0, 4)} verfallen am {dateDe(account.urlaub.naechster_verfall.am)}.</p>
      {/if}
      <p class="small"><a href={`/api/reports/vacation/${$user?.id}/pdf`} target="_blank">Urlaubskartei als PDF</a></p>
      <h2>Gutstunden</h2>
      <table class="inline-table">
        <tbody>
          {#each account.gutstunden.toepfe as t}
            <tr><td>Topf {t.topf}</td><td class="right mono">{(t.saldo_min / 60).toFixed(2).replace('.', ',')} h</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<div class="card tight table-wrap" style="margin-top:20px">
  <table>
    <thead><tr><th>Art</th><th>Von</th><th>Bis</th><th>Einheit</th><th>Status</th><th>Kommentar</th><th></th></tr></thead>
    <tbody>
      {#each list as a}
        <tr>
          <td>{a.label}</td><td class="mono">{dateDe(a.von)}</td><td class="mono">{dateDe(a.bis)}</td>
          <td>{a.einheit === 'tag' ? 'Tage' : a.einheit === 'halber_tag' ? 'halber Tag' : `${a.wert} h`}</td>
          <td><span class="badge {badge(a.status)}">{STATUS_LABEL[a.status]}</span></td>
          <td class="small">{a.kommentar ?? ''}{#if a.entscheidung_kommentar} <span class="muted">— {a.entscheidung_kommentar}</span>{/if}</td>
          <td class="right">{#if a.status === 'beantragt'}<button class="small" onclick={() => withdraw(a.id)}>Zurückziehen</button>{/if}</td>
        </tr>
      {:else}
        <tr><td colspan="7" class="muted">Keine Abwesenheiten.</td></tr>
      {/each}
    </tbody>
  </table>
</div>
