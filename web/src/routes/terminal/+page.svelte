<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { hm, PUNCH_LABELS } from '$lib/fmt';
  import { branding } from '$lib/stores';

  let clock = $state('');
  let step = $state<'nr' | 'pin' | 'action' | 'done'>('nr');
  let nr = $state('');
  let pin = $state('');
  let status = $state<any>(null);
  let error = $state('');
  let busy = $state(false);
  let timer: any;
  let resetTimer: any;

  function tick() {
    const d = new Date();
    clock = d.toLocaleTimeString('de-AT', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }
  onMount(() => { tick(); timer = setInterval(tick, 1000); });
  onDestroy(() => { clearInterval(timer); clearTimeout(resetTimer); });

  function reset() {
    step = 'nr'; nr = ''; pin = ''; status = null; error = '';
    clearTimeout(resetTimer);
  }
  function armReset(ms = 20000) {
    clearTimeout(resetTimer);
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
</script>

<svelte:window onkeydown={onKeydown} />

<div class="terminal">
  <div style="text-align:center">
    {#if $branding.logo_data_url}<img src={$branding.logo_data_url} alt="" style="max-height:50px" />{/if}
    <div class="muted">{$branding.firmenname}</div>
    <div class="clock">{clock}</div>
    <div class="muted">{new Date().toLocaleDateString('de-AT', { weekday: 'long', day: '2-digit', month: '2-digit', year: 'numeric' })}</div>
  </div>

  {#if error}<div class="alert err" style="margin:0">{error}</div>{/if}

  {#if step === 'nr' || step === 'pin'}
    <div style="text-align:center">
      <div class="muted">{step === 'nr' ? 'Personalnummer eingeben' : 'PIN eingeben'}</div>
      <div class="pin">{step === 'nr' ? nr || ' ' : '•'.repeat(pin.length) || ' '}</div>
    </div>
    <div class="keypad">
      {#each ['1','2','3','4','5','6','7','8','9'] as k}<button onclick={() => key(k)}>{k}</button>{/each}
      <button onclick={back}>⌫</button>
      <button onclick={() => key('0')}>0</button>
      <button class="primary" onclick={next} disabled={busy}>OK</button>
    </div>
    {#if step === 'pin'}<button onclick={reset}>Abbrechen</button>{/if}
  {:else if step === 'action' && status}
    <div style="text-align:center">
      <div style="font-size:1.6rem;font-weight:600">{status.name}</div>
      <div class="muted">
        {#if status.zustand === 'arbeitet'}eingestempelt seit {status.heute.at(-1)?.zeit}
        {:else if status.zustand === 'pause'}in Pause seit {status.heute.at(-1)?.zeit}
        {:else}nicht eingestempelt{/if}
        · heute {hm(status.tag.worked_min)} · Saldo bis gestern {hm(status.saldo_min, true)}
      </div>
    </div>
    <div class="actions">
      {#if status.zustand === 'draussen'}
        <button class="primary big" onclick={() => punch('kommen')} disabled={busy}>Kommen</button>
      {:else if status.zustand === 'arbeitet'}
        <button class="big" onclick={() => punch('pause_start')} disabled={busy}>Pause</button>
        <button class="primary big" onclick={() => punch('gehen')} disabled={busy}>Gehen</button>
      {:else}
        <button class="primary big" onclick={() => punch('pause_ende')} disabled={busy}>Pause Ende</button>
        <button class="big" onclick={() => punch('gehen')} disabled={busy}>Gehen</button>
      {/if}
    </div>
    <button onclick={reset}>Abbrechen</button>
  {:else if step === 'done' && status}
    <div style="text-align:center">
      <div style="font-size:1.6rem;font-weight:600">{status.name}</div>
      <div style="font-size:2rem;color:var(--ok)">✓ {PUNCH_LABELS[status.heute.at(-1)?.art]} {status.heute.at(-1)?.zeit}</div>
      <div class="muted">heute {hm(status.tag.worked_min)} · Saldo bis gestern {hm(status.saldo_min, true)}</div>
    </div>
    <button onclick={reset}>Fertig</button>
  {/if}
</div>
