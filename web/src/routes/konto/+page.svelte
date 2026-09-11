<script lang="ts">
  import { api, errMsg } from '$lib/api';
  import { user, loadUser } from '$lib/stores';

  let altes = $state('');
  let neues = $state('');
  let neues2 = $state('');
  let pwPasswort = $state('');
  let pin = $state('');
  let msg = $state('');
  let error = $state('');

  async function changePw(e: Event) {
    e.preventDefault();
    msg = error = '';
    if (neues !== neues2) { error = 'Passwörter stimmen nicht überein'; return; }
    try {
      await api.post('/auth/password', { altes_passwort: altes, neues_passwort: neues });
      msg = 'Passwort geändert.';
      altes = neues = neues2 = '';
    } catch (err) { error = errMsg(err); }
  }
  async function changePin(e: Event) {
    e.preventDefault();
    msg = error = '';
    try {
      await api.post('/auth/pin', { passwort: pwPasswort, pin });
      msg = 'PIN gesetzt.';
      pwPasswort = pin = '';
      await loadUser();
    } catch (err) { error = errMsg(err); }
  }
</script>

<div class="page-head"><h1>Mein Konto</h1></div>
{#if msg}<div class="alert ok">{msg}</div>{/if}
{#if error}<div class="alert err">{error}</div>{/if}
<div class="grid cols-2">
  <div class="card" style="margin:0">
    <div class="card-title">Passwort ändern</div>
    <form onsubmit={changePw}>
      <div class="field"><label for="a">Aktuelles Passwort</label><input id="a" type="password" bind:value={altes} required /></div>
      <div class="field"><label for="n">Neues Passwort (mind. 8 Zeichen)</label><input id="n" type="password" bind:value={neues} required minlength="8" /></div>
      <div class="field"><label for="n2">Wiederholen</label><input id="n2" type="password" bind:value={neues2} required /></div>
      <button class="primary">Speichern</button>
    </form>
  </div>
  <div class="card" style="margin:0">
    <div class="card-title">Terminal-PIN {#if $user?.hat_pin}<span class="badge ok">gesetzt</span>{:else}<span class="badge warn">nicht gesetzt</span>{/if}</div>
    <p class="small muted">Mit Personalnummer <strong>{$user?.personalnr}</strong> und PIN stempeln Sie am Terminal.</p>
    <form onsubmit={changePin}>
      <div class="field"><label for="pp">Passwort zur Bestätigung</label><input id="pp" type="password" bind:value={pwPasswort} required /></div>
      <div class="field"><label for="pin">Neue PIN (4 bis 8 Ziffern)</label><input id="pin" inputmode="numeric" pattern="[0-9]*" bind:value={pin} required /></div>
      <button class="primary">PIN setzen</button>
    </form>
  </div>
</div>
