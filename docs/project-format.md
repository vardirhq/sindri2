# Project format

A Sindri project is a directory containing `sindri.toml`.

That file is the whole of the format. Its presence is what makes a folder a
project rather than a folder with a scene in it, which is the distinction the
editor had no way to draw: it opened a scene file, showed whatever directory
that file happened to sit in, and called it the project.

## The file

```toml
format_version = 1

[project]
name = "Gather"
main_scene = "assets/gather.scene.json"
```

Three fields, and each is read by something today.

- `format_version` describes the file rather than the project, which is why it
  sits above the table — TOML puts bare keys before the first table, so a
  version inside `[project]` would be a field order that serializes into a file
  it cannot read back. A manifest whose version is higher than the editor
  understands is **refused**, not guessed at: opening it would mean ignoring
  whatever the newer field said, and a project that quietly loses a setting is
  worse than one that says it needs a newer editor.
- `name` is what the project is called. Stored rather than taken from the
  directory name, because the two are not the same thing: the companion game is
  called Gather and lives in a folder called `game`. It is what the welcome
  window lists and what the project browser's header shows.
- `main_scene` is the scene opening the project opens, relative to the root and
  written with forward slashes so a checkout on another platform still finds it.
  Set from the project browser — **Set as main scene** on any scene row that is
  not already it — and claimed automatically by a scene made in a project that
  nominates none, which is never an overwrite: a project that already opens on
  something has been decided about.
  Optional, because a project can legitimately have no obvious first scene, and
  a nominated scene that has been deleted opens **nothing** rather than some
  other scene that happens to be nearby — standing one in for the other reads as
  though the named one loaded.

`PROJECT_OVERVIEW.md` sketches a larger file: window size, feature flags, an
asset root, a web canvas selector. None of that is here, and the sketch itself
says why — "avoid designing an enormous configuration schema before features
require it". The next field arrives with the feature that reads it.

`sindri.toml` is not `sindri.manifest.json`. The manifest is an asset ledger of
bytes and hashes, written by a build and verified by a loader; this is project
metadata, written by a person or by the editor.

## What creating one makes

New Project writes a directory holding the manifest, a scene, and the folders
assets are put in:

```text
my-first-game/
├── fonts/
├── scripts/
├── textures/
├── main.scene.json
└── sindri.toml
```

The asset folders sit beside the scene rather than under an `assets/` root
because that is where asset references actually resolve today:
`SceneTextures::for_scene` roots the loader at the scene's own directory. A
layout that looked tidier and loaded nothing would be worse than a flat one. A
project laid out differently — Gather keeps its scene under `assets/` — is
opened exactly the same way; this is what the editor *creates*, not what it
requires.

The scene it writes is the one New Scene writes: one world camera, from the
component registry's own default payload. A second copy of that answer living
beside the project format is a copy that drifts.

A directory that already holds a `sindri.toml` is refused rather than
overwritten. A directory holding other files is allowed, and the form says so
before the button is pressed.

## The project root and the asset root are two directories

A project is rooted at its `sindri.toml`. Asset references are not: a scene
names its textures, scripts, fonts, and clips relative to **the directory the
scene file itself is in**, because that is where `SceneTextures::for_scene`
roots the loader.

For a project the editor creates those are the same folder, and the distinction
never shows. For Gather they are two folders apart:

```text
game/                       <- the project root: sindri.toml, Cargo.toml, src/
└── assets/                 <- the asset root: where references resolve
    ├── gather.scene.json
    └── textures/orb.png    <- the scene names this "textures/orb.png"
```

`assets/textures/orb.png` is that file's path from the project root and is not a
reference to it. Writing it into a texture field names a file the loader will
look for at `game/assets/assets/textures/orb.png`, which is nothing, and the
sprite draws the missing checker.

So the browser knows both. `ProjectEntry::relative` is the path below the root,
which is what a search result shows to tell two files of the same name apart;
`ProjectEntry::reference` is how a scene names the file, which is what every
picker offers, what **Copy asset path** copies, and what the inspector checks a
typed reference against. A file outside the asset root has no reference at all —
Gather's `src/main.rs` is a real file that no component can name — and is
offered by nothing rather than offered under a path that will not resolve.

