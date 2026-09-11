<script lang="ts">
  import { api, errMsg } from '$lib/api';
  import { dateDe, PUNCH_LABELS } from '$lib/fmt';

  interface Props {
    onsaved?: () => void;
  }
  let { onsaved }: Props = $props();

  let dlg: HTMLDialogElement;
  let day = $state<any>(null);
  let typ = $state<'einfuegen' | 'stornieren'>('einfuegen');
  let zeit = $state('');
  let art = $state('gehen');
  let punchId = $state<number | null>(null);
  let begruendung = $state('');
  let error = $state('');
  let busy = $state(false);

  export function open(d: any) {
    day = d;
    typ = 'einfuegen';
    zeit = '';
    art = d.open_shift ? 'gehen' : 'kommen';
    punchId = d.punches?.[0]?.id ?? null;
    begruendung = '';
    error = '';
    dlg.showModal();
  }

  async function submit(e: Event) {
    e.preventDefault();
    error = '';
    busy = true;
    try {
      await api.post('/punch-requests', {
        typ,
        datum: day.date,
        zeit: typ === 'einfuegen' ? zeit : undefined,
        art: typ === 'einfuegen' ? art : undefined,
        punch_id: typ === 'stornieren' ? punchId : undefined,
        begruendung
      });
      dlg.close();
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
    <h2 style="margin-top:0">Korrektur beantragen: {dateDe(day.date)}</h2>
    {#if error}<div class="alert err">{error}</div>{/if}
    <form onsubmit={submit}>
      <div class="row" style="margin-bottom:.8rem">
        <label style="margin:0"><input type="radio" bind:group={typ} value="einfuegen" />Stempelung nachtragen</label>
        <label style="margin:0"><input type="radio" bind:group={typ} value="stornieren" disabled={!day.punches?.length} />Stempelung streichen</label>
      </div>
      {#if typ === 'einfuegen'}
        <div class="form-grid">
          <div class="field"><label for="cz">Uhrzeit</label><input id="cz" type="time" bind:value={zeit} required /></div>
          <div class="field"><label for="ca">Art</label>
            <select id="ca" bind:value={art}>
              {#each Object.entries(PUNCH_LABELS) as [k, v]}<option value={k}>{v}</option>{/each}
            </select></div>
        </div>
      {:else}
        <div class="field"><label for="cp">Stempelung</label>
          <select id="cp" bind:value={punchId}>
            {#each day.punches as p}<option value={p.id}>{p.zeit} {PUNCH_LABELS[p.art]}</option>{/each}
          </select></div>
      {/if}
      <div class="field"><label for="cb">Begründung</label><input id="cb" bind:value={begruendung} required placeholder="z. B. Gehen vergessen, Büro um 17:05 verlassen" /></div>
      <div class="row">
        <button class="primary" disabled={busy}>Antrag senden</button>
        <button type="button" onclick={() => dlg.close()}>Abbrechen</button>
      </div>
    </form>
  {/if}
</dialog>
