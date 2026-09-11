<script lang="ts">
  import { branding } from '$lib/stores';
  const initial = $derived(($branding.firmenname || 'Z').trim().charAt(0).toUpperCase());
</script>

{#if $branding.logo_data_url}
  <img src={$branding.logo_data_url} alt="" />
{:else}
  <span class="brand-mark" aria-hidden="true">{initial}</span>
{/if}
