# Decay LSP modernization plan

Status: **active audit checklist**.

This document is the source of truth for bringing `decay-lsp`, the Decay
editor integrations, and the batch preflight up to the standard required of
Sindri's primary gameplay language. It records gaps found in the September 2026
deep audit. Do not mark an item complete because an API or protocol handler
exists: exercise it with tests and, where applicable, the real VS Code client.

The architecture is worth preserving. `decay-lsp` uses the real Decay parser
and semantic analyzer together with `sindri_decay::environment()`; the work
below should deepen that shared source of truth rather than create another
language model.

## Principles

- Keep `decay/` independent of `sindri-*`; `sindri-decay` remains the
  one-way bridge that supplies Sindri's host environment.
- Prefer semantic information from Decay over string parsing in the LSP.
- Do not duplicate grammar keywords, host globals, types, or signatures where
  they can be derived or generated from authoritative language/host data.
- One structured Decay diagnostic should feed LSP, batch checking,
  `sindri-diagnostics`, CI, and eventually the editor Problems surface.
- Keep the server dependency-light unless a concrete protocol burden justifies
  an LSP framework.
- Treat protocol correctness, Unicode positions, stale document state, and
  project refresh as correctness issues, not editor polish.
- Every completed section needs regression tests. CI is not the test harness.

## P0 — correctness and drift

These are defects or sources of actively misleading tooling.

- [ ] Make language keyword knowledge authoritative instead of manually copied.
  - [x] Add current `for` and `in` support to completion/hover immediately.
  - [ ] Remove or generate the hard-coded keyword list once the language crate
        exposes authoritative keyword metadata.
  - [ ] Add a test that fails when parser keywords and tooling keywords drift.
- [ ] Bring the VS Code TextMate grammar up to the current Decay language.
  - [x] Highlight `for` and `in`.
  - [ ] Audit every implemented keyword, literal, declaration, primitive type,
        attribute, and operator against `decay/LANGUAGE.md`.
  - [ ] Stop hard-coding Sindri host globals in the grammar where generated
        semantic highlighting can replace them.
- [ ] Correct LSP document synchronization.
  - [x] Advertise explicit `TextDocumentSyncOptions` rather than numeric
        `textDocumentSync: 1`.
  - [x] Request save notifications if save remains part of refresh behavior.
  - [x] Implement `textDocument/didClose`.
  - [x] Remove closed documents from memory.
  - [x] Clear diagnostics when a document closes where the client requires it.
  - [x] Track document versions and reject/ignore stale changes.
- [ ] Make workspace/project refresh real.
  - [x] Implement `workspace/didChangeWatchedFiles` or remove the VS Code
        watcher until the server consumes it.
  - [x] Refresh scene/entity/audio data when relevant files change, not only
        when an unrelated Decay document is saved.
  - [x] Handle created, changed, and deleted project files.
  - [x] Avoid a full recursive project scan on every ordinary script save.
  - [ ] Keep project-index results deterministic.
- [ ] Fix completion insertion for functions.
  - [x] Generate zero placeholders for zero-argument functions.
  - [x] Generate one placeholder per required parameter.
  - [x] Preserve useful signature/detail text.
  - [ ] Test host and user-defined functions with 0, 1, and multiple arguments.
- [ ] Audit LSP position handling against the protocol's negotiated encoding.
  - [x] Prove non-ASCII text before the cursor maps to the correct Decay byte
        offset and diagnostic range.
  - [ ] Either negotiate UTF-8 positions or correctly translate the client's
        UTF-16 positions.
  - [ ] Add Unicode regression tests.
- [ ] Return proper JSON-RPC/LSP errors for malformed or unsupported requests
      where the protocol requires an error instead of silently returning null.
- [ ] Verify shutdown/exit lifecycle and process exit semantics against LSP
      3.17, including exit without shutdown.

## P0 — test harness

The LSP must stop relying on manual VS Code use as proof.

- [ ] Add protocol framing tests for valid messages, multiple headers, EOF,
      malformed JSON, missing/invalid `Content-Length`, and sequential messages.
- [ ] Add an in-process server harness that sends JSON-RPC requests and captures
      responses/notifications.
- [ ] Cover initialize capabilities.
- [ ] Cover open/change/save/close document lifecycle.
- [ ] Cover syntax and semantic diagnostic publication and clearing.
- [ ] Cover completion at root, after `this.`, through typed host chains, and
      for user container members.
