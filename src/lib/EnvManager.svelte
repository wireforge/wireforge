<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { Environment, EnvSummary, EnvValue, SecretStatus, WfError } from './types';

  let {
    open = $bindable(),
    root,
    environments,
    focusSecrets = false,
    onchanged,
  }: {
    open: boolean;
    root: string;
    environments: EnvSummary[];
    focusSecrets?: boolean;
    onchanged: () => void;
  } = $props();

  interface Row {
    key: string;
    value: string;
    secret: boolean;
  }

  let selected = $state<string | null>(null);
  let rows = $state<Row[]>([]);
  let secrets = $state<SecretStatus[]>([]);
  let secretInputs = $state<Record<string, string>>({});
  let newEnvName = $state('');
  let error = $state<string | null>(null);
  let dirty = $state(false);

  const errMsg = (e: unknown) => (e as WfError)?.message ?? String(e);

  // When the dialog opens, pick an environment to edit.
  $effect(() => {
    if (open && selected === null && environments.length) {
      selectEnv(environments[0].slug);
    }
  });

  function rowsFromEnv(env: Environment): Row[] {
    return Object.entries(env.values).map(([key, v]) => ({
      key,
      value: typeof v === 'string' ? v : '',
      secret: typeof v !== 'string',
    }));
  }

  async function selectEnv(slug: string) {
    selected = slug;
    error = null;
    dirty = false;
    try {
      const env = await invoke<Environment>('load_environment', { root, slug });
      rows = rowsFromEnv(env);
    } catch (e) {
      error = errMsg(e);
      rows = [];
    }
    await loadSecrets();
  }

  async function loadSecrets() {
    try {
      secrets = await invoke<SecretStatus[]>('secret_status', { root, environment: selected });
    } catch {
      secrets = [];
    }
  }

  function addRow() {
    rows = [...rows, { key: '', value: '', secret: false }];
    dirty = true;
  }

  function removeRow(i: number) {
    rows = rows.filter((_, idx) => idx !== i);
    dirty = true;
  }

  function markSecret(i: number, secret: boolean) {
    rows[i].secret = secret;
    if (secret) rows[i].value = ''; // a secret value never lives in the file
    dirty = true;
  }

  async function saveValues() {
    if (!selected) return;
    const values: Record<string, EnvValue> = {};
    for (const r of rows) {
      const key = r.key.trim();
      if (!key) continue;
      values[key] = r.secret ? { secret: true } : r.value;
    }
    try {
      const current = await invoke<Environment>('load_environment', { root, slug: selected });
      current.values = values;
      await invoke('save_environment', { root, slug: selected, environment: current });
      dirty = false;
      onchanged();
      await loadSecrets();
    } catch (e) {
      error = errMsg(e);
    }
  }

  async function createEnv() {
    const name = newEnvName.trim();
    if (!name) return;
    try {
      const slug = await invoke<string>('create_environment', { root, name });
      newEnvName = '';
      onchanged();
      await selectEnv(slug);
    } catch (e) {
      error = errMsg(e);
    }
  }

  async function setSecret(s: SecretStatus) {
    const value = secretInputs[s.name] ?? '';
    if (!value) return;
    try {
      await invoke('set_secret', { root, environment: s.environment, name: s.name, value });
      secretInputs[s.name] = ''; // never keep the value around
      await loadSecrets();
    } catch (e) {
      error = errMsg(e);
    }
  }

  async function clearSecret(s: SecretStatus) {
    try {
      await invoke('delete_secret', { root, environment: s.environment, name: s.name });
      await loadSecrets();
    } catch (e) {
      error = errMsg(e);
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
  <button class="backdrop" onclick={close} aria-label="Close environments dialog"></button>
  <div class="panel" role="dialog" aria-label="Environments and secrets">
    <header class="head">
      <h2>Environments &amp; Secrets</h2>
      <button class="x" onclick={close} aria-label="Close">✕</button>
    </header>

    {#if error}<p class="error">{error}</p>{/if}

    <div class="cols">
      <aside class="list">
        {#each environments as env (env.slug)}
          <button class="env" class:active={selected === env.slug} onclick={() => selectEnv(env.slug)}>
            {env.name}
            {#if env.hasLocal}<span class="local" title="Has a local override">local</span>{/if}
          </button>
        {/each}
        {#if !environments.length}
          <p class="muted">No environments yet.</p>
        {/if}
        <div class="newrow">
          <input placeholder="New environment…" bind:value={newEnvName} aria-label="New environment name" />
          <button class="ghost" onclick={createEnv}>Add</button>
        </div>
      </aside>

      <section class="detail">
        {#if selected}
          <div class="block">
            <div class="block-head">
              <h3>Values</h3>
              <button class="primary sm" onclick={saveValues} disabled={!dirty}>
                {dirty ? 'Save' : 'Saved'}
              </button>
            </div>
            <div class="rows">
              {#each rows as row, i (i)}
                <div class="row">
                  <input class="k" placeholder="key" bind:value={row.key} oninput={() => (dirty = true)} />
                  {#if row.secret}
                    <span class="secret-cell">resolved from keychain ↓</span>
                  {:else}
                    <input class="v" placeholder="value" bind:value={row.value} oninput={() => (dirty = true)} />
                  {/if}
                  <label class="chk" title="Mark as secret (value resolved from keychain, never stored in the file)">
                    <input type="checkbox" checked={row.secret} onchange={(e) => markSecret(i, e.currentTarget.checked)} />
                    secret
                  </label>
                  <button class="del" onclick={() => removeRow(i)} aria-label="Remove value">✕</button>
                </div>
              {/each}
              <button class="ghost sm" onclick={addRow}>+ Add value</button>
            </div>
          </div>

          <div class="block" class:flash={focusSecrets}>
            <div class="block-head">
              <h3>Secrets</h3>
              <span class="muted">resolved at send time · never written to files</span>
            </div>
            {#if secrets.length}
              <div class="rows">
                {#each secrets as s (s.name)}
                  <div class="secret">
                    <div class="secret-meta">
                      <span class="name">{s.name}</span>
                      {#if s.required}<span class="badge req">required</span>{/if}
                      <span class="badge {s.set ? 'ok' : 'miss'}">{s.set ? 'set' : 'missing'}</span>
                      <span class="seg">{s.environment}</span>
                    </div>
                    {#if s.description}<p class="desc">{s.description}</p>{/if}
                    <div class="secret-set">
                      <input
                        type="password"
                        placeholder={s.set ? 'Replace value…' : 'Enter value…'}
                        bind:value={secretInputs[s.name]}
                        aria-label="Secret value for {s.name}"
                      />
                      <button class="primary sm" onclick={() => setSecret(s)}>Set</button>
                      {#if s.set}<button class="ghost sm" onclick={() => clearSecret(s)}>Clear</button>{/if}
                      {#if s.docUrl}<a class="doc" href={s.docUrl} target="_blank" rel="noreferrer">docs</a>{/if}
                    </div>
                  </div>
                {/each}
              </div>
            {:else}
              <p class="muted">
                No declared secrets. Add a <code>secrets.manifest.json</code> to your workspace, or mark a value
                as secret above.
              </p>
            {/if}
          </div>
        {:else}
          <p class="muted">Select or create an environment.</p>
        {/if}
      </section>
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
    top: 8vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(760px, calc(100vw - 32px));
    max-height: 80vh;
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
  h3 {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
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
  .cols {
    display: flex;
    min-height: 0;
    flex: 1;
  }
  .list {
    flex: 0 0 200px;
    border-right: 1px solid var(--border);
    padding: 10px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .env {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    text-align: left;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--text);
    padding: 6px 8px;
    cursor: pointer;
    font-size: 12px;
  }
  .env:hover {
    background: var(--surface-code);
  }
  .env.active {
    background: var(--surface-code);
    border-color: var(--border);
  }
  .local {
    font-size: 9px;
    color: var(--accent);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0 4px;
  }
  .newrow {
    display: flex;
    gap: 4px;
    margin-top: 8px;
  }
  .detail {
    flex: 1;
    padding: 14px 16px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .block-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .block.flash {
    outline: 1px solid var(--accent);
    outline-offset: 6px;
    border-radius: 4px;
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .row .k {
    flex: 0 0 34%;
  }
  .row .v {
    flex: 1;
  }
  .secret-cell {
    flex: 1;
    font-size: 11px;
    color: var(--text-muted);
    font-style: italic;
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .del {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 11px;
  }
  .secret {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 10px;
  }
  .secret-meta {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .secret-meta .name {
    font-weight: 600;
    font-size: 12px;
  }
  .badge {
    font-size: 9px;
    border-radius: 4px;
    padding: 0 5px;
    border: 1px solid var(--border);
  }
  .badge.req {
    color: var(--text-muted);
  }
  .badge.ok {
    color: var(--success);
    border-color: var(--success);
  }
  .badge.miss {
    color: var(--danger);
    border-color: var(--danger);
  }
  .seg {
    margin-left: auto;
    font-size: 10px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
  .desc {
    margin: 4px 0;
    font-size: 11px;
    color: var(--text-muted);
  }
  .secret-set {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
  }
  .secret-set input {
    flex: 1;
  }
  .doc {
    font-size: 11px;
    color: var(--accent);
  }
  input {
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 12px;
    min-width: 0;
  }
  input[type='checkbox'] {
    min-width: auto;
  }
  button {
    border-radius: 6px;
    font-size: 12px;
    cursor: pointer;
  }
  .sm {
    padding: 3px 10px;
  }
  .ghost {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
    padding: 4px 10px;
  }
  .primary {
    background: var(--accent);
    color: #fff;
    border: 1px solid var(--accent);
    padding: 4px 12px;
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
