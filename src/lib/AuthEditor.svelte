<script lang="ts">
  import type { Auth } from './types';

  let { auth = $bindable() }: { auth: Auth } = $props();

  function setType(type: string) {
    if (type === 'bearer') {
      auth = { type: 'bearer', token: auth.type === 'bearer' ? auth.token : '' };
    } else if (type === 'basic') {
      auth = {
        type: 'basic',
        username: auth.type === 'basic' ? auth.username : '',
        password: auth.type === 'basic' ? auth.password : '',
      };
    } else if (type === 'apiKey') {
      auth = {
        type: 'apiKey',
        placement: auth.type === 'apiKey' ? auth.placement : 'header',
        key: auth.type === 'apiKey' ? auth.key : '',
        value: auth.type === 'apiKey' ? auth.value : '',
      };
    } else {
      auth = { type: 'none' };
    }
  }
</script>

<div class="auth">
  <select value={auth.type} onchange={(e) => setType(e.currentTarget.value)} aria-label="Auth type">
    <option value="none">None</option>
    <option value="bearer">Bearer</option>
    <option value="basic">Basic</option>
    <option value="apiKey">API Key</option>
  </select>

  {#if auth.type === 'bearer'}
    <input class="mono" bind:value={auth.token} placeholder="Token" spellcheck="false" />
  {:else if auth.type === 'basic'}
    <input class="mono" bind:value={auth.username} placeholder="Username" spellcheck="false" />
    <input class="mono" type="password" bind:value={auth.password} placeholder="Password" />
  {:else if auth.type === 'apiKey'}
    <select bind:value={auth.placement} aria-label="Placement">
      <option value="header">Header</option>
      <option value="query">Query</option>
    </select>
    <input class="mono" bind:value={auth.key} placeholder="Key name" spellcheck="false" />
    <input class="mono" bind:value={auth.value} placeholder="Value" spellcheck="false" />
  {/if}
</div>

<style>
  .auth {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 420px;
  }
  select,
  input {
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 12px;
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>
