<script lang="ts">
  import { api, errMsg } from '$lib/api';
  import { ABSENCE_KINDS, todayIso } from '$lib/fmt';
  import { branding } from '$lib/stores';

  interface Props {
    /** Admin bucht für employeeId, sonst eigener Antrag */
    employeeId?: number;
    onsaved?: () => void;
  }
  let { employeeId, onsaved }: Props = $props();
  const admin = $derived(employeeId !== undefined);

  let art = $state('urlaub');
  let von = $state(todayIso());
  let bis = $state(todayIso());
  let einheit = $state('tag');
  let wert = $state(4);
  let kommentar = $state('');
  let error = $state('');
  let hinweise = $state<string[]>([]);
  let busy = $state(false);

  const kinds = $derived(ABSENCE_KINDS.filter((k) => admin || k.self));
  const isVacation = $derived(art === 'urlaub' || art === 'pers_feiertag');
  const showUnit = $derived(!isVacation || $branding.urlaub_halbe_tage || $branding.urlaub_stunden);

  $effect(() => {
    if (einheit !== 'tag') bis = von;
  });

  async function submit(e: Event) {
    e.preventDefault();
    error = '';
    hinweise = [];
    busy = true;
    try {
      const body = { art, von, bis, einheit, wert: einheit === 'stunden' ? wert : undefined, kommentar };
      const r: any = admin ? await api.post(`/employees/${employeeId}/absences`, body) : await api.post('/absences', body);
      hinweise = r?.hinweise ?? [];
      kommentar = '';
      onsaved?.();
    } catch (err) {
      error = errMsg(err);
    } finally {
      busy = false;
    }
  }
</script>

<form onsubmit={submit}>
  {#if error}<div class="alert err">{error}</div>{/if}
  {#each hinweise as h}<div class="alert warn">{h}</div>{/each}
  <div class="form-grid">
    <div class="field">
      <label for="art">Art</label>
      <select id="art" bind:value={art}>
        {#each kinds as k}<option value={k.art}>{k.label}</option>{/each}
      </select>
    </div>
    <div class="field">
      <label for="von">Von</label>
      <input id="von" type="date" bind:value={von} required />
    </div>
    <div class="field">
      <label for="bis">Bis</label>
      <input id="bis" type="date" bind:value={bis} required disabled={einheit !== 'tag'} />
    </div>
    {#if showUnit}
      <div class="field">
        <label for="einheit">Einheit</label>
        <select id="einheit" bind:value={einheit}>
          <option value="tag">Ganze Tage</option>
          {#if !isVacation || $branding.urlaub_halbe_tage}<option value="halber_tag">Halber Tag</option>{/if}
          {#if !isVacation || $branding.urlaub_stunden}<option value="stunden">Stunden</option>{/if}
        </select>
      </div>
    {/if}
    {#if einheit === 'stunden'}
      <div class="field">
        <label for="wert">Stunden</label>
        <input id="wert" type="number" step="0.25" min="0.25" max="24" bind:value={wert} />
      </div>
    {/if}
  </div>
  {#if isVacation && einheit !== 'tag'}
    <div class="alert warn small">{$branding.urlaub_hinweis}</div>
  {/if}
  <div class="field" style="margin-top:16px">
    <label for="k">Kommentar</label>
    <input id="k" bind:value={kommentar} placeholder="optional" />
  </div>
  <div class="row" style="justify-content:flex-end"><button class="primary" disabled={busy}>{admin ? 'Buchen' : 'Beantragen'}</button></div>
</form>
