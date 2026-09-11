<script lang="ts">
  import { page } from '$app/state';
  import { afterNavigate } from '$app/navigation';
  import { onMount } from 'svelte';
  import { user } from '$lib/stores';
  import { api } from '$lib/api';
  let { children } = $props();
  let offen = $state<{ abwesenheiten: number; korrekturen: number; gesamt: number } | null>(null);
  async function loadCount() {
    try { offen = await api.get('/admin/open-count'); } catch { offen = null; }
  }
  onMount(loadCount);
  afterNavigate(loadCount);
  const tabs = [
    ['/admin', 'Übersicht', true],
    ['/admin/mitarbeiter', 'Mitarbeiter', false],
    ['/admin/antraege', 'Anträge', false],
    ['/admin/abschluss', 'Abschluss & Export', false],
    ['/admin/feiertage', 'Feiertage', false],
    ['/admin/einstellungen', 'Einstellungen', false],
    ['/admin/protokoll', 'Protokoll', false]
  ] as const;
  function active(path: string, exact: boolean) {
    const p = page.url.pathname;
    return exact ? p === path : p.startsWith(path);
  }
</script>

{#if $user?.rolle !== 'admin'}
  <div class="alert err">Keine Berechtigung.</div>
{:else}
  <div class="tabs bar">
    {#each tabs as [path, label, exact]}
      <a href={path} class:active={active(path, exact)}>{label}{#if path === '/admin/antraege' && offen?.gesamt}<span class="badge warn" style="margin-left:6px" title="{offen.abwesenheiten} Abwesenheiten, {offen.korrekturen} Korrekturen">{offen.gesamt} zu erledigen</span>{/if}</a>
    {/each}
  </div>
  {@render children()}
{/if}
