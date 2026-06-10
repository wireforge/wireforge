<script lang="ts">
  import {
    applyCustomTheme,
    currentTokens,
    newThemeId,
    parseTheme,
    TOKEN_VARS,
    type Theme,
  } from './theme';

  let {
    open = $bindable(),
    customThemes = $bindable(),
    activeThemeId = $bindable(),
    baseMode,
    reapply,
  }: {
    open: boolean;
    customThemes: Theme[];
    activeThemeId: string;
    baseMode: 'dark' | 'light';
    reapply: () => void;
  } = $props();

  const TOKEN_KEYS = Object.keys(TOKEN_VARS);

  let editing = $state<Theme | null>(null);
  let importText = $state('');
  let error = $state<string | null>(null);
  let copied = $state(false);

  function startNew() {
    error = null;
    editing = {
      format: 'wireforge.theme',
      version: 1,
      id: newThemeId(),
      name: 'Custom theme',
      base: baseMode,
      tokens: currentTokens(),
    };
    applyCustomTheme(editing);
  }

  function startEdit(t: Theme) {
    error = null;
    editing = structuredClone($state.snapshot(t)) as Theme;
    applyCustomTheme(editing);
  }

  function live() {
    if (editing) applyCustomTheme(editing);
  }

  function save() {
    if (!editing) return;
    const name = editing.name.trim() || 'Custom theme';
    const tokens: Record<string, string> = {};
    for (const [k, v] of Object.entries(editing.tokens)) tokens[k] = String(v).toUpperCase();
    const theme: Theme = { ...editing, name, tokens };

    const idx = customThemes.findIndex((t) => t.id === theme.id);
    if (idx >= 0) customThemes[idx] = theme;
    else customThemes = [...customThemes, theme];
    activeThemeId = theme.id; // applied by the parent effect
    editing = null;
  }

  function cancel() {
    editing = null;
    reapply();
  }

  function apply(t: Theme) {
    activeThemeId = t.id;
  }

  function useBuiltIn() {
    activeThemeId = '';
  }

  function remove(t: Theme) {
    customThemes = customThemes.filter((x) => x.id !== t.id);
    if (activeThemeId === t.id) activeThemeId = '';
  }

  async function exportTheme(t: Theme) {
    try {
      await navigator.clipboard.writeText(JSON.stringify(t, null, 2));
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      // clipboard unavailable
    }
  }

  function doImport() {
    error = null;
    try {
      const t = parseTheme(importText);
      customThemes = [...customThemes.filter((x) => x.id !== t.id), t];
      activeThemeId = t.id;
      importText = '';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function close() {
    if (editing) cancel();
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
  <button class="backdrop" onclick={close} aria-label="Close theme editor"></button>
  <div class="panel" role="dialog" aria-label="Theme editor">
    <header class="head">
      <h2>Theme editor</h2>
      <button class="x" onclick={close} aria-label="Close">✕</button>
    </header>

    {#if error}<p class="error">{error}</p>{/if}

    {#if editing}
      <div class="edit-head">
        <input class="name" bind:value={editing.name} aria-label="Theme name" placeholder="Theme name" />
        <label class="base">
          base
          <select bind:value={editing.base} onchange={live} aria-label="Base">
            <option value="dark">dark</option>
            <option value="light">light</option>
          </select>
        </label>
      </div>
      <div class="tokens">
        {#each TOKEN_KEYS as key (key)}
          <label class="token">
            <input type="color" bind:value={editing.tokens[key]} oninput={live} aria-label={key} />
            <span class="tname">{key}</span>
            <code>{editing.tokens[key]}</code>
          </label>
        {/each}
      </div>
      <div class="actions">
        <button class="ghost" onclick={cancel}>Cancel</button>
        <button class="primary" onclick={save}>Save &amp; apply</button>
      </div>
    {:else}
      <section class="group">
        <div class="group-head">
          <h3>Custom themes</h3>
          <button class="ghost sm" onclick={startNew}>Duplicate current</button>
        </div>
        {#if customThemes.length}
          <ul class="list">
            {#each customThemes as t (t.id)}
              <li class="item" class:active={activeThemeId === t.id}>
                <span class="dot" style="background: {t.tokens.accent}"></span>
                <span class="iname">{t.name}</span>
                <span class="ibase">{t.base}</span>
                <span class="ops">
                  <button class="link" onclick={() => apply(t)}>{activeThemeId === t.id ? 'Active' : 'Apply'}</button>
                  <button class="link" onclick={() => startEdit(t)}>Edit</button>
                  <button class="link" onclick={() => exportTheme(t)}>Copy JSON</button>
                  <button class="link danger" onclick={() => remove(t)}>Delete</button>
                </span>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="note">No custom themes yet. Duplicate the current appearance to start.</p>
        {/if}
        <button class="ghost sm" onclick={useBuiltIn} disabled={!activeThemeId}>Use built-in theme</button>
        {#if copied}<span class="copied">JSON copied</span>{/if}
      </section>

      <section class="group">
        <h3>Import</h3>
        <textarea class="import mono" bind:value={importText} placeholder="Paste a wireforge.theme JSON…" rows="3"></textarea>
        <button class="ghost sm" onclick={doImport} disabled={!importText.trim()}>Import theme</button>
      </section>
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
    z-index: 42;
  }
  .panel {
    position: fixed;
    top: 9vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(520px, calc(100vw - 32px));
    max-height: 82vh;
    overflow: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px;
    z-index: 43;
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
  .error {
    color: var(--danger);
    font-size: 12px;
    margin: 0 0 8px;
  }
  .group {
    border-top: 1px solid var(--border);
    padding: 12px 0;
  }
  .group-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  h3 {
    margin: 0;
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .list {
    list-style: none;
    margin: 0 0 8px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    font-size: 12px;
  }
  .item.active {
    border-color: var(--accent);
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    flex: 0 0 auto;
    border: 1px solid var(--border);
  }
  .iname {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ibase {
    color: var(--text-muted);
    font-size: 10px;
  }
  .ops {
    display: flex;
    gap: 8px;
  }
  .link {
    background: transparent;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: 11px;
    padding: 0;
  }
  .link.danger {
    color: var(--danger);
  }
  .edit-head {
    display: flex;
    gap: 8px;
    margin-bottom: 10px;
  }
  .name {
    flex: 1;
  }
  .base {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--text-muted);
  }
  .tokens {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px 12px;
    margin-bottom: 12px;
  }
  .token {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
  }
  .token input[type='color'] {
    width: 26px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: none;
  }
  .tname {
    color: var(--text-muted);
    flex: 1;
  }
  .token code {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  input,
  select,
  textarea {
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 8px;
    font-size: 12px;
    min-width: 0;
  }
  .import {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    margin-bottom: 8px;
    font-size: 11px;
  }
  .ghost {
    background: transparent;
    color: var(--text-muted);
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
  .sm {
    padding: 4px 10px;
  }
  .primary {
    background: var(--accent);
    color: #fff;
    border: 1px solid var(--accent);
    border-radius: 6px;
    padding: 5px 14px;
    cursor: pointer;
    font-size: 12px;
  }
  .note {
    color: var(--text-muted);
    font-size: 12px;
    margin: 0 0 8px;
  }
  .copied {
    color: var(--success);
    font-size: 11px;
    margin-left: 8px;
  }
</style>
