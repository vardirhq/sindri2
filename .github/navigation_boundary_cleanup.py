from pathlib import Path


def replace(path: str, old: str, new: str, count: int = 1) -> None:
    p = Path(path)
    text = p.read_text()
    if old not in text:
        raise SystemExit(f"missing cleanup anchor in {path}: {old[:100]!r}")
    p.write_text(text.replace(old, new, count))


# Editor now implements the navigation bridge directly, so its dependency must
# be explicit rather than arriving transitively through sindri-scene.
replace(
    "editor/Cargo.toml",
    'sindri-core = { path = "../crates/sindri-core" }\n',
    'sindri-core = { path = "../crates/sindri-core" }\n'
    'sindri-grid = { path = "../crates/sindri-grid" }\n',
)

# Once the public audio host owns the backwards-compatible no-navigation
# constructor, the inner world host only needs the explicit constructor.
p = Path("crates/sindri-decay/src/host.rs")
text = p.read_text()
old = '''    pub fn new(
        world: &'a mut World,
        entity: EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
    ) -> Self {
        Self::with_navigation(world, entity, context, blackboard, None)
    }

'''
if old not in text:
    raise SystemExit("missing now-unused inner WorldHost::new")
text = text.replace(old, "", 1)
p.write_text(text)

# Rust 1.95's pedantic Clippy quite reasonably dislikes closures whose entire
# career is calling one method. Keep the strict CI clean in both host bridges.
for path in ("game/src/lib.rs", "editor/src/scripts.rs"):
    p = Path(path)
    text = p.read_text()
    old = '.map(|path| path.map(|path| path.into_nodes()))'
    if old not in text:
        raise SystemExit(f"missing route conversion in {path}")
    p.write_text(text.replace(old, '.map(|path| path.map(sindri_grid::GridPath::into_nodes))', 1))
