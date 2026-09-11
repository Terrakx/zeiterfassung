<script lang="ts">
  import { page } from '$app/state';
  import { user } from '$lib/stores';
  let { children } = $props();
  const tabs = [
    ['/admin', 'Übersicht', true],
    ['/admin/mitarbeiter', 'Mitarbeiter', false],
    ['/admin/antraege', 'Anträge', false],
    ['/admin/abschluss', 'Abschluss & Export', false],
    ['/admin/feiertage', 'Feiertage', false],
    ['/admin/einstellungen', 'Einstellungen', false]
  ] as const;
  function active(path: string, exact: boolean) {
    const p = page.url.pathname;
    return exact ? p === path : p.startsWith(path);
  }
</script>

{#if $user?.rolle !== 'admin'}
  <div class="alert err">Keine Berechtigung.</div>
{:else}
  <div class="tabs">
    {#each tabs as [path, label, exact]}<a href={path} class:active={active(path, exact)}>{label}</a>{/each}
  </div>
  {@render children()}
{/if}
