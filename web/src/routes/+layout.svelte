<script lang="ts">
  import '@fontsource/ibm-plex-sans/400.css';
  import '@fontsource/ibm-plex-sans/500.css';
  import '@fontsource/ibm-plex-sans/600.css';
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { goto, afterNavigate } from '$app/navigation';
  import { user, branding, loadBranding, loadUser } from '$lib/stores';
  import { api } from '$lib/api';
  import BrandMark from '$lib/BrandMark.svelte';

  let { children } = $props();
  let ready = $state(false);
  let menuOpen = $state(false);

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
  afterNavigate(() => (menuOpen = false));

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
  const links = $derived([
    ['/', 'Stempeln', true],
    ['/monat', 'Monatsübersicht', false],
    ['/abwesenheiten', 'Abwesenheiten', false],
    ['/konto', 'Mein Konto', false],
    ...($user?.rolle === 'admin' ? [['/admin', 'Verwaltung', false]] : [])
  ] as [string, string, boolean][]);
</script>

{#if !ready}
  <main><p class="muted">Lade …</p></main>
{:else if isTerminal || isLogin}
  {@render children()}
{:else if $user}
  <div class="app">
    <header class="topbar">
      <div class="topbar-inner">
        <div class="topbar-left">
          <a class="brand" href="/"><BrandMark /><span>{$branding.firmenname}</span></a>
          <nav>
            {#each links as [href, label, exact]}<a {href} class:active={active(href, exact)}>{label}</a>{/each}
          </nav>
        </div>
        <div class="topbar-right">
          <a class="user" href="/konto">{$user.vorname} {$user.nachname}</a>
          <button class="logout" onclick={logout}>Abmelden</button>
          <button class="menu-btn" aria-label="Menü" onclick={() => (menuOpen = !menuOpen)}><span></span><span></span><span></span></button>
        </div>
      </div>
      <div class="mobile-nav" class:open={menuOpen}>
        {#each links as [href, label, exact]}<a {href} class:active={active(href, exact)}>{label}</a>{/each}
        <a href="/login" onclick={(e) => { e.preventDefault(); logout(); }}>Abmelden</a>
      </div>
    </header>
    <main>{@render children()}</main>
    <footer class="foot">{$branding.fusszeile || $branding.firmenname}</footer>
  </div>
{/if}
