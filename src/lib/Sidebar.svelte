<script lang="ts">
  import Self from './Sidebar.svelte';
  import type { TreeNode } from './types';

  let {
    nodes,
    onopen,
    activePath,
    query = '',
    isRoot = true,
    onNewRequest,
    onNewFolder,
    onRename,
    onDelete,
    onDuplicate,
    onMove,
  }: {
    nodes: TreeNode[];
    onopen: (path: string) => void;
    activePath?: string;
    query?: string;
    isRoot?: boolean;
    onNewRequest: (folderPath: string) => void;
    onNewFolder: (parentPath: string) => void;
    onRename: (path: string, name: string) => void;
    onDelete: (path: string) => void;
    onDuplicate: (path: string) => void;
    onMove: (src: string, destFolder: string) => void;
  } = $props();

  let expanded = $state<Set<string>>(new Set());
  let editingId = $state<string | null>(null);
  let editValue = $state('');
  let dropTarget = $state<string | null>(null);

  const shown = (id: string) => expanded.has(id) || !!query;

  function toggle(id: string) {
    if (expanded.has(id)) expanded.delete(id);
    else expanded.add(id);
    expanded = new Set(expanded);
  }

  function startRename(n: TreeNode) {
    editingId = n.id;
    editValue = n.name;
  }
  function commitRename(n: TreeNode) {
    if (editingId !== n.id) return;
    const v = editValue.trim();
    editingId = null;
    if (v && v !== n.name) onRename(n.path, v);
  }
  function onEditKey(e: KeyboardEvent, n: TreeNode) {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitRename(n);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      editingId = null;
    }
  }

  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  // --- Drag & drop (move into folders / to root) ---
  function allow(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
  }
  function onDragStart(e: DragEvent, path: string) {
    e.dataTransfer?.setData('text/plain', path);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  }
  function onDropInto(e: DragEvent, folderPath: string) {
    e.preventDefault();
    dropTarget = null;
    const src = e.dataTransfer?.getData('text/plain');
    if (src && src !== folderPath) onMove(src, folderPath);
  }
  function onDropRoot(e: DragEvent) {
    e.preventDefault();
    dropTarget = null;
    const src = e.dataTransfer?.getData('text/plain');
    if (src) onMove(src, '');
  }
</script>

<ul class="tree">
  {#each nodes as n (n.id)}
    <li>
      {#if n.kind === 'folder'}
        <div class="row" class:drop={dropTarget === n.id}>
          {#if editingId === n.id}
            <span class="chev">{shown(n.id) ? '▾' : '▸'}</span>
            <input
              class="edit"
              bind:value={editValue}
              use:focusSelect
              onkeydown={(e) => onEditKey(e, n)}
              onblur={() => commitRename(n)}
            />
          {:else}
            <button
              class="hit"
              draggable="true"
              ondragstart={(e) => onDragStart(e, n.path)}
              ondragenter={() => (dropTarget = n.id)}
              ondragleave={() => (dropTarget = dropTarget === n.id ? null : dropTarget)}
              ondragover={allow}
              ondrop={(e) => onDropInto(e, n.path)}
              onclick={() => toggle(n.id)}
              title={n.name}
            >
              <span class="chev">{shown(n.id) ? '▾' : '▸'}</span>
              <span class="name">{n.name}</span>
            </button>
            <span class="actions">
              <button class="act" title="New request" aria-label="New request" onclick={() => onNewRequest(n.path)}>＋</button>
              <button class="act" title="New folder" aria-label="New folder" onclick={() => onNewFolder(n.path)}>🗀</button>
              <button class="act" title="Rename" aria-label="Rename" onclick={() => startRename(n)}>✎</button>
              <button class="act" title="Delete" aria-label="Delete" onclick={() => onDelete(n.path)}>🗑</button>
            </span>
          {/if}
        </div>
        {#if shown(n.id)}
          <div class="children">
            <Self
              nodes={n.children}
              {onopen}
              {activePath}
              {query}
              isRoot={false}
              {onNewRequest}
              {onNewFolder}
              {onRename}
              {onDelete}
              {onDuplicate}
              {onMove}
            />
          </div>
        {/if}
      {:else}
        <div class="row">
          {#if editingId === n.id}
            <span class="m m-{n.method.toLowerCase()}">{n.method}</span>
            <input
              class="edit"
              bind:value={editValue}
              use:focusSelect
              onkeydown={(e) => onEditKey(e, n)}
              onblur={() => commitRename(n)}
            />
          {:else}
            <button
              class="hit"
              class:active={n.path === activePath}
              draggable="true"
              ondragstart={(e) => onDragStart(e, n.path)}
              onclick={() => onopen(n.path)}
              title={n.name}
            >
              <span class="m m-{n.method.toLowerCase()}">{n.method}</span>
              <span class="name">{n.name}</span>
            </button>
            <span class="actions">
              <button class="act" title="Duplicate" aria-label="Duplicate" onclick={() => onDuplicate(n.path)}>⧉</button>
              <button class="act" title="Rename" aria-label="Rename" onclick={() => startRename(n)}>✎</button>
              <button class="act" title="Delete" aria-label="Delete" onclick={() => onDelete(n.path)}>🗑</button>
            </span>
          {/if}
        </div>
      {/if}
    </li>
  {/each}
</ul>

{#if isRoot}
  <button
    class="root-drop"
    class:drop={dropTarget === '__root__'}
    ondragenter={() => (dropTarget = '__root__')}
    ondragleave={() => (dropTarget = dropTarget === '__root__' ? null : dropTarget)}
    ondragover={allow}
    ondrop={onDropRoot}
  >
    Drop here to move to root
  </button>
{/if}

<style>
  .tree {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .children {
    padding-left: 12px;
  }
  .row {
    display: flex;
    align-items: center;
    border-radius: 6px;
  }
  .row.drop {
    outline: 1px dashed var(--accent);
    outline-offset: -1px;
  }
  .row:hover {
    background: var(--surface-code);
  }
  .hit {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    color: var(--text);
    padding: 3px 6px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
    text-align: left;
  }
  .hit.active {
    color: var(--text);
    font-weight: 600;
  }
  .chev {
    color: var(--text-muted);
    width: 10px;
    flex: 0 0 auto;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .m {
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 700;
    color: var(--text-muted);
    min-width: 34px;
    flex: 0 0 auto;
  }
  .m-get {
    color: var(--success);
  }
  .m-post,
  .m-put,
  .m-patch {
    color: var(--accent);
  }
  .m-delete {
    color: var(--danger);
  }
  .edit {
    flex: 1;
    min-width: 0;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--accent);
    border-radius: 4px;
    padding: 2px 6px;
    font-size: 12px;
    margin: 1px 0;
  }
  .actions {
    display: none;
    align-items: center;
    gap: 1px;
    padding-right: 4px;
  }
  .row:hover .actions {
    display: flex;
  }
  .act {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 4px;
    font-size: 11px;
    line-height: 1;
  }
  .act:hover {
    background: var(--bg);
    color: var(--text);
  }
  .root-drop {
    display: block;
    width: 100%;
    margin-top: 6px;
    padding: 6px;
    background: transparent;
    border: 1px dashed var(--border);
    border-radius: 6px;
    color: var(--text-muted);
    font-size: 11px;
    cursor: default;
    text-align: center;
  }
  .root-drop.drop {
    border-color: var(--accent);
    color: var(--text);
  }
</style>
