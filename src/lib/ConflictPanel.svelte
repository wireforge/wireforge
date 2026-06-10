<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { RepoStatus, ConflictSides, WfError } from './types';

  let {
    open = $bindable(),
    root,
    status,
    onchanged,
  }: {
    open: boolean;
    root: string;
    status: RepoStatus | null;
    onchanged: () => void;
  } = $props();

  let selected = $state<string | null>(null);
  let sides = $state<ConflictSides | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  const conflicts = $derived((status?.files ?? []).filter((f) => f.status === 'conflicted'));
  const errMsg = (e: unknown) => (e as WfError)?.message ?? String(e);

  // Keep a valid selection as the conflict set changes; load its sides.
  $effect(() => {
    if (!open) return;
    const paths = conflicts.map((c) => c.path);
    if (selected && paths.includes(selected)) {
      loadSides(selected);
    } else if (paths.length) {
      loadSides(paths[0]);
    } else {
      selected = null;
      sides = null;
    }
  });

  async function loadSides(path: string) {
    selected = path;
    try {
      sides = await invoke<ConflictSides>('git_conflict_sides', { root, path });
    } catch (e) {
      sides = null;
      error = errMsg(e);
    }
  }

  async function resolve(choice: 'mine' | 'theirs') {
    if (!selected || busy) return;
    busy = true;
    error = null;
    try {
      await invoke('git_resolve_conflict', { root, path: selected, choice });
      selected = null; // force re-pick of the next conflict
      onchanged();
    } catch (e) {
      error = errMsg(e);
    } finally {
      busy = false;
    }
  }

  async function markResolved() {
    if (!selected || busy) return;
    busy = true;
    error = null;
    try {
      await invoke('git_mark_resolved', { root, path: selected });
      selected = null;
      onchanged();
    } catch (e) {
      error = errMsg(e);
    } finally {
      busy = false;
    }
  }

  async function finishMerge() {
    if (busy) return;
    busy = true;
    error = null;
    try {
      await invoke('git_commit', { root, message: "Merge branch from origin", paths: [] });
      onchanged();
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
    if (open && e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <button class="backdrop" onclick={close} aria-label="Close conflicts dialog"></button>
  <div class="panel" role="dialog" aria-label="Resolve conflicts">
    <header class="head">
      <h2>Resolve conflicts</h2>
      <button class="x" onclick={close} aria-label="Close">✕</button>
    </header>

    {#if error}<p class="error">{error}</p>{/if}

    {#if conflicts.length === 0}
      <div class="done">
        <p class="ok">All conflicts resolved.</p>
        <p class="muted">The losing side of each file was kept beside it as a <code>.conflict</code> backup.</p>
        <button class="primary" onclick={finishMerge} disabled={busy}>
          {busy ? 'Committing…' : 'Finish merge (commit)'}
        </button>
      </div>
    {:else}
      <div class="cols">
        <aside class="list">
          <p class="muted count">{conflicts.length} conflicted</p>
          {#each conflicts as c (c.path)}
            <button class="file" class:active={selected === c.path} onclick={() => loadSides(c.path)} title={c.path}>
              <span class="g">!</span>
              <span class="p">{c.path}</span>
            </button>
          {/each}
        </aside>

        <section class="detail">
          {#if selected}
            <div class="actions">
              <button class="ghost" onclick={() => resolve('mine')} disabled={busy}>Keep mine</button>
              <button class="ghost" onclick={() => resolve('theirs')} disabled={busy}>Keep theirs</button>
              <button class="ghost" onclick={markResolved} disabled={busy} title="Use the file as it is on disk (after editing it yourself)">
                Mark resolved
              </button>
            </div>
            <div class="sides">
              <div class="sidecol">
                <h3>Mine (ours)</h3>
                <pre class="mono">{sides?.ours ?? '(deleted)'}</pre>
              </div>
              <div class="sidecol">
                <h3>Theirs</h3>
                <pre class="mono">{sides?.theirs ?? '(deleted)'}</pre>
              </div>
            </div>
          {:else}
            <p class="muted">Select a conflicted file.</p>
          {/if}
        </section>
      </div>
    {/if}
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
    top: 7vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(860px, calc(100vw - 32px));
    height: 78vh;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    z-index: 41;
    overflow: hidden;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
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
  .error {
    margin: 0;
    padding: 8px 16px;
    color: var(--danger);
    font-size: 12px;
  }
  .done {
    padding: 24px 16px;
    text-align: center;
  }
  .ok {
    color: var(--success);
    font-size: 14px;
    margin: 0 0 6px;
  }
  .cols {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .list {
    flex: 0 0 240px;
    border-right: 1px solid var(--border);
    overflow: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .count {
    margin: 0 0 6px;
  }
  .file {
    display: flex;
    align-items: center;
    gap: 6px;
    text-align: left;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--text);
    padding: 5px 8px;
    cursor: pointer;
    font-size: 12px;
  }
  .file:hover {
    background: var(--surface-code);
  }
  .file.active {
    background: var(--surface-code);
    border-color: var(--border);
  }
  .file .g {
    color: var(--danger);
    font-weight: 700;
    font-family: var(--font-mono);
    flex: 0 0 auto;
  }
  .file .p {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
  }
  .detail {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 12px 16px;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-bottom: 10px;
  }
  .sides {
    display: flex;
    gap: 10px;
    flex: 1;
    min-height: 0;
  }
  .sidecol {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .sidecol h3 {
    margin: 0 0 4px;
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .sidecol pre {
    flex: 1;
    margin: 0;
    overflow: auto;
    background: var(--surface-code);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 10px;
    font-size: 11px;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .ghost {
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 12px;
    cursor: pointer;
    font-size: 12px;
  }
  .ghost:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .primary {
    background: var(--accent);
    color: #fff;
    border: 1px solid var(--accent);
    border-radius: 6px;
    padding: 8px 16px;
    cursor: pointer;
    font-size: 13px;
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .muted {
    color: var(--text-muted);
    font-size: 12px;
  }
  code {
    font-family: var(--font-mono);
    font-size: 11px;
  }
</style>
