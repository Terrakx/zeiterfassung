<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  import { hm, PUNCH_LABELS, warningText, dateDe } from '$lib/fmt';

  let status = $state<any>(null);
  let account = $state<any>(null);
  let error = $state('');
  let busy = $state(false);

  async function load() {
    try {
      [status, account] = await Promise.all([api.get('/punch/status'), api.get('/absences/account')]);
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

  onMount(load);
  const zustand = $derived(status?.zustand ?? 'draussen');
</script>

<h1>Stempeln</h1>
{#if error}<div class="alert err">{error}</div>{/if}

{#if status}
  <div class="grid cols-2">
    <div class="card">
      <div class="row" style="justify-content:space-between">
        <div>
          <div class="muted small">{dateDe(status.tag.date)} · {status.jetzt} Uhr</div>
          <div style="font-size:1.3rem;font-weight:600">
            {#if zustand === 'arbeitet'}<span class="badge ok">eingestempelt</span>
            {:else if zustand === 'pause'}<span class="badge warn">in Pause</span>
            {:else}<span class="badge">nicht eingestempelt</span>{/if}
          </div>
        </div>
      </div>
      <div class="row" style="margin-top:1rem">
        {#if zustand === 'draussen'}
          <button class="primary big" onclick={() => punch('kommen')} disabled={busy}>Kommen</button>
        {:else if zustand === 'arbeitet'}
          <button class="big" onclick={() => punch('pause_start')} disabled={busy}>Pause</button>
          <button class="primary big" onclick={() => punch('gehen')} disabled={busy}>Gehen</button>
        {:else}
          <button class="primary big" onclick={() => punch('pause_ende')} disabled={busy}>Pause Ende</button>
          <button class="big" onclick={() => punch('gehen')} disabled={busy}>Gehen</button>
        {/if}
      </div>
      <h3>Heute</h3>
      {#if status.heute.length === 0}
        <p class="muted small">Noch keine Stempelung.</p>
      {:else}
        <table>
          <tbody>
            {#each status.heute as p}
              <tr><td class="mono">{p.zeit}</td><td>{PUNCH_LABELS[p.art]}</td><td class="muted small">{p.quelle}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
      {#if status.tag.warnings.length}
        <ul class="warn-list">{#each status.tag.warnings as w}<li>{warningText(w)}</li>{/each}</ul>
      {/if}
    </div>

    <div class="card">
      <div class="grid cols-3">
        <div class="stat"><span class="v mono">{hm(status.tag.worked_min)}</span><span class="l">Ist heute</span></div>
        <div class="stat"><span class="v mono">{hm(status.tag.target_min)}</span><span class="l">Soll heute</span></div>
        <div class="stat"><span class="v mono" class:pos={status.saldo_min > 0} class:neg={status.saldo_min < 0}>{hm(status.saldo_min, true)}</span><span class="l">Gleitzeitsaldo bis gestern</span></div>
      </div>
      {#if account}
        <hr style="border:0;border-top:1px solid var(--border);margin:1rem 0" />
        <div class="grid cols-3">
          <div class="stat"><span class="v">{account.urlaub.rest ?? 0}</span><span class="l">Resturlaub (Tage)</span></div>
          <div class="stat"><span class="v">{account.urlaub.geplant ?? 0}</span><span class="l">davon geplant</span></div>
          <div class="stat">
            <span class="v mono">{hm(account.gutstunden.toepfe.reduce((s: number, t: any) => s + t.saldo_min, 0))}</span>
            <span class="l">Gutstunden</span>
          </div>
        </div>
      {/if}
      <p class="small" style="margin-top:1rem"><a href="/monat">Monatsübersicht</a> · <a href="/abwesenheiten">Abwesenheit beantragen</a></p>
    </div>
  </div>
{/if}
