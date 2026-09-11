<script lang="ts">
  import { goto } from '$app/navigation';
  import { api, errMsg } from '$lib/api';
  import { user, branding, type User } from '$lib/stores';

  let username = $state('');
  let password = $state('');
  let error = $state('');
  let busy = $state(false);

  async function submit(e: Event) {
    e.preventDefault();
    error = '';
    busy = true;
    try {
      const u = await api.post<User>('/auth/login', { username, password });
      user.set(u);
      goto('/');
    } catch (err) {
      error = errMsg(err) === 'nicht angemeldet' ? 'Benutzername oder Passwort falsch' : errMsg(err);
    } finally {
      busy = false;
    }
  }
</script>

<div class="login">
  <div class="card">
    {#if $branding.logo_data_url}
      <p style="text-align:center"><img src={$branding.logo_data_url} alt="" style="max-height:60px" /></p>
    {/if}
    <h1>{$branding.firmenname}</h1>
    {#if error}<div class="alert err">{error}</div>{/if}
    <form onsubmit={submit}>
      <div class="field">
        <label for="u">Benutzername</label>
        <input id="u" bind:value={username} autocomplete="username" required />
      </div>
      <div class="field">
        <label for="p">Passwort</label>
        <input id="p" type="password" bind:value={password} autocomplete="current-password" required />
      </div>
      <button class="primary" style="width:100%" disabled={busy}>Anmelden</button>
    </form>
    <p class="small muted" style="text-align:center;margin-top:1rem"><a href="/terminal">Zum Stempelterminal</a></p>
  </div>
</div>
