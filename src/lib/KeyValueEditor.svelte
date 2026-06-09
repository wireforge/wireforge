<script lang="ts">
  import type { KeyValue } from './types';

  let {
    items = $bindable(),
    keyPlaceholder = 'Key',
    valuePlaceholder = 'Value',
  }: { items: KeyValue[]; keyPlaceholder?: string; valuePlaceholder?: string } = $props();

  function add() {
    items = [...items, { enabled: true, key: '', value: '' }];
  }

  function remove(index: number) {
    items = items.filter((_, i) => i !== index);
  }
</script>

<div class="kv-editor">
  {#each items as item, i (i)}
    <div class="kv">
      <input type="checkbox" bind:checked={item.enabled} aria-label="Enabled" />
      <input class="mono" bind:value={item.key} placeholder={keyPlaceholder} spellcheck="false" />
      <input class="mono" bind:value={item.value} placeholder={valuePlaceholder} spellcheck="false" />
      <button class="ghost" onclick={() => remove(i)} aria-label="Remove row">✕</button>
    </div>
  {/each}
  <button class="ghost add" onclick={add}>+ Add</button>
</div>

<style>
  .kv {
    display: grid;
    grid-template-columns: auto 1fr 1fr auto;
    gap: 6px;
    margin-bottom: 4px;
  }
  input {
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 12px;
  }
  input[type='checkbox'] {
    padding: 0;
  }
  .ghost {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 2px 8px;
    cursor: pointer;
  }
  .add {
    margin-top: 2px;
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>
