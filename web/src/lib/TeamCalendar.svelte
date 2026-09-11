<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { thisMonth, shiftMonth, monthLabel } from '$lib/fmt';

  let monat = $state(thisMonth());
  let data = $state<any>(null);
  let error = $state('');

  async function load() {
    try { data = await api.get(`/admin/calendar?monat=${monat}`); } catch (e) { error = errMsg(e); }
  }
  onMount(load);
  function go(delta: number) { monat = shiftMonth(monat, delta); load(); }

  const CODE: Record<string, string> = {
    urlaub: 'U', krank: 'K', arbeitsunfall: 'AU', freizeitunfall: 'FU', pflegeurlaub: 'PU', zeitausgleich: 'ZA',
    absonderung: 'AB', sonderurlaub: 'SU', pers_feiertag: 'PF', arzt: 'A', unbezahlt: 'UB', dienstreise: 'DR'
  };
  const days = $derived(() => {
    if (!data) return [] as { iso: string; d: number; wd: string; weekend: boolean; holiday?: string }[];
    const [y, m] = monat.split('-').map(Number);
    const n = new Date(y, m, 0).getDate();
    const hol = new Map<string, string>(data.feiertage.map((h: any) => [h.datum, h.name]));
    return Array.from({ length: n }, (_, i) => {
      const dt = new Date(y, m - 1, i + 1);
      const iso = `${monat}-${String(i + 1).padStart(2, '0')}`;
      return { iso, d: i + 1, wd: ['So', 'Mo', 'Di', 'Mi', 'Do', 'Fr', 'Sa'][dt.getDay()], weekend: dt.getDay() === 0 || dt.getDay() === 6, holiday: hol.get(iso) };
    });
  });
  const today = new Date().toISOString().slice(0, 10);
</script>

<div class="page-head" style="margin-top:32px">
  <h2 style="margin:0;font-size:24px">Abwesenheitskalender</h2>
  <div class="monthnav"><button onclick={() => go(-1)}>‹</button><span>{monthLabel(monat)}</span><button onclick={() => go(1)}>›</button></div>
</div>
{#if error}<div class="alert err">{error}</div>{/if}
{#if data}
  <div class="card tight table-wrap">
    <table class="cal">
      <thead>
        <tr>
          <th>Mitarbeiter</th>
          {#each days() as d}<th class="c" class:we={d.weekend || d.holiday} class:today={d.iso === today} title={d.holiday ?? ''}>{d.d}<br /><span class="xs muted">{d.wd}</span></th>{/each}
        </tr>
      </thead>
      <tbody>
        {#each data.rows as r}
          <tr>
            <td style="white-space:nowrap"><a href={`/admin/mitarbeiter/${r.id}`}>{r.name}</a></td>
            {#each days() as d}
              {@const t = r.tage[d.iso]}
              <td class="c" class:we={d.weekend || d.holiday} class:today={d.iso === today} title={t?.label ? `${t.label} (${t.status})` : d.holiday ?? ''}>
                {#if t?.art}<span class="cell" class:pending={t.status === 'beantragt'} class:half={t.einheit !== 'tag'}>{CODE[t.art] ?? '·'}</span>
                {:else if t?.frei}<span class="muted xs">–</span>{/if}
              </td>
            {/each}
          </tr>
        {:else}<tr><td colspan="32" class="muted">Keine aktiven Mitarbeitenden.</td></tr>{/each}
      </tbody>
    </table>
    <div class="table-foot">
      <span><span class="cell">U</span> genehmigt</span><span><span class="cell pending">U</span> beantragt</span><span><span class="cell half">U</span> halber Tag / Stunden</span>
      <span>U Urlaub · K Krank · ZA Zeitausgleich · PU Pflegeurlaub · AB Absonderung · SU Sonderurlaub · PF pers. Feiertag · A Arzt · DR Dienstreise · AU/FU Unfall · UB unbezahlt</span>
    </div>
  </div>
{/if}

<style>
  .cal th.c, .cal td.c { padding: 4px 2px; text-align: center; min-width: 26px; font-size: 12px; }
  .cal th.c { line-height: 1.2; }
  .cal td.c { height: 34px; }
  .cal .we { background: var(--row-weekend); }
  .cal .today { background: var(--row-today); }
  .cell { display: inline-block; min-width: 22px; padding: 2px 3px; border-radius: 4px; background: var(--info-bg); color: var(--info-fg); font-size: 11px; font-weight: 600; }
  .cell.pending { background: var(--warn-bg); color: var(--warn-fg); border: 1px dashed var(--warn-border); }
  .cell.half { background: linear-gradient(135deg, var(--info-bg) 50%, transparent 50%); }
</style>