- [ ] Cover hover.
- [ ] Cover document symbols.
- [ ] Cover project-aware string completion.
- [ ] Cover watched project file creation/change/deletion.
- [ ] Cover Unicode positions.
- [ ] Cover malformed/unknown requests without crashing the server.
- [ ] Add a VS Code extension smoke test or equivalent client integration test
      that launches the real server and proves synchronization + one language
      feature end to end.
- [ ] Keep `decay-lsp` source files under repository size limits while adding
      tests; split server responsibilities before `main.rs` grows further.

## P1 — one diagnostic pipeline

Live diagnostics are currently the strongest part of the LSP. Make them the
shared foundation instead of re-rendering the same compiler result repeatedly.

- [ ] Define a stable structured Decay diagnostic at the language/tooling seam:
      phase/source, severity, code when available, message, span/location,
      related information, and optional fix/suggestion.
- [ ] Convert syntax diagnostics into that representation once.
- [ ] Convert semantic diagnostics into that representation once.
- [x] Render the same representation as LSP diagnostics.
- [x] Render it in `decay-lsp --check` human output.
- [x] Add machine-readable `--check --json` output with a documented schema.
- [x] Adapt it into `sindri-diagnostics` without parsing human compiler prose.
- [x] Emit GitHub annotations/summary through the shared diagnostics path.
- [ ] Preserve diagnostic codes so agents and editor code actions can identify
      failure classes without matching message strings.
- [ ] Add related spans/information for diagnostics involving two declarations
      or conflicting symbols where the compiler has enough context.
- [ ] Add suggested fixes only where they are mechanically trustworthy.
- [x] Ensure a failure in the diagnostics renderer cannot hide the compiler's
      original non-zero result.

## P1 — replace textual runtime reminders

The batch checker currently finds runtime-contract reminders using substring
search. Keep the reminders, remove the false-positive-prone implementation.

- [ ] Detect `World.set_property` and `World.property_number` calls from the
      parsed/semantic representation, not `source.find`.
- [ ] Report every relevant call, not only the first occurrence.
- [ ] Do not trigger on comments or string literals.
- [ ] Tolerate normal whitespace/formatting differences.
- [ ] Attach the reminder to the actual call span.
- [ ] Give reminders stable diagnostic codes.
- [ ] Keep reminders non-fatal until a rule can be statically proven invalid.
- [ ] Add tests for real calls, multiple calls, comments, strings, and near-name
      false positives.

## P1 — semantic query model

The compiler should answer editor questions. The LSP should translate those
answers into protocol objects rather than reconstructing scope with strings.

- [ ] Design a read-only semantic model/query API in Decay that does not depend
      on LSP or Sindri.
- [ ] Provide `symbol_at(offset)`.
- [ ] Provide `type_at(offset)` where the checker knows a type.
- [ ] Provide lexical `scope_at(offset)`.
- [ ] Expose locals and function parameters in scope.
- [ ] Provide member lookup for a resolved type/value.
- [ ] Provide definition locations for user symbols.
- [ ] Provide references for user symbols.
- [ ] Preserve shadowing correctly.
- [ ] Make incomplete/error-recovered source queryable enough for editing.
- [ ] Add semantic-model tests in the Decay workspace independent of the LSP.

## P1 — completion quality

Once semantic queries exist, completion should use them.

- [ ] Complete local `let`/`var` bindings.
- [ ] Complete function parameters.
- [ ] Complete script/component fields and functions with lexical correctness.
- [ ] Complete members from the resolved expression type, not textual chains
      alone.
- [ ] Respect shadowing and nearest scope.
- [ ] Avoid duplicate completion entries when host/container/local names meet.
- [ ] Provide correct completion kinds and signature details.
- [ ] Rank/filter candidates sensibly by typed prefix and context.
- [ ] Keep completion useful on syntactically incomplete source.
- [ ] Add snippets only when they improve insertion rather than corrupt it.

### Project-aware argument completion

- [ ] Keep `World.find("...")` entity-name completion.
- [ ] Keep `Audio.play/loop("...")` asset completion.
- [ ] Audit the generated host API for every argument that represents a finite
      project-authored name/path.
- [ ] Add prefab completion for spawn APIs where the host signature permits it.
- [ ] Add scene completion for scene-transition APIs.
- [ ] Add animation clip completion where the target entity/component context
      can be established honestly.
