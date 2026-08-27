# Sindri Decay for VS Code

Language support for `.decay` gameplay scripts.

The extension supplies syntax highlighting and starts the repository's `decay-lsp` language server. The server uses the same `sindri-decay::environment()` as the runtime, so host members accepted by the game are the members offered by completion and checked by diagnostics.

## Development setup

Build the server from the repository root:

```sh
cargo build --package decay-lsp
```

Install this extension's dependencies:

```sh
cd editors/vscode-decay
npm install
```

Then run the extension from VS Code's Extension Development Host. Set `decay.server.path` to the built executable if `decay-lsp` is not on `PATH`.

## Current language features

- live syntax and semantic diagnostics
- completion for Decay keywords, script members, Sindri globals, and typed host members
- hover signatures/types
- document symbols for scripts, components, fields, and functions
- scene-aware entity-name completion inside `World.find("...")`
- project-aware audio asset completion inside `Audio.play("...")` and `Audio.loop("...")`
- TextMate syntax highlighting and bracket/comment behavior

Entity names and audio assets are refreshed when a Decay document is saved. The language server deliberately reads project files but never writes them.
