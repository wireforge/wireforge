<script lang="ts">
  import type { UnifiedRequest } from './types';
  import KeyValueEditor from './KeyValueEditor.svelte';
  import AuthEditor from './AuthEditor.svelte';

  let {
    request = $bindable(),
    sending,
    onsend,
  }: { request: UnifiedRequest; sending: boolean; onsend: () => void } = $props();

  const methods = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];

  let tab = $state<'params' | 'headers' | 'body' | 'auth'>('params');

  function setBodyMode(mode: string) {
    if (mode === request.body.mode) return;
    if (mode === 'json') {
      request.body = { mode: 'json', text: request.body.mode === 'json' ? request.body.text : '{\n  \n}' };
    } else if (mode === 'raw') {
      request.body = {
        mode: 'raw',
        contentType: 'text/plain',
        text: request.body.mode === 'raw' ? request.body.text : '',
      };
    } else if (mode === 'formUrlEncoded') {
      request.body = {
        mode: 'formUrlEncoded',
        fields: request.body.mode === 'formUrlEncoded' ? request.body.fields : [],
      };
    } else if (mode === 'multipart' || mode === 'graphql') {
      // Imported-only modes; not editable yet, so never switch into them.
      return;
    } else {
      request.body = { mode: 'none' };
    }
  }

  // The active editor's URL input focuses on the app-level "focus URL" command.
  let urlEl = $state<HTMLInputElement>();
  $effect(() => {
    const handler = () => urlEl?.focus();
    window.addEventListener('wf:focus-url', handler);
    return () => window.removeEventListener('wf:focus-url', handler);
  });
</script>

<div class="editor">
  <div class="bar">
    <select class="method" bind:value={request.method} aria-label="Method">
      {#each methods as m}
        <option value={m}>{m}</option>
      {/each}
    </select>
    <input
      class="url mono"
      bind:this={urlEl}
      bind:value={request.url}
      placeholder="https://api.example.com/v1/..."
      spellcheck="false"
      autocomplete="off"
      aria-label="URL"
    />
    <button class="send" onclick={onsend} disabled={sending}>
      {sending ? 'Sending…' : 'Send'}
    </button>
  </div>

  <div class="tabs">
    <button class:active={tab === 'params'} onclick={() => (tab = 'params')}>
      Params{request.params.length ? ` (${request.params.length})` : ''}
    </button>
    <button class:active={tab === 'headers'} onclick={() => (tab = 'headers')}>
      Headers{request.headers.length ? ` (${request.headers.length})` : ''}
    </button>
    <button class:active={tab === 'body'} onclick={() => (tab = 'body')}>Body</button>
    <button class:active={tab === 'auth'} onclick={() => (tab = 'auth')}>Auth</button>
  </div>

  <div class="tab-body">
    {#if tab === 'params'}
      <KeyValueEditor bind:items={request.params} />
    {:else if tab === 'headers'}
      <KeyValueEditor bind:items={request.headers} />
    {:else if tab === 'body'}
      <div class="body-mode">
        <select value={request.body.mode} onchange={(e) => setBodyMode(e.currentTarget.value)} aria-label="Body mode">
          <option value="none">None</option>
          <option value="json">JSON</option>
          <option value="raw">Raw</option>
          <option value="formUrlEncoded">Form URL-encoded</option>
          {#if request.body.mode === 'multipart' || request.body.mode === 'graphql'}
            <option value={request.body.mode}>{request.body.mode === 'multipart' ? 'Multipart (imported)' : 'GraphQL (imported)'}</option>
          {/if}
        </select>
      </div>
      {#if request.body.mode === 'json' || request.body.mode === 'raw'}
        <textarea class="body mono" bind:value={request.body.text} spellcheck="false"></textarea>
      {:else if request.body.mode === 'formUrlEncoded'}
        <KeyValueEditor bind:items={request.body.fields} />
      {:else if request.body.mode === 'multipart' || request.body.mode === 'graphql'}
        <p class="body-hint">
          This body was imported and isn't editable yet. It is preserved in the request file.
        </p>
      {/if}
    {:else if tab === 'auth'}
      <AuthEditor bind:auth={request.auth} />
    {/if}
  </div>
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100%;
  }
  .bar {
    display: flex;
    gap: 8px;
  }
  .method {
    flex: 0 0 auto;
  }
  .url {
    flex: 1;
    min-width: 0;
  }
  select,
  input,
  textarea {
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 12px;
  }
  .send {
    background: var(--accent);
    color: #0f1318;
    border: none;
    border-radius: 6px;
    padding: 6px 16px;
    font-weight: 600;
    cursor: pointer;
  }
  .send:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    background: transparent;
    color: var(--text-muted);
    border: none;
    border-bottom: 2px solid transparent;
    padding: 6px 10px;
    cursor: pointer;
    font-size: 12px;
  }
  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .tab-body {
    flex: 1;
    min-height: 0;
  }
  .body-mode {
    margin-bottom: 8px;
  }
  .body {
    width: 100%;
    min-height: 200px;
    resize: vertical;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .body-hint {
    color: var(--text-muted);
    font-size: 12px;
    padding: 8px 2px;
  }
</style>