- [ ] Add other asset-kind completion from project metadata rather than file
      extension guesses where Sindri already has authoritative asset metadata.
- [ ] Never offer a project value in a context whose type/API cannot consume it.

## P1 — hover and signature help

- [ ] Resolve hover through the semantic symbol at the cursor.
- [ ] Hover locals and parameters.
- [ ] Hover user fields/functions.
- [ ] Hover chained host members and methods.
- [ ] Show resolved expression/type information where useful.
- [ ] Show host API documentation from generated/source-of-truth metadata when
      available.
- [ ] Implement `textDocument/signatureHelp`.
- [ ] Show parameter names/types and active parameter for host and user calls.
- [ ] Test nested calls and incomplete argument lists.

## P2 — navigation and refactoring

- [ ] Implement go to definition for user-defined fields/functions and local
      bindings.
- [ ] Implement find references.
- [ ] Implement document highlights for the symbol under the cursor.
- [ ] Implement safe rename for symbols the semantic model can prove.
- [ ] Refuse rename where dynamic/project string references make it unsafe.
- [ ] Add workspace symbols if cross-file Decay declarations become meaningful.
- [ ] Test shadowing, same-name members, and multiple open documents.

## P2 — formatting

Decay needs one formatter rather than editor-specific whitespace behavior.

- [ ] Define formatter ownership in the Decay workspace.
- [ ] Format valid syntax deterministically.
- [ ] Decide and document behavior for partially invalid source.
- [ ] Add golden/idempotence tests.
- [ ] Expose formatting through the LSP.
- [ ] Expose the same formatter through CLI/preflight tooling.
- [ ] Add VS Code format-document integration.
- [ ] Do not make the TextMate grammar or LSP maintain a second formatting
      grammar.

## P2 — semantic highlighting

- [ ] Implement semantic tokens from parsed/semantic symbols.
- [ ] Distinguish host globals, host methods, user functions, fields, locals,
      parameters, types, keywords, and literals.
- [ ] Derive host symbol classification from the environment rather than a
      hard-coded VS Code list.
- [ ] Keep TextMate highlighting as a fast/fallback lexical layer.
- [ ] Add token tests for representative Decay source.

## P2 — code actions and authoring assistance

- [ ] Expose trustworthy compiler fixes as LSP code actions.
- [ ] Add quick fixes for unambiguous syntax/semantic cases only.
- [ ] Consider import-like/project fixes only if Decay gains such concepts.
- [ ] Add folding ranges for containers/functions/control blocks.
- [ ] Add selection ranges if semantic structure makes them cheap.
- [ ] Consider inlay hints for inferred/unknown types only when they clarify
      code rather than add noise.

## P2 — project model and indexing

- [ ] Replace blind recursive scanning with the Sindri project model where
      practical.
- [ ] Respect `sindri.toml` and authoritative asset roots/includes.
- [ ] Index only data needed by language features.
- [ ] Incrementally update changed project entries.
- [ ] Remove deleted entries.
- [ ] Surface an index warning/diagnostic when a relevant project file cannot be
      read or parsed instead of silently pretending the index is complete.
- [ ] Avoid indexing build output, caches, dependencies, and unrelated trees.
- [ ] Test Windows-style and Unix-style paths.
- [ ] Test workspace roots containing spaces and non-ASCII characters.
- [ ] Decide multi-root workspace behavior and implement or explicitly reject it.

## P2 — protocol robustness

- [ ] Record the supported LSP version/subset.
- [ ] Negotiate client capabilities instead of assuming VS Code behavior.
- [ ] Support cancellation for requests that can become expensive.
- [ ] Avoid doing project-wide filesystem work on the main request path.
- [ ] Bound memory retained for documents/project indexes.
- [ ] Log protocol/server failures to stderr without corrupting stdout framing.
- [ ] Add optional trace logging that never enters protocol stdout.
- [ ] Consider client process monitoring if the chosen client integration needs
      it.
- [ ] Reassess a maintained Rust LSP library only after tests show the custom
      protocol layer is becoming a maintenance burden; do not add one merely for
      fashion.

## P2 — VS Code extension quality

