<script lang="ts">
  import { hm, PUNCH_LABELS, warningText, todayIso, dateDe } from '$lib/fmt';

  interface Props {
    month: any;
    admin?: boolean;
    onstorno?: (punchId: number) => void;
  }
  let { month, admin = false, onstorno }: Props = $props();
  const today = todayIso();

  function isWeekend(d: any) {
    return d.weekday === 'Sa' || d.weekday === 'So';
  }
</script>

<div class="grid cols-3" style="margin-bottom:1rem">
  <div class="card stat"><span class="v mono">{hm(month.soll_min)}</span><span class="l">Soll</span></div>
  <div class="card stat"><span class="v mono">{hm(month.ist_min + month.abwesenheit_min + month.feiertag_min)}</span><span class="l">Ist gesamt (Arbeit {hm(month.ist_min)}, Abwesenheit {hm(month.abwesenheit_min)}, Feiertag {hm(month.feiertag_min)})</span></div>
  <div class="card stat"><span class="v mono" class:pos={month.diff_min > 0} class:neg={month.diff_min < 0}>{hm(month.diff_min, true)}</span><span class="l">Differenz im Monat</span></div>
  <div class="card stat"><span class="v mono">{hm(month.saldo_start_min, true)}</span><span class="l">Saldo Monatsbeginn</span></div>
  <div class="card stat"><span class="v mono" class:pos={month.saldo_ende_min > 0} class:neg={month.saldo_ende_min < 0}>{hm(month.saldo_ende_min, true)}</span><span class="l">Saldo Monatsende</span></div>
  <div class="card stat"><span class="v">{month.urlaub.rest}</span><span class="l">Resturlaub (Tage), Urlaubsjahr ab {dateDe(month.urlaub.urlaubsjahr_von)}</span></div>
</div>

{#if month.abwesenheit_nach_art.length}
  <p class="small">
    {#each month.abwesenheit_nach_art as a, i}{i ? ' · ' : ''}{a.label}: {a.tage} Tage ({hm(a.minuten)}){/each}
  </p>
{/if}

<div class="table-wrap card" style="padding:0">
  <table>
    <thead>
      <tr>
        <th>Datum</th><th></th><th class="right">Soll</th><th>Kommen</th><th>Gehen</th><th class="right">Pause</th>
        <th class="right">Ist</th><th>Abwesenheit</th><th class="right">Diff</th><th>Hinweise</th>
      </tr>
    </thead>
    <tbody>
      {#each month.days as d}
        <tr class:weekend={isWeekend(d)} class:holiday={d.holiday_name} class:today={d.date === today}>
          <td class="mono">{dateDe(d.date)}</td>
          <td>{d.weekday}{#if d.holiday_name} <span class="badge info">{d.holiday_name}</span>{/if}</td>
          <td class="right mono">{d.target_min ? hm(d.target_min) : ''}</td>
          <td class="mono">{d.first_in ?? ''}</td>
          <td class="mono">{d.last_out ?? ''}{#if d.open_shift}<span class="badge err">offen</span>{/if}</td>
          <td class="right mono">{d.break_min ? hm(d.break_min) : ''}</td>
          <td class="right mono">{d.worked_min ? hm(d.worked_min) : ''}</td>
          <td>
            {#each d.absences as a}<span class="badge">{a.label}{#if a.einheit !== 'tag'} ({hm(a.minutes)}){/if}</span> {/each}
            {#if d.holiday_min}<span class="badge info">Feiertag {hm(d.holiday_min)}</span>{/if}
          </td>
          <td class="right mono" class:pos={d.diff_min > 0} class:neg={d.diff_min < 0}>{d.target_min || d.worked_min || d.diff_min ? hm(d.diff_min, true) : ''}</td>
          <td class="small">
            {#if d.warnings.length}<ul class="warn-list" style="margin:0">{#each d.warnings as w}<li>{warningText(w)}</li>{/each}</ul>{/if}
            {#if admin && d.punches.length}
              <div class="muted" style="margin-top:.2rem">
                {#each d.punches as p}
                  <span class="mono">{p.zeit}</span> {PUNCH_LABELS[p.art]} <span class="muted">({p.quelle})</span>
                  {#if onstorno}<button class="small" style="padding:0 .3rem" title="Stornieren" onclick={() => onstorno(p.id)}>×</button>{/if}<br />
                {/each}
              </div>
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>
