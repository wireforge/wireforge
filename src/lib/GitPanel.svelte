<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { RepoStatus, WfError } from './types';

  let {
    open = $bindable(),
    root,
    status,
    oncommitted,
  }: {
    open: boolean;
    root: string;
    status: RepoStatus | null;
    oncommitted: () => void;
  } = $props();

  const LETTER: Record<string, string> = {
    untracked: 'U',
    modified: 'M',
    added: 'A',
    renamed: 'R',
    deleted: 'D',
    conflicted: '!',
  };

  let checked = $state<Record<string, boolean>>({});
  let selectedFile = $state<string | null>(null);
  let diffText = $state('');
  let message = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);

  const files = $derived(status?.files ?? []);
  const selectedPaths = $derived(files.map((f) => f.path).filter((p) => checked[p]));

  const errMsg = (e: unknown) => (e as WfError)?.message ?? String(e);

  // Initialize selection and the diff view whenever the dialog opens.
  $effect(() => {
    if (!open) return;
    const next: Record<string, boolean> = {};
    for (const f of files) next[f.path] = checked[f.path] ?? true;
    checked = next;
    if (!selectedFile && files.length) showDiff(files[0].path);
    else showDiff(selectedFile);
  });

  async function showDiff(path: string | null) {
    selectedFile = path;
    try {
      diffText = await invoke<string>('git_diff', { root, path: path ?? undefined });
    } catch (e) {
      diffText = '';
      error = errMsg(e);
    }
  }

  function lineClass(line: string): string {
    if (line.startsWith('@@')) return 'hunk';
    if (line.startsWith('+')) return 'add';
    if (line.startsWith('-')) return 'del';
    if (line.startsWith('diff ') || line.startsWith('index ') || line.startsWith('+++') || line.startsWith('---'))
      return 'meta';
    return '';
  }

  async function doCommit() {
    if (busy || !message.trim() || selectedPaths.length === 0) return;
    busy = true;
    error = null;
    try {
      await invoke('git_commit', { root, message: message.trim(), paths: selectedPaths });
      message = '';
      oncommitted();
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
      doCommit();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <button class="backdrop" onclick={close} aria-label="Close commit dialog"></button>
  <div class="panel" role="dialog" aria-label="Commit changes">
    <header class="head">
      <h2>Commit</h2>
      <span class="branch mono">
        ⎇ {status?.branch ?? 'no commits'}
        {#if status?.ahead}↑{status.ahead}{/if}
        {#if status?.behind}↓{status.behind}{/if}
      </span>
      <button class="x" onclick={close} aria-label="Close">✕</button>
    </header>

    {#if error}<p class="error">{error}</p>{/if}

    <div class="cols">
      <aside class="files">
        {#if files.length}
          <button class="all" onclick={() => showDiff(null)} class:active={selectedFile === null}>
            All changes ({files.length})
          </button>
          {#each files as f (f.path)}
            <div class="file" class:active={selectedFile === f.path}>
              <input type="checkbox" bind:checked={checked[f.path]} aria-label="Stage {f.path}" />
              <button class="fname" onclick={() => showDiff(f.path)} title={f.path}>
                <span class="g g-{f.status}">{LETTER[f.status] ?? '?'}</span>
                <span class="p">{f.path}</span>
              </button>
            </div>
          {/each}
        {:else}
          <p class="muted">No changes to commit.</p>
        {/if}
      </aside>

      <section class="diff">
        {#if diffText}
          <pre class="mono">{#each diffText.split('\n') as line, i (i)}<span class="dl {lineClass(line)}">{line}
</span>{/each}</pre>
        {:else}
          <p class="muted">Select a file to see its diff.</p>
        {/if}
      </section>
    </div>

    <footer class="foot">
      <textarea
        class="msg"
        placeholder="Commit message…"
        bind:value={message}
        rows="2"
        aria-label="Commit message"
      ></textarea>
      <button class="primary" onclick={doCommit} disabled={busy || !message.trim() || selectedPaths.length === 0}>
        {busy ? 'Committing…' : `Commit ${selectedPaths.length} file${selectedPaths.length === 1 ? '' : 's'}`}
      </button>
    </footer>
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
    width: min(840px, calc(100vw - 32px));
    height: 80vh;
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
    gap: 10px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 14px;
  }
  .branch {
    color: var(--text-muted);
    font-size: 11px;
  }
  .x {
    margin-left: auto;
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
  .cols {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .files {
    flex: 0 0 280px;
    border-right: 1px solid var(--border);
    overflow: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .all {
    text-align: left;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--text-muted);
    padding: 4px 8px;
    cursor: pointer;
    font-size: 12px;
  }
  .all.active,
  .file.active {
    background: var(--surface-code);
    border-radius: 6px;
  }
  .file {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 6px;
  }
  .fname {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    color: var(--text);
    cursor: pointer;
    font-size: 12px;
    text-align: left;
  }
  .fname .p {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
  }
  .g {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    flex: 0 0 auto;
  }
  .g-modified,
  .g-renamed {
    color: var(--accent);
  }
  .g-added,
  .g-untracked {
    color: var(--success);
  }
  .g-deleted,
  .g-conflicted {
    color: var(--danger);
  }
  .diff {
    flex: 1;
    overflow: auto;
    min-width: 0;
    background: var(--surface-code);
  }
  .diff pre {
    margin: 0;
    padding: 8px 10px;
    font-size: 11px;
    line-height: 1.5;
  }
  .dl {
    display: inline;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .dl.add {
    color: var(--success);
  }
  .dl.del {
    color: var(--danger);
  }
  .dl.hunk {
    color: var(--accent);
  }
  .dl.meta {
    color: var(--text-muted);
  }
  .foot {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    padding: 10px 16px;
    border-top: 1px solid var(--border);
  }
  .msg {
    flex: 1;
    resize: vertical;
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 12px;
    font-family: inherit;
  }
  .muted {
    color: var(--text-muted);
    font-size: 12px;
    padding: 10px;
  }
  .primary {
    background: var(--accent);
    color: #fff;
    border: 1px solid var(--accent);
    border-radius: 6px;
    padding: 8px 14px;
    cursor: pointer;
    font-size: 12px;
    white-space: nowrap;
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
