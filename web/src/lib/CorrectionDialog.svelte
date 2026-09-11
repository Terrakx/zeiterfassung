<script lang="ts">
  import { api, errMsg } from '$lib/api';
  import { dateDe, PUNCH_LABELS } from '$lib/fmt';

  interface Props {
    onsaved?: () => void;
  }
  let { onsaved }: Props = $props();

  type Row = { zeit: string; art: string };
  let dlg = $state<HTMLDialogElement>();
  let day = $state<any>(null);
  let rows = $state<Row[]>([]);
  let begruendung = $state('');
  let error = $state('');
  let busy = $state(false);

  export function open(d: any) {
    day = d;
    // Vorbelegung: die aktuellen Stempelungen des Tages (Zeiten nach Mitternacht ohne „+1“)
    rows = (d.punches ?? []).map((p: any) => ({ zeit: String(p.zeit).slice(0, 5), art: p.art }));
    if (rows.length === 0) standardDay();
    begruendung = '';
    error = '';
    dlg?.showModal();
  }
  function standardDay() {
    const soll = day?.target_min ?? 480;
    const end = 8 * 60 + 30 + soll; // 08:00 Beginn, 30 min Pause
    const hm = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
    rows = soll > 360
      ? [{ zeit: '08:00', art: 'kommen' }, { zeit: '12:00', art: 'pause_start' }, { zeit: '12:30', art: 'pause_ende' }, { zeit: hm(end), art: 'gehen' }]
      : [{ zeit: '08:00', art: 'kommen' }, { zeit: hm(8 * 60 + soll), art: 'gehen' }];
  }
  function add() {
    const last = rows[rows.length - 1];
    const next = last ? (last.art === 'kommen' || last.art === 'pause_ende' ? 'gehen' : 'kommen') : 'kommen';
    rows = [...rows, { zeit: last?.zeit ?? '08:00', art: next }];
  }
  function remove(i: number) {
    rows = rows.filter((_, j) => j !== i);
  }
  const sorted = $derived([...rows].sort((a, b) => a.zeit.localeCompare(b.zeit)));
  const check = $derived(() => {
    let s = 'draussen';
    for (const r of sorted) {
      const k = `${s}:${r.art}`;
      const next: Record<string, string> = { 'draussen:kommen': 'arbeitet', 'arbeitet:gehen': 'draussen', 'arbeitet:pause_start': 'pause', 'pause:pause_ende': 'arbeitet', 'pause:gehen': 'draussen' };
      if (!next[k]) return `Reihenfolge nicht schlüssig bei ${r.zeit} ${PUNCH_LABELS[r.art]}`;
      s = next[k];
    }
    if (sorted.length && s !== 'draussen') return 'Der Tag muss mit „Gehen“ enden';
    return '';
  });
  const worked = $derived(() => {
    let w = 0, start: number | null = null;
    const min = (t: string) => Number(t.slice(0, 2)) * 60 + Number(t.slice(3, 5));
    for (const r of sorted) {
      if (r.art === 'kommen' || r.art === 'pause_ende') start = min(r.zeit);
      else if ((r.art === 'gehen' || r.art === 'pause_start') && start !== null) { w += min(r.zeit) - start; start = null; }
    }
    return w;
  });

  async function submit(e: Event) {
    e.preventDefault();
    error = '';
    busy = true;
    try {
      await api.post('/punch-requests', { typ: 'tag', datum: day.date, stempelungen: sorted, begruendung });
      dlg?.close();
      onsaved?.();
    } catch (err) {
      error = errMsg(err);
    } finally {
      busy = false;
    }
  }
</script>

<dialog bind:this={dlg}>
  {#if day}
    <h2>Korrektur beantragen: {day.weekday}, {dateDe(day.date)}</h2>
    <p class="small muted" style="margin-top:-8px">Tragen Sie ein, wie der Tag tatsächlich war. Nach Genehmigung ersetzt diese Folge alle bisherigen Stempelungen des Tages.</p>
    {#if error}<div class="alert err">{error}</div>{/if}
    <form onsubmit={submit}>
      <div class="rowlist">
        {#each rows as r, i}
          <div style="padding:6px 0">
            <input type="time" bind:value={r.zeit} required style="width:120px" aria-label="Uhrzeit" />
            <select bind:value={r.art} style="flex:1" aria-label="Art">
              {#each Object.entries(PUNCH_LABELS) as [k, v]}<option value={k}>{v}</option>{/each}
            </select>
            <button type="button" class="small" onclick={() => remove(i)} title="Zeile entfernen">×</button>
          </div>
        {/each}
      </div>
      <div class="row" style="margin:10px 0 16px">
        <button type="button" class="small" onclick={add}>+ Stempelung</button>
        <button type="button" class="small" onclick={standardDay}>Standardtag einsetzen</button>
        <button type="button" class="small" onclick={() => (rows = [])}>Alle entfernen</button>
        <span class="small muted" style="margin-left:auto">
          {#if check()}<span style="color:var(--err)">{check()}</span>
          {:else if sorted.length}Arbeitszeit {Math.floor(worked() / 60)}:{String(worked() % 60).padStart(2, '0')}{#if day.target_min} · Soll {Math.floor(day.target_min / 60)}:{String(day.target_min % 60).padStart(2, '0')}{/if}
          {:else}Tag ohne Stempelungen{/if}
        </span>
      </div>
      <div class="field"><label for="cb">Begründung</label><input id="cb" bind:value={begruendung} required placeholder="z. B. Gehen vergessen, Büro um 17:05 verlassen" /></div>
      <div class="row" style="justify-content:flex-end">
        <button type="button" onclick={() => dlg?.close()}>Abbrechen</button>
        <button class="primary" disabled={busy || !!check()}>Antrag senden</button>
      </div>
    </form>
  {/if}
</dialog>