- [ ] Keep extension metadata/versioning aligned with supported server behavior.
- [ ] Handle server-not-found/startup failure with an actionable user message.
- [ ] Restart/reconnect cleanly after server failure where appropriate.
- [ ] Ensure watcher patterns match actual Sindri project files.
- [ ] Dispose watchers/client resources correctly.
- [ ] Add extension packaging validation.
- [ ] Add an install/run development path that does not require tribal knowledge.
- [ ] Document how an external Sindri project locates `decay-lsp`.
- [ ] Verify Windows, Linux, and macOS executable/path handling.

## P2 — agent and automation interface

`decay-lsp` is also the cheapest way to stop coding agents from inventing
Decay syntax and host APIs.

- [ ] Make `--check --json` stable enough for agent consumption.
- [ ] Add a machine-readable command to enumerate the current Sindri Decay host
      API, or point agents at an existing generated manifest with equivalent
      guarantees.
- [ ] Add machine-readable symbol/project queries only where they reuse the
      semantic/project model rather than inventing an agent-only API.
- [x] Document the shortest preflight workflow in `docs/decay-agent-guide.md`.
- [ ] Make diagnostics identify exact file/span/code/fix where known.
- [ ] Keep human output concise; machines should consume structured output.
- [ ] Add fixtures specifically for syntax patterns agents have repeatedly
      gotten wrong.

## P3 — editor integration

The native Sindri editor should eventually consume the same tooling instead of
building another Decay analyzer.

- [ ] Define the boundary by which the editor consumes structured Decay
      diagnostics.
- [ ] Add a Problems surface backed by the same diagnostics as CLI/LSP.
- [ ] Navigate from a problem to file/span.
- [ ] Reuse semantic completion/hover data if/when the editor grows a Decay
      source editor.
- [ ] Do not make LSP/JSON-RPC mandatory inside the same process if a direct Rust
      API is cleaner; share the semantic service beneath both.
- [ ] Keep editor diagnostics and external-editor diagnostics behaviorally
      equivalent through shared tests/fixtures.

## Documentation and generated truth

- [ ] Update `decay/LANGUAGE.md` whenever language syntax/semantics change.
- [ ] Update `docs/scripting.md` when Sindri host behavior changes.
- [ ] Regenerate `docs/generated/decay-api.md` when the host surface changes.
- [ ] Update `docs/decay-agent-guide.md` when preflight/agent commands change.
- [ ] Update `editors/vscode-decay/README.md` as capabilities become real.
- [ ] Keep this checklist referenced by `AGENTS.md`, `CLAUDE.md`, and
      `docs/parity.md`.
- [ ] Update the audit date/status in this document after each substantial
      tooling audit.

## Definition of done

The modernization is complete only when all of the following are true:

- [ ] Current Decay syntax cannot drift from LSP lexical knowledge unnoticed.
- [ ] Current Sindri host APIs cannot drift from editor host knowledge unnoticed.
- [ ] LSP synchronization is protocol-correct and covered by lifecycle tests.
- [ ] Unicode positions are correct.
- [ ] Diagnostics have one structured source consumed by LSP, batch preflight,
      CI/`sindri-diagnostics`, and the native editor integration point.
- [ ] Completion and hover are semantic and include locals/parameters.
- [ ] Definition, references, rename, and signature help work for the symbol
      classes where they can be implemented safely.
- [ ] Project-aware completions refresh incrementally and reflect the actual
      Sindri project model.
- [ ] Decay has one deterministic formatter exposed through tooling.
- [ ] Semantic highlighting no longer depends on a stale hard-coded host list.
- [ ] The VS Code extension has an end-to-end smoke test with the real server.
- [ ] Batch preflight runtime reminders are syntax-aware and structured.
- [ ] Agent-facing diagnostics are machine-readable and stable.
- [ ] The native editor can consume the same diagnostic truth without a second
      compiler/tooling implementation.
- [ ] Required native workspace, Decay workspace, and applicable WASM checks are
      green on the final implementation head.

## Audit record

Initial deep audit: **2026-09-21**.

The audit found a sound core boundary but meaningful tooling drift: missing
`for`/`in` keyword knowledge, stale VS Code host highlighting, incomplete
LSP synchronization, a file watcher the server does not consume, no close
lifecycle, simplistic function snippets, textual runtime-reminder detection,
manual/string-based completion and hover rather than semantic scope queries,
full-project rescans, and very little direct LSP test coverage.

Those findings are the reason this file exists. Future discoveries belong here
before or alongside their implementation so the modernization cannot quietly
degrade into fixing whichever editor annoyance happens to be visible that day.
