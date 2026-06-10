<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { RequestFile, WfError } from './types';

  let {
    open = $bindable(),
    oncreated,
  }: {
    open: boolean;
    oncreated: (request: RequestFile) => void;
  } = $props();

  let text = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  const errMsg = (e: unknown) => (e as WfError)?.message ?? String(e);

  async function doImport() {
    if (busy || !text.trim()) return;
    busy = true;
    error = null;
    try {
      const rf = await invoke<RequestFile>('import_curl', { text });
      oncreated(rf);
      text = '';
      open = false;
    } catch (e) {
      error = errMsg(e);
    } finally {
      busy = false;
    }
  }

  function close() {
    open = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    } else if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      doImport();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <button class="backdrop" onclick={close} aria-label="Close cURL import"></button>
  <div class="panel" role="dialog" aria-label="Import cURL">
    <header class="head">
      <h2>Import cURL</h2>
      <button class="x" onclick={close} aria-label="Close">✕</button>
    </header>
    <p class="hint">Paste a cURL command (e.g. from a browser's “Copy as cURL”). It opens as a new request.</p>
    {#if error}<p class="error">{error}</p>{/if}
    <textarea
      class="curl mono"
      bind:value={text}
      placeholder={"curl 'https://api.example.com/users' -H 'Accept: application/json'"}
      spellcheck="false"
    ></textarea>
    <div class="actions">
      <button class="ghost" onclick={close}>Cancel</button>
      <button class="primary" onclick={doImport} disabled={busy || !text.trim()}>
        {busy ? 'Importing…' : 'Import'}
      </button>
    </div>
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
    top: 16vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(560px, calc(100vw - 32px));
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
  .hint {
    color: var(--text-muted);
    font-size: 12px;
    margin: 0 0 8px;
  }
  .error {
    color: var(--danger);
    font-size: 12px;
    margin: 0 0 8px;
  }
  .curl {
    width: 100%;
    box-sizing: border-box;
    height: 160px;
    resize: vertical;
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 10px;
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 10px;
  }
  .ghost {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 12px;
    cursor: pointer;
    font-size: 12px;
  }
  .primary {
    background: var(--accent);
    color: #fff;
    border: 1px solid var(--accent);
    border-radius: 6px;
    padding: 6px 14px;
    cursor: pointer;
    font-size: 12px;
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
