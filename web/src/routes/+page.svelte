<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { hm, PUNCH_LABELS, warningText, days } from '$lib/fmt';

  let status = $state<any>(null);
  let account = $state<any>(null);
  let error = $state('');
  let busy = $state(false);
  let clock = $state('');
  let timer: any;

  function tick() {
    clock = new Date().toLocaleTimeString('de-AT', { hour: '2-digit', minute: '2-digit' });
  }
  async function load() {
    try {
      [status, account] = await Promise.all([api.get<any>('/punch/status'), api.get<any>('/absences/account')]);
    } catch (e) {
      error = errMsg(e);
    }
  }
  async function punch(art: string) {
    error = '';
    busy = true;
    try {
      status = await api.post('/punch', { art });
      account = await api.get('/absences/account');
    } catch (e) {
      error = errMsg(e);
    } finally {
      busy = false;
    }
  }
  onMount(() => { tick(); timer = setInterval(tick, 15000); load(); });
  onDestroy(() => clearInterval(timer));

  const zustand = $derived(status?.zustand ?? 'draussen');
  const since = $derived(status?.heute?.at(-1)?.zeit ?? '');
  const today = new Date().toLocaleDateString('de-AT', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
  const QUELLE: Record<string, string> = { terminal: 'Terminal', portal: 'Portal', admin: 'Verwaltung', import: 'Import' };
</script>

<div class="page-head">
  <h1>Stempeln</h1>
  <span class="page-date">{today}</span>
</div>
{#if error}<div class="alert err">{error}</div>{/if}

{#if status}
  <div class="grid cols-2">
    <div class="card" style="margin:0;display:flex;flex-direction:column;gap:20px">
      <div class="row between" style="align-items:flex-start">
        <div>
          <div class="big-time">{clock}</div>
          <div class="xs muted" style="margin-top:6px">Aktuelle Uhrzeit</div>
        </div>
        {#if zustand === 'arbeitet'}<span class="badge ok"><span class="dot"></span>eingestempelt seit {since}</span>
        {:else if zustand === 'pause'}<span class="badge warn"><span class="dot"></span>Pause seit {since}</span>
        {:else}<span class="badge">nicht eingestempelt</span>{/if}
      </div>
      <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
        {#if zustand === 'draussen' && status.sperre}
          <div class="alert warn" style="grid-column:1 / -1;margin:0">{status.sperre} Bitte an die Verwaltung wenden.</div>
        {:else if zustand === 'draussen'}
          <button class="action primary" style="grid-column:1 / -1" onclick={() => punch('kommen')} disabled={busy}>Kommen</button>
        {:else if zustand === 'arbeitet'}
          <button class="action" onclick={() => punch('pause_start')} disabled={busy}>Pause</button>
          <button class="action primary" onclick={() => punch('gehen')} disabled={busy}>Gehen</button>
        {:else}
          <button class="action primary" onclick={() => punch('pause_ende')} disabled={busy}>Pause Ende</button>
          <button class="action" onclick={() => punch('gehen')} disabled={busy}>Gehen</button>
        {/if}
      </div>
      <div>
        <div class="section-label">Heutige Stempelungen</div>
        {#if status.heute.length === 0}
          <p class="small muted" style="margin:0">Noch keine Stempelung.</p>
        {:else}
          <div class="stamp-list">
            {#each status.heute as p}
              <div><span>{p.zeit}</span><span>{PUNCH_LABELS[p.art]}</span><span>{QUELLE[p.quelle] ?? p.quelle}</span></div>
            {/each}
          </div>
        {/if}
      </div>
      {#each status.tag.warnings as w}<div class="alert warn" style="margin:0">{warningText(w)}</div>{/each}
    </div>

    <div style="display:flex;flex-direction:column;gap:20px">
      <div class="grid kpis">
        <div class="kpi"><div class="l">Ist heute</div><div class="v">{hm(status.tag.worked_min)}</div></div>
        <div class="kpi"><div class="l">Soll heute</div><div class="v">{hm(status.tag.target_min)}</div></div>
        <div class="kpi"><div class="l">Gleitzeitsaldo bis gestern</div><div class="v" class:pos={status.saldo_min > 0} class:neg={status.saldo_min < 0}>{hm(status.saldo_min, true)}</div></div>
        {#if account}
          <div class="kpi"><div class="l">Resturlaub</div><div class="v">{days(account.urlaub.rest ?? 0)} Tage</div></div>
          <div class="kpi"><div class="l">davon geplant</div><div class="v">{days(account.urlaub.geplant ?? 0)} Tage</div></div>
          <div class="kpi"><div class="l">Gutstunden</div><div class="v">{hm(account.gutstunden.toepfe.reduce((s: number, t: any) => s + t.saldo_min, 0))}</div></div>
        {/if}
      </div>
      <div class="linklist">
        <a href="/monat"><span>Monatsübersicht {new Date().toLocaleDateString('de-AT', { month: 'long' })}</span><span>Öffnen ›</span></a>
        <a href="/abwesenheiten"><span>Abwesenheit beantragen</span><span>Öffnen ›</span></a>
      </div>
    </div>
  </div>
{/if}
