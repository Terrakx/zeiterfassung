<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { hm, PUNCH_LABELS } from '$lib/fmt';
  import { branding } from '$lib/stores';
  import BrandMark from '$lib/BrandMark.svelte';

  let hhmm = $state('');
  let sec = $state('');
  let step = $state<'nr' | 'pin' | 'action' | 'done'>('nr');
  let nr = $state('');
  let pin = $state('');
  let status = $state<any>(null);
  let error = $state('');
  let busy = $state(false);
  let remaining = $state(0);
  let timer: any;
  let resetTimer: any;
  let countTimer: any;

  function tick() {
    const d = new Date();
    hhmm = d.toLocaleTimeString('de-AT', { hour: '2-digit', minute: '2-digit' });
    sec = ':' + String(d.getSeconds()).padStart(2, '0');
  }
  onMount(() => { tick(); timer = setInterval(tick, 1000); });
  onDestroy(() => { clearInterval(timer); clearTimeout(resetTimer); clearInterval(countTimer); });

  function reset() {
    step = 'nr'; nr = ''; pin = ''; status = null; error = '';
    clearTimeout(resetTimer); clearInterval(countTimer); remaining = 0;
  }
  function armReset(ms = 20000) {
    clearTimeout(resetTimer); clearInterval(countTimer);
    remaining = Math.round(ms / 1000);
    countTimer = setInterval(() => (remaining = Math.max(0, remaining - 1)), 1000);
    resetTimer = setTimeout(reset, ms);
  }
  function key(k: string) {
    error = '';
    armReset();
    if (step === 'nr') { if (nr.length < 10) nr += k; }
    else if (step === 'pin') { if (pin.length < 8) pin += k; }
  }
  function back() {
    if (step === 'nr') nr = nr.slice(0, -1);
    else if (step === 'pin') pin = pin.slice(0, -1);
  }
  async function next() {
    error = '';
    if (step === 'nr') { if (!nr) return; step = 'pin'; armReset(); return; }
    if (step === 'pin') {
      busy = true;
      try {
        status = await api.post('/terminal/punch', { personalnr: nr, pin });
        step = 'action';
        armReset();
      } catch (e) {
        error = errMsg(e) === 'nicht angemeldet' ? 'Personalnummer oder PIN falsch' : errMsg(e);
        pin = '';
      } finally { busy = false; }
    }
  }
  async function punch(art: string) {
    busy = true; error = '';
    try {
      status = await api.post('/terminal/punch', { personalnr: nr, pin, art });
      step = 'done';
      armReset(6000);
    } catch (e) { error = errMsg(e); armReset(); }
    finally { busy = false; }
  }
  function onKeydown(e: KeyboardEvent) {
    if (/^[0-9]$/.test(e.key)) key(e.key);
    else if (e.key === 'Backspace') back();
    else if (e.key === 'Enter') next();
    else if (e.key === 'Escape') reset();
  }
  const today = new Date().toLocaleDateString('de-AT', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
  const since = $derived(status?.heute?.at(-1)?.zeit ?? '');
</script>

<svelte:window onkeydown={onKeydown} />

<div class="terminal" class:dark={$branding.terminal_dunkel}>
  <div class="t-head">
    <div class="t-brand"><BrandMark />{$branding.firmenname}</div>
    <div>
      <div class="clock">{hhmm}<small>{sec}</small></div>
      <div class="t-date">{today}</div>
    </div>
  </div>

  <div class="t-body">
    {#if error}<div class="alert err">{error}</div>{/if}

    {#if step === 'nr' || step === 'pin'}
      <div class="t-entry">
        <div>
          <div class="t-prompt">{step === 'nr' ? 'Personalnummer eingeben' : 'PIN eingeben'}</div>
          <div class="t-display">{step === 'nr' ? nr : '•'.repeat(pin.length)}<span class="caret"></span></div>
          <div class="t-help">{step === 'nr' ? 'Anschließend PIN eingeben und mit OK bestätigen.' : 'PIN eingeben und mit OK bestätigen.'}</div>
        </div>
        <div class="keypad">
          {#each ['1','2','3','4','5','6','7','8','9'] as k}<button onclick={() => key(k)}>{k}</button>{/each}
          <button class="back" onclick={back}>⌫</button>
          <button onclick={() => key('0')}>0</button>
          <button class="primary" onclick={next} disabled={busy}>OK</button>
        </div>
      </div>
    {:else if step === 'action' && status}
      <div class="t-person">
        <div>
          <div class="t-name">{status.name}</div>
          {#if status.zustand === 'arbeitet'}<div class="t-status ok"><span class="dot"></span>eingestempelt seit {since}</div>
          {:else if status.zustand === 'pause'}<div class="t-status"><span class="dot"></span>in Pause seit {since}</div>
          {:else}<div class="t-status"><span class="dot"></span>nicht eingestempelt</div>{/if}
        </div>
        <div class="t-kpis">
          <div class="t-kpi"><div class="l">Heute gearbeitet</div><div class="v">{hm(status.tag.worked_min)}</div></div>
          <div class="t-kpi"><div class="l">Saldo bis gestern</div><div class="v" class:pos={status.saldo_min > 0} class:neg={status.saldo_min < 0}>{hm(status.saldo_min, true)}</div></div>
        </div>
      </div>
      <div class="actions">
        {#if status.zustand === 'draussen' && status.sperre}
          <div class="t-block">{status.sperre} Bitte an die Verwaltung wenden.</div>
        {:else if status.zustand === 'draussen'}
          <button class="primary" style="grid-column:1 / -1" onclick={() => punch('kommen')} disabled={busy}>Kommen</button>
        {:else if status.zustand === 'arbeitet'}
          <button onclick={() => punch('pause_start')} disabled={busy}>Pause</button>
          <button class="primary" onclick={() => punch('gehen')} disabled={busy}>Gehen</button>
        {:else}
          <button class="primary" onclick={() => punch('pause_ende')} disabled={busy}>Pause Ende</button>
          <button onclick={() => punch('gehen')} disabled={busy}>Gehen</button>
        {/if}
      </div>
    {:else if step === 'done' && status}
      <div style="text-align:center">
        <div class="t-name" style="margin-bottom:0">{status.name}</div>
        <div class="t-done">✓ {PUNCH_LABELS[status.heute.at(-1)?.art]} {status.heute.at(-1)?.zeit}</div>
        <div class="t-status">heute {hm(status.tag.worked_min)} · Saldo bis gestern {hm(status.saldo_min, true)}</div>
      </div>
    {/if}
  </div>

  <div class="t-foot">
    {#if step === 'nr'}
      <span>{$branding.fusszeile || $branding.firmenname}</span><span>Stempelterminal</span>
    {:else}
      <button onclick={reset}>{step === 'done' ? 'Fertig' : 'Abbrechen'}</button>
      <span>Automatischer Reset in {remaining} s</span>
    {/if}
  </div>
</div>