The browser lists the asset root by default for the same reason. A project's
Cargo manifest and its `src/` are part of the project and are not part of what
an editor field can point at, and a panel two thirds full of rows whose paths
mean nothing is a directory listing rather than an asset browser. The rest of
the project is one control away in the browser's toolbar, and the choice is
remembered; the control is drawn only where the two listings actually differ.

## Which project a launch opens

In order of how deliberately it was asked for, which is the ordering
`scene_io` already applied to scenes:

1. **A path on the command line.** A directory holding a manifest opens as a
   project; anything else opens as a scene, including a path to nothing — a
   named file that is missing is a failure the editor reports, never a reason to
   open something else.
2. **The last project**, when the user asked for that. The welcome window's
   footer is the only place that preference is set.
3. **The welcome window**, which asks.

A scene carries its project with it. Opening one — from the command line, from
a file dialog, from a browser row — walks up from the file to the nearest
`sindri.toml`, so a scene inside a project opens *as* that project: the browser
is rooted at the project and headed with its name, and lists the assets inside
it. A scene in no project leaves the editor with none, which is the state it was
always in before projects existed and is still a perfectly good way to edit one
file.

Opening a project opens the scene the editor was last left in when that scene is
inside it, and `main_scene` otherwise. Reopening a project should put someone
back where they were working rather than at its front door.

## The welcome window

Its own window, and the editor's is hidden until a project is open. It is not
part of editing anything: the editor's window is about a scene — its title
carries that scene's name, its panels hold that scene's entities, its viewport
renders it — and a "no project open" state painted over all of that would be an
editor pretending to be a launcher.

It lists the projects that have been opened, most recent first, twelve at most.
A project that has moved or been deleted is **shown and marked missing** rather
than quietly dropped: silently pruning the list answers "where did my project
go" with an empty row where it used to be, and the editor cannot tell an
unmounted volume from a deletion. A row leaves only when someone asks it to, and
removing it touches nothing on disk.

Each row is remembered by path and shown by name, with the name stored beside
the path. Reading every remembered project's manifest to draw the list would
mean a file read per row per frame, and a project on a disconnected network
drive would hang the window rather than appear in it. The name is re-read
whenever a project is opened, so renaming a project in its manifest shows up
next time it is opened.

Beside the list are the two ways to get a project that is not on it — New and
Open — and the projects this repository ships, listed only when they are
actually there. `SHIPPED` is relative to the working directory, which is the
repository root under `cargo run` and is somewhere else entirely for an
installed editor: a sample row that fails on the click is worse than no sample
row.

What it deliberately is not is Unity Hub. There are no editor versions to
install, no account, and no news, because Sindri has none of those things to
manage.

### How the window exists

A deferred egui viewport, which eframe's wgpu integration opens as a real second
window. Deferred rather than immediate because an immediate child viewport is
drawn by its parent, and the parent here is the hidden editor: eframe throttles
a hidden window to ten frames a second so that a `Visible` command still reaches
it, which would make the one window the user can see repaint at the rate of the
one they cannot.

The editor's window starts hidden and is revealed when a project opens. That is
also what makes closing the welcome window with no project open close the
editor: the alternative is a running process with nothing on screen and no way
back to it.

Two consequences worth knowing:

- The welcome window is titled "Sindri" and not "Sindri Editor", because
  `scripts/capture-editor.sh` finds the editor by matching a title ending in the
  latter. That script also names the demo scene explicitly now — a launch with
  nothing on its command line opens the welcome window, which is right for a
  person and wrong for a screenshot.
- Where multiple windows are unavailable, egui embeds a child viewport inside
  its parent. The editor checks for that and shows its own window, so the
  welcome window is not painted somewhere nobody can see. On Wayland,
  `set_visible(false)` is not honoured, so the empty editor window is visible
  behind the welcome window rather than hidden; nothing else changes.

## What this does not do yet

- One scene at a time. The roadmap's "manage more than one scene at a time" is
  untouched: a project can hold many scenes and the editor opens one of them.
- Only half a settings surface. **Set as main scene** in the project browser
  nominates what a project opens on, and a scene made inside a project that
  nominates nothing claims the empty place. The project's *name* still cannot
  be changed from the editor — a project is renamed by editing the file.
- Nothing in the engine reads `sindri.toml`. It is editor metadata today. When
  the runtime needs a project file — a window size, a starting scene for a
  build — this is the file it should read, and moving the format into a crate is
  the change that would make it one.
