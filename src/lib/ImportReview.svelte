<script lang="ts">
  import type { ImportPreview, ImportResult } from './types';

  let {
    open = $bindable(),
    preview,
    result,
    error,
    busy,
    onconfirm,
  }: {
    open: boolean;
    preview: ImportPreview | null;
    result: ImportResult | null;
    error: string | null;
    busy: boolean;
    onconfirm: () => void;
  } = $props();

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
  <button class="backdrop" onclick={close} aria-label="Close import dialog"></button>
  <div class="panel" role="dialog" aria-label="Import review">
    <h2>Import Postman {preview?.kind === 'environment' ? 'environment' : 'collection'}</h2>

    {#if error}
      <p class="error">{error}</p>
    {:else if preview}
      <p class="meta">
        <strong>{preview.name}</strong>
        {#if preview.kind === 'collection'}
          — {preview.requests} request{preview.requests === 1 ? '' : 's'}, {preview.folders}
          folder{preview.folders === 1 ? '' : 's'}{preview.variables
            ? `, ${preview.variables} variable${preview.variables === 1 ? '' : 's'}`
            : ''}
        {:else}
          — {preview.variables} variable{preview.variables === 1 ? '' : 's'}
        {/if}
      </p>

      {#if preview.warnings.length}
        <p class="warn-head">
          {preview.warnings.length} warning{preview.warnings.length === 1 ? '' : 's'} — these fields
          are reported, not silently dropped:
        </p>
        <ul class="warnings">
          {#each preview.warnings as w, i (i)}
            <li>
              {#if w.path}<span class="path">{w.path}</span>{/if}
              <span>{w.message}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="ok">Everything maps cleanly — no warnings.</p>
      {/if}

      {#if result}
        <p class="ok done">
          Imported “{result.name}”{result.environmentFile ? ` → ${result.environmentFile}` : ''}.
        </p>
      {/if}
    {/if}

    <div class="actions">
      {#if result || error}
        <button class="primary" onclick={close}>Close</button>
      {:else}
        <button class="ghost" onclick={close}>Cancel</button>
        <button class="primary" onclick={onconfirm} disabled={busy}>
          {busy ? 'Importing…' : 'Import'}
        </button>
      {/if}
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
    top: 12vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(560px, calc(100vw - 32px));
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px;
    z-index: 41;
  }
  h2 {
    margin: 0 0 10px;
    font-size: 14px;
  }
  .meta {
    margin: 0 0 10px;
    font-size: 13px;
    color: var(--text);
  }
  .warn-head {
    margin: 0 0 6px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .warnings {
    list-style: none;
    margin: 0 0 10px;
    padding: 8px;
    overflow: auto;
    flex: 1;
    min-height: 0;
    background: var(--surface-code);
    border: 1px solid var(--border);
    border-radius: 8px;
    font-size: 12px;
  }
  .warnings li {
    display: flex;
    gap: 8px;
    padding: 3px 2px;
  }
  .path {
    color: var(--accent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 40%;
    flex: 0 0 auto;
  }
  .ok {
    margin: 0 0 10px;
    font-size: 12px;
    color: var(--success);
  }
  .done {
    font-weight: 600;
  }
  .error {
    margin: 0 0 10px;
    font-size: 12px;
    color: var(--danger);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .actions button {
    border-radius: 6px;
    padding: 6px 14px;
    font-size: 12px;
    cursor: pointer;
  }
  .ghost {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
  }
  .primary {
    background: var(--accent);
    color: #fff;
    border: 1px solid var(--accent);
  }
  .primary:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
