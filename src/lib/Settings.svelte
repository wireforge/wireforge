<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { ThemeMode } from './theme';

  let {
    open = $bindable(),
    theme = $bindable(),
    density = $bindable(),
    ghHost = $bindable(),
    ghClientId = $bindable(),
    onthemeeditor,
  }: {
    open: boolean;
    theme: ThemeMode;
    density: 'comfortable' | 'compact';
    ghHost: string;
    ghClientId: string;
    onthemeeditor: () => void;
  } = $props();

  let version = $state('');

  $effect(() => {
    if (open && !version) {
      invoke<string>('app_info')
        .then((v) => (version = v))
        .catch(() => (version = ''));
    }
  });

  function close() {
    open = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (open && e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <button class="backdrop" onclick={close} aria-label="Close settings"></button>
  <div class="panel" role="dialog" aria-label="Settings">
    <header class="head">
      <h2>Settings</h2>
      <button class="x" onclick={close} aria-label="Close">✕</button>
    </header>

    <section class="group">
      <h3>Appearance</h3>
      <label class="row">
        <span>Theme</span>
        <select bind:value={theme} aria-label="Theme">
          <option value="dark">Dark</option>
          <option value="light">Light</option>
          <option value="system">System</option>
        </select>
      </label>
      <label class="row">
        <span>Density</span>
        <select bind:value={density} aria-label="Density">
          <option value="comfortable">Comfortable</option>
          <option value="compact">Compact</option>
        </select>
      </label>
      <div class="row">
        <span>Custom themes</span>
        <button class="link" onclick={onthemeeditor}>Open theme editor…</button>
      </div>
    </section>

    <section class="group">
      <h3>GitHub</h3>
      <label class="row">
        <span>Host</span>
        <input class="mono" bind:value={ghHost} placeholder="github.com" aria-label="GitHub host" />
      </label>
      <label class="row">
        <span>OAuth client id</span>
        <input class="mono" bind:value={ghClientId} placeholder="Iv1.xxxxxxxx" aria-label="OAuth client id" />
      </label>
      <p class="note">Used for the device-flow sign-in. Sign in from the GitHub button in the top bar.</p>
    </section>

    <section class="group">
      <h3>About</h3>
      <p class="note">{version || 'wireforge'}</p>
    </section>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 45%);
    border: none;
    cursor: default;
    z-index: 40;
  }
  .panel {
    position: fixed;
    top: 12vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(460px, calc(100vw - 32px));
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px;
    z-index: 41;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  h2 {
    margin: 0;
    font-size: 14px;
  }
  .x {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 13px;
  }
  .group {
    border-top: 1px solid var(--border);
    padding: 12px 0;
  }
  .group:last-child {
    padding-bottom: 0;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 8px;
    font-size: 12px;
  }
  .row span {
    color: var(--text-muted);
    flex: 0 0 auto;
  }
  select,
  input {
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 8px;
    font-size: 12px;
    min-width: 0;
  }
  input {
    flex: 1;
  }
  .note {
    color: var(--text-muted);
    font-size: 11px;
    margin: 4px 0 0;
  }
  .link {
    background: transparent;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: 12px;
    padding: 0;
  }
</style>
