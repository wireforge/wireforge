<script lang="ts">
  import type { UnifiedResponse, WfError } from './types';

  let {
    response,
    error,
    sending,
  }: { response: UnifiedResponse | null; error: WfError | null; sending: boolean } = $props();

  let tab = $state<'body' | 'headers'>('body');

  const prettyBody = $derived.by(() => {
    if (!response) return '';
    const contentType =
      response.headers.find((h) => h.key.toLowerCase() === 'content-type')?.value ?? '';
    if (contentType.includes('json')) {
      try {
        return JSON.stringify(JSON.parse(response.body), null, 2);
      } catch {
        return response.body;
      }
    }
    return response.body;
  });

  function statusClass(status: number): string {
    if (status >= 200 && status < 300) return 'ok';
    if (status >= 400) return 'err';
    return 'warn';
  }
</script>

{#if sending}
  <div class="state">Sending…</div>
{:else if error}
  <div class="error">
    <div class="err-code mono">{error.code}</div>
    <div class="err-msg">{error.message}</div>
  </div>
{:else if response}
  <div class="meta mono">
    <span class="status {statusClass(response.status)}">{response.status} {response.statusText}</span>
    <span>{response.durationMs} ms</span>
    <span>{response.size} B</span>
    {#if response.httpVersion}<span>{response.httpVersion}</span>{/if}
    {#if response.remoteIp}<span>{response.remoteIp}</span>{/if}
  </div>

  <div class="tabs">
    <button class:active={tab === 'body'} onclick={() => (tab = 'body')}>Body</button>
    <button class:active={tab === 'headers'} onclick={() => (tab = 'headers')}>
      Headers ({response.headers.length})
    </button>
  </div>

  {#if tab === 'body'}
    <pre class="body mono">{prettyBody}</pre>
  {:else}
    <table class="headers mono">
      <tbody>
        {#each response.headers as h, i (i)}
          <tr><td>{h.key}</td><td>{h.value}</td></tr>
        {/each}
      </tbody>
    </table>
  {/if}
{:else}
  <div class="state">No response yet. Send a request.</div>
{/if}

<style>
  .state {
    color: var(--text-muted);
    padding: 8px 0;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: center;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 12px;
  }
  .status {
    font-weight: 700;
  }
  .status.ok {
    color: var(--success);
  }
  .status.warn {
    color: var(--warning);
  }
  .status.err {
    color: var(--danger);
  }
  .tabs {
    display: flex;
    gap: 4px;
    margin: 8px 0;
  }
  .tabs button {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 10px;
    cursor: pointer;
  }
  .tabs button.active {
    color: var(--text);
    border-color: var(--accent);
  }
  .body {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    font-size: 12px;
    color: var(--text);
  }
  .headers {
    border-collapse: collapse;
    font-size: 12px;
    width: 100%;
  }
  .headers td {
    border-bottom: 1px solid var(--border);
    padding: 3px 8px 3px 0;
    vertical-align: top;
    color: var(--text);
  }
  .headers td:first-child {
    color: var(--text-muted);
    white-space: nowrap;
  }
  .error {
    border: 1px solid var(--danger);
    border-radius: 8px;
    padding: 12px;
  }
  .err-code {
    color: var(--danger);
    font-weight: 700;
    margin-bottom: 4px;
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>
