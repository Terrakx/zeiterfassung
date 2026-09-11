<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg } from '$lib/api';
  let rows = $state<any[]>([]);
  let error = $state('');
  let open = $state<number | null>(null);
  onMount(async () => { try { rows = await api.get('/admin/audit?limit=300'); } catch (e) { error = errMsg(e); } });
  const fmt = (ts: string) => ts.slice(0, 16).replace('T', ' ');
</script>

<h1>Protokoll</h1>
<p class="small muted">Alle Änderungen durch Verwaltung und Mitarbeiter, unveränderlich. Klick auf eine Zeile zeigt Details.</p>
{#if error}<div class="alert err">{error}</div>{/if}
<div class="card table-wrap" style="padding:0">
  <table>
    <thead><tr><th>Zeit (UTC)</th><th>Wer</th><th>Aktion</th><th>Ziel</th></tr></thead>
    <tbody>
      {#each rows as r}
        <tr onclick={() => (open = open === r.id ? null : r.id)} style="cursor:pointer">
          <td class="mono">{fmt(r.ts)}</td><td>{r.actor}</td><td>{r.aktion}</td><td class="mono small">{r.ziel ?? ''}</td>
        </tr>
        {#if open === r.id}
          <tr><td colspan="4"><pre class="small" style="white-space:pre-wrap;margin:0">{JSON.stringify({ vorher: r.vorher, nachher: r.nachher }, null, 2)}</pre></td></tr>
        {/if}
      {/each}
    </tbody>
  </table>
</div>
