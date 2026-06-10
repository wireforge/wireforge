<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onDestroy } from 'svelte';
  import type { DeviceStart, PollOutcome, GithubAuthStatus, WfError } from './types';

  let {
    open = $bindable(),
    host = $bindable(),
    clientId = $bindable(),
  }: {
    open: boolean;
    host: string;
    clientId: string;
  } = $props();

  let status = $state<GithubAuthStatus | null>(null);
  let device = $state<DeviceStart | null>(null);
  let phase = $state<'idle' | 'waiting' | 'done' | 'error'>('idle');
  let error = $state<string | null>(null);
  let copied = $state<string | null>(null);

  let stopped = false;
  let interval = 5;

  const errMsg = (e: unknown) => (e as WfError)?.message ?? String(e);

  // Load status when the dialog opens; stop any polling when it closes.
  $effect(() => {
    if (open) {
      loadStatus();
    } else {
      reset();
    }
  });

  onDestroy(() => {
    stopped = true;
  });

  function reset() {
    stopped = true;
    device = null;
    phase = 'idle';
    error = null;
  }

  async function loadStatus() {
    try {
      status = await invoke<GithubAuthStatus>('github_auth_status', { host });
    } catch (e) {
      status = null;
      error = errMsg(e);
    }
  }

  async function signIn() {
    error = null;
    if (!clientId.trim()) {
      error = 'Enter a GitHub OAuth client id first.';
      return;
    }
    try {
      device = await invoke<DeviceStart>('github_device_start', { host, clientId });
    } catch (e) {
      error = errMsg(e);
      return;
    }
    stopped = false;
    interval = device.interval || 5;
    phase = 'waiting';
    poll();
  }

  async function poll() {
    if (stopped || !device) return;
    await new Promise((r) => setTimeout(r, interval * 1000));
    if (stopped || !device) return;
    try {
      const out = await invoke<PollOutcome>('github_device_poll', {
        host,
        clientId,
        deviceCode: device.deviceCode,
      });
      if (out.status === 'authorized') {
        phase = 'done';
        device = null;
        await loadStatus();
        return;
      }
      if (out.status === 'denied') {
        error = 'Authorization was denied.';
        phase = 'error';
        return;
      }
      if (out.status === 'slowDown') interval += 5;
      poll();
    } catch (e) {
      error = errMsg(e);
      phase = 'error';
    }
  }

  async function signOut() {
    try {
      await invoke('github_logout', { host });
      await loadStatus();
    } catch (e) {
      error = errMsg(e);
    }
  }

  async function copy(text: string, label: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = label;
      setTimeout(() => (copied = copied === label ? null : copied), 1500);
    } catch {
      // clipboard unavailable
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
  <button class="backdrop" onclick={close} aria-label="Close GitHub dialog"></button>
  <div class="panel" role="dialog" aria-label="GitHub account">
    <header class="head">
      <h2>GitHub</h2>
      <button class="x" onclick={close} aria-label="Close">✕</button>
    </header>

    {#if error}<p class="error">{error}</p>{/if}

    {#if status?.authenticated}
      <p class="ok">
        Signed in as <strong>{status.login ?? '(unknown)'}</strong> on <span class="mono">{status.host}</span>.
      </p>
      <div class="actions">
        <button class="ghost" onclick={signOut}>Sign out</button>
      </div>
    {:else if phase === 'waiting' && device}
      {@const d = device}
      <p class="step">1. Copy this code:</p>
      <div class="code-row">
        <span class="code mono">{d.userCode}</span>
        <button class="ghost" onclick={() => copy(d.userCode, 'code')}>
          {copied === 'code' ? 'Copied' : 'Copy'}
        </button>
      </div>
      <p class="step">2. Open the verification page and enter it:</p>
      <div class="code-row">
        <span class="uri mono">{d.verificationUri}</span>
        <button class="ghost" onclick={() => copy(d.verificationUri, 'uri')}>
          {copied === 'uri' ? 'Copied' : 'Copy link'}
        </button>
      </div>
      <p class="waiting">Waiting for authorization…</p>
    {:else if phase === 'done'}
      <p class="ok">Signed in. You can close this dialog.</p>
    {:else}
      <p class="hint">Sign in to push, pull, and collaborate through GitHub.</p>
      <label class="field">
        <span>Host</span>
        <input class="mono" bind:value={host} placeholder="github.com" aria-label="GitHub host" />
      </label>
      <label class="field">
        <span>OAuth client id</span>
        <input class="mono" bind:value={clientId} placeholder="Iv1.xxxxxxxx" aria-label="OAuth client id" />
      </label>
      <p class="note">
        Device flow uses a public OAuth App client id (no secret). Register an OAuth App for your
        organization and paste its client id here.
      </p>
      <div class="actions">
        <button class="primary" onclick={signIn} disabled={!clientId.trim()}>Sign in with GitHub</button>
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
    top: 14vh;
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
    margin-bottom: 10px;
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
    margin: 0 0 10px;
  }
  .ok {
    color: var(--success);
    font-size: 13px;
    margin: 0 0 12px;
  }
  .hint,
  .note {
    color: var(--text-muted);
    font-size: 12px;
    margin: 0 0 10px;
  }
  .note {
    margin-top: 4px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-bottom: 10px;
    font-size: 12px;
  }
  .field span {
    color: var(--text-muted);
  }
  input {
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 12px;
  }
  .step {
    font-size: 12px;
    margin: 8px 0 4px;
  }
  .code-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .code {
    font-size: 20px;
    letter-spacing: 3px;
    font-weight: 700;
    background: var(--surface-code);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 10px;
  }
  .uri {
    flex: 1;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .waiting {
    color: var(--text-muted);
    font-size: 12px;
    margin-top: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
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
