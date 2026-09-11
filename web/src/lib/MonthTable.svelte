<script lang="ts">
  import { hm, PUNCH_LABELS, warningText, todayIso, dateDe, days as fmtDays } from '$lib/fmt';

  interface Props {
    month: any;
    admin?: boolean;
    onstorno?: (punchId: number) => void;
    /** Mitarbeiter: Korrekturantrag für einen Tag stellen */
    onrequest?: (day: any) => void;
    /** Ansicht: Tabelle oder Karten (mobil) */
    view?: 'table' | 'cards';
    /** Tage (YYYY-MM-DD) mit offenem Korrekturantrag */
    pending?: Set<string>;
  }
  let { month, admin = false, onstorno, onrequest, view = 'table', pending = new Set() }: Props = $props();
  const today = todayIso();
  const isWeekend = (d: any) => d.weekday === 'Sa' || d.weekday === 'So';
  const dayNum = (iso: string) => iso.slice(8, 10) + '.';
  const monthName = $derived(new Date(month.monat + '-01').toLocaleDateString('de-AT', { month: 'long' }));
  const canRequest = (d: any) => onrequest && !d.future && !month.geschlossen && (d.is_working_day || d.punches.length) && !pending.has(d.date);
  const isPending = (d: any) => pending.has(d.date);
  const hintText = (d: any) => d.warnings.map(warningText).join('; ');
  const showDiff = (d: any) => !d.future && (d.target_min || d.worked_min || d.diff_min);
</script>

<div class="grid kpis-6">
  <div class="kpi compact"><div class="l">Soll</div><div class="v">{hm(month.soll_min)}</div></div>
  <div class="kpi compact"><div class="l">Ist gesamt</div><div class="v">{hm(month.ist_min + month.abwesenheit_min + month.feiertag_min)}</div></div>
  <div class="kpi compact"><div class="l">Differenz im Monat</div><div class="v" class:pos={month.diff_min > 0} class:neg={month.diff_min < 0}>{hm(month.diff_min, true)}</div></div>
  <div class="kpi compact"><div class="l">Saldo Monatsbeginn</div><div class="v" class:pos={month.saldo_start_min > 0} class:neg={month.saldo_start_min < 0}>{hm(month.saldo_start_min, true)}</div></div>
  <div class="kpi compact"><div class="l">Saldo Monatsende</div><div class="v" class:pos={month.saldo_ende_min > 0} class:neg={month.saldo_ende_min < 0}>{hm(month.saldo_ende_min, true)}</div></div>
  <div class="kpi compact"><div class="l">Resturlaub</div><div class="v">{fmtDays(month.urlaub.rest)} Tage</div></div>
</div>

{#if month.abwesenheit_nach_art.length}
  <p class="small muted" style="margin:12px 0 0">
    Arbeit {hm(month.ist_min)} · Abwesenheit {hm(month.abwesenheit_min)} · Feiertag {hm(month.feiertag_min)} ·
    {#each month.abwesenheit_nach_art as a, i}{i ? ' · ' : ''}{a.label}: {fmtDays(a.tage)} Tage{/each}
  </p>
{/if}

{#if view === 'cards'}
  <div style="margin-top:14px">
    {#each month.days as d}
      {#if (!d.future && (d.is_working_day || d.punches.length)) || d.absences.length}
        <div class="daycard" class:today={d.date === today} class:weekend={isWeekend(d)} class:holiday={d.holiday_name}>
          <div class="head">
            <span>{d.weekday}, {dayNum(d.date)} {monthName}{#if d.holiday_name} · {d.holiday_name}{/if}</span>
            <span class:pos={d.diff_min > 0} class:neg={d.diff_min < 0}>{showDiff(d) ? hm(d.diff_min, true) : ''}</span>
          </div>
          {#if d.is_working_day || d.punches.length}
            <div class="cells">
              <div><div class="l">Kommen</div><div>{d.first_in ?? '–'}</div></div>
              <div><div class="l">Gehen</div><div>{d.last_out ?? (d.open_shift ? 'offen' : '–')}</div></div>
              <div><div class="l">Pause</div><div>{d.break_min ? hm(d.break_min) : '–'}</div></div>
              <div><div class="l">Ist / Soll</div><div><strong>{hm(d.worked_min)}</strong> / {hm(d.target_min)}</div></div>
            </div>
          {/if}
          {#if d.warnings.length}<div class="hint">⚠ {hintText(d)}</div>{/if}
          <div class="row between">
            <span>{#each d.absences as a}<span class="badge info">{a.label}{#if a.einheit !== 'tag'} {hm(a.minutes)}{/if}</span> {/each}</span>
            {#if isPending(d)}<span class="badge warn">Korrektur beantragt</span>{:else if canRequest(d)}<button class="link small" style="font-size:12px" onclick={() => onrequest?.(d)}>Korrektur beantragen</button>{/if}
          </div>
        </div>
      {/if}
    {/each}
  </div>
{:else}
  <div class="card tight table-wrap" style="margin-top:20px">
    <table>
      <thead>
        <tr>
          <th>Datum</th><th></th><th class="right">Soll</th><th class="right">Kommen</th><th class="right">Gehen</th><th class="right">Pause</th>
          <th class="right">Ist</th><th>Abwesenheit</th><th class="right">Differenz</th><th>Hinweise</th><th></th>
        </tr>
      </thead>
      <tbody>
        {#each month.days as d}
          <tr class:weekend={isWeekend(d)} class:holiday={d.holiday_name} class:today={d.date === today}>
            <td>{dayNum(d.date)}</td>
            <td class="muted">{d.weekday}</td>
            <td class="right">{d.target_min ? hm(d.target_min) : ''}</td>
            <td class="right">{d.first_in ?? ''}</td>
            <td class="right">{d.last_out ?? ''}{#if d.open_shift}<span class="badge err">offen</span>{/if}</td>
            <td class="right">{d.break_min ? hm(d.break_min) : ''}</td>
            <td class="right" style="font-weight:500">{d.worked_min ? hm(d.worked_min) : ''}</td>
            <td>
              {#each d.absences as a}<span class="badge info">{a.label}{#if a.einheit !== 'tag'} {hm(a.minutes)}{/if}</span> {/each}
              {#if d.holiday_name}<span class="badge warn">{d.holiday_name}</span>{/if}
            </td>
            <td class="right" style="font-weight:500" class:pos={d.diff_min > 0} class:neg={d.diff_min < 0}>{showDiff(d) ? hm(d.diff_min, true) : ''}</td>
            <td class="hint" title={hintText(d)}>
              <span style="display:block;max-width:260px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{hintText(d)}</span>
              {#if admin && d.punches.length}
                <div class="muted xs" style="white-space:nowrap">
                  {#each d.punches as p, i}{i ? ' · ' : ''}{p.zeit} {PUNCH_LABELS[p.art]}{#if onstorno}<button class="link small" style="padding:0 3px;font-size:12px" title="Stornieren" onclick={() => onstorno(p.id)}>×</button>{/if}{/each}
                </div>
              {/if}
            </td>
            <td class="right">{#if isPending(d)}<span class="badge warn">Korrektur beantragt</span>{:else if canRequest(d)}<button class="small" onclick={() => onrequest?.(d)}>Korrektur beantragen</button>{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    <div class="table-foot">
      <span class="legend"><i style="background:var(--row-today)"></i>heute</span>
      <span class="legend"><i style="background:var(--row-weekend)"></i>Wochenende</span>
      <span class="legend"><i style="background:var(--row-holiday)"></i>Feiertag</span>
    </div>
  </div>
{/if}
