# Dependency policy

Sindri uses established libraries where they solve difficult generic
infrastructure well — `wgpu`, `winit`, `glam`, `serde` — and builds its own
differentiation elsewhere. What follows is the part of that stance a machine can
check, enforced by `deny.toml` and the `Dependencies` workflow.

Text follows the same rule: Glyphon supplies the hard generic work of Unicode
shaping, glyph caching, and wgpu rendering, while Sindri owns scene components,
asset references, extraction, ordering, and host integration. `fontdb` is used
at the asset boundary to validate a project font and discover its declared
family before the renderer sees it.

```bash
cargo deny check          # everything below
cargo deny check licenses # or one at a time
```

## No lockfile, so the policy is the record

`Cargo.lock` is not committed, which means a clean clone resolves fresh and there
is no lockfile diff to review a dependency change against. `deny.toml` is
therefore the only durable statement of what the tree may contain. It is checked
across the four targets this project actually builds for — Linux, Windows, macOS,
and `wasm32-unknown-unknown` — rather than every target `wgpu` and `winit`
support, because flagging a platform Sindri never compiles is noise.

## Licences

Permissive licences only. A copyleft dependency would change what shipping a
Sindri game requires, and that is a product decision rather than a build one.

The allowlist is kept to licences actually present in the tree, so adding a
dependency under a new licence fails the check and a human decides. Font licences
are on it: `egui` bundles typefaces under OFL-1.1 and Ubuntu-font-1.0, and Sindri
ships Inter under OFL-1.1 itself.

## Sources

Only crates.io. A git dependency is a build that cannot be reproduced from a
version number, and an unknown registry is one nobody has agreed to trust.

## Bans

Duplicate versions are a warning. The `wgpu`, `winit`, and `wasm-bindgen` trees
legitimately carry several versions of small `windows-sys` crates, and failing on
that would mean pinning transitive dependencies this project does not own.

Wildcard requirements are denied. A wildcard resolves to whatever exists on the
day of the build, which a workspace with no lockfile cannot tolerate. This also
catches a subtler mistake: a path dependency without a version is a wildcard, and
crates.io rejects it at publish time, so a crate can look fine locally for months
and fail the first time anyone tries to release it.

## Advisories

Advisories run in their own job, and that job does not block merges. A newly
published advisory in a transitive dependency is not caused by whichever pull
request runs next, and blocking unrelated work on it produces pressure to silence
the check rather than to act on it. A weekly schedule is what makes that safe:
the advisory is still surfaced when nobody is opening pull requests.

Ignoring an advisory is a decision to keep shipping something, so each entry in
`[advisories] ignore` carries a comment saying what it is, why it stands, and
what would clear it. Keeping the list honest is what keeps the job readable — if
the only findings are known ones, a new finding is visible.

## Automated updates

Dependabot watches two ecosystems, and they are worth different amounts here.

Workflow actions are pinned by major version and get real update pull requests.
Cargo requirements are deliberately loose and, with no committed lockfile, only
produce a pull request when a requirement genuinely has to move — the routine
"bump the lockfile" churn simply does not exist in this repository. What a
lockfile bump would otherwise catch is covered by the scheduled advisory run
instead.

Major-version updates to `wgpu` and `eframe` are excluded from automation on
purpose. Those two set the MSRV, so raising them is a decision with a changelog
entry, not a routine update.

## Adding a dependency

Ask whether the boundary is real before adding one. Then check that it does not
raise the MSRV beyond 1.95, that it compiles for `wasm32-unknown-unknown` if the
crate it lands in must, and that its licence is already on the allowlist. Shared
versions belong in `[workspace.dependencies]`, referenced as
`dep.workspace = true`.
