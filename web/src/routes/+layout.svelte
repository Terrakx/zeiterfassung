<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { user, branding, loadBranding, loadUser } from '$lib/stores';
  import { api } from '$lib/api';

  let { children } = $props();
  let ready = $state(false);

  const isTerminal = $derived(page.url.pathname.startsWith('/terminal'));
  const isLogin = $derived(page.url.pathname === '/login');

  onMount(async () => {
    // Host-basiertes Routing: timecard.local zeigt direkt das Terminal.
    if (location.hostname.startsWith('timecard') && page.url.pathname === '/') {
      await goto('/terminal', { replaceState: true });
    }
    await Promise.all([loadBranding(), loadUser()]);
    ready = true;
  });

  $effect(() => {
    if (!ready || isTerminal) return;
    if ($user === null && !isLogin) goto('/login');
    if ($user && isLogin) goto('/');
  });

  async function logout() {
    await api.post('/auth/logout');
    user.set(null);
    goto('/login');
  }

  function active(path: string, exact = false) {
    const p = page.url.pathname;
    return exact ? p === path : p === path || p.startsWith(path + '/');
  }
</script>

{#if !ready}
  <main><p class="muted">Lade …</p></main>
{:else if isTerminal || isLogin}
  {@render children()}
{:else if $user}
  <div class="app">
    <header class="topbar">
      <a class="brand" href="/">
        {#if $branding.logo_data_url}<img src={$branding.logo_data_url} alt="" />{/if}
        {$branding.firmenname}
      </a>
      <nav>
        <a href="/" class:active={active('/', true)}>Stempeln</a>
        <a href="/monat" class:active={active('/monat')}>Monat</a>
        <a href="/abwesenheiten" class:active={active('/abwesenheiten')}>Abwesenheiten</a>
        {#if $user.rolle === 'admin'}
          <a href="/admin" class:active={active('/admin')}>Verwaltung</a>
        {/if}
      </nav>
      <div class="spacer"></div>
      <a class="user" href="/konto" style="color:#fff">{$user.vorname} {$user.nachname}</a>
      <button onclick={logout}>Abmelden</button>
    </header>
    <main>{@render children()}</main>
    {#if $branding.fusszeile}<footer class="foot">{$branding.fusszeile}</footer>{/if}
  </div>
{/if}
