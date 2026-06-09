# wireforge

**wireforge** is a lightweight, open-source desktop API client for individuals and small teams.

It is built around a simple idea: API work should be fast, keyboard-first, local-first, and friendly to Git collaboration. wireforge focuses on REST for v1, with GraphQL planned for v2.

> Status: early development (pre-v0.1). The desktop shell and module scaffold are in place; features land incrementally.

## Product Positioning

wireforge is a desktop-first alternative to heavyweight API tools. It is intended for developers who want:

- A fast native desktop app.
- File-based collections that work well with Git.
- Clear request and response workflows.
- Built-in collaboration through GitHub.
- Privacy-first local storage with secrets kept out of collection files.

## Core Principles

### Keyboard First

Common actions should be available from the keyboard. The command palette is a central part of the product, not an afterthought.

### Speed Is A Feature

The app should feel immediate. Local operations should avoid unnecessary loading states, large trees and responses should be virtualized, and the UI should stay responsive under realistic workloads.

### Git Aware

Collections are files. Files live in Git. Branch, dirty, unpushed, unsynced, and conflict states should be visible in the product surface.

## Tech Stack

- Tauri 2 — desktop shell.
- Svelte 5 + TypeScript — UI.
- Rust — backend (HTTP transport, Git operations, file persistence, secrets).

## Development

Requires Node.js and the Rust toolchain.

```sh
npm install
npm run tauri:dev
```

App icons must be generated once before the first build; see `src-tauri/icons/README.md`.

## License

wireforge is open source under the MIT License.
