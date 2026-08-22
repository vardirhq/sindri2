from pathlib import Path


def replace(path, old, new, count=1):
    p = Path(path)
    text = p.read_text()
    if old not in text:
        raise SystemExit(f'missing pattern in {path}: {old[:100]!r}')
    p.write_text(text.replace(old, new, count))

# Keep sindri-decay on its documented dependency side of the graph.
p = Path('crates/sindri-decay/Cargo.toml')
text = p.read_text().replace('sindri-scene = { path = "../sindri-scene" }\n', '')
p.write_text(text)

# The language host asks its caller for navigation rather than learning about
# scene/render types. It still owns target->cell conversion because Grid.* is
# its scripting surface; the provider owns only the world-navigation policy.
p = Path('crates/sindri-decay/src/host.rs')
text = p.read_text()
text = text.replace(
    'use sindri_grid::{\n    GridCoord, GridPathfinder, GridPoint, GridSpace, PlanePoint, PlaneYAxis, Projection,\n};\n',
    'use sindri_grid::{GridCoord, GridPoint, GridSpace, PlanePoint, PlaneYAxis, Projection};\n',
)
text = text.replace('use sindri_scene::WorldGridNavigation;\n', '')
anchor = '''#[derive(Clone, Copy)]
struct MapGrid {
    space: GridSpace,
    transform: Transform3D,
}
'''
insert = anchor + '''
/// Navigation supplied by the engine host without making the Decay binding
/// depend on scene or rendering crates.
///
/// The script surface already knows the mover, map, and goal cell. The provider
/// answers only the policy question: which authored route exists in this world?
/// Gather and the editor both implement this with `WorldGridNavigation`.
pub trait GridNavigationHost {
    fn find_path(
        &self,
        world: &World,
        mover: EntityId,
        grid: EntityId,
        goal: GridCoord,
    ) -> Result<Option<Vec<GridCoord>>, String>;
}
'''
if anchor not in text:
    raise SystemExit('missing MapGrid anchor')
text = text.replace(anchor, insert, 1)
text = text.replace(
    '''    /// The notes every script in the world shares.
    blackboard: &'a mut Blackboard,
    /// What the script said, in order. Drained by the caller after the call.
''',
    '''    /// The notes every script in the world shares.
    blackboard: &'a mut Blackboard,
    /// Optional world navigation supplied by the caller. Ordinary scripts do
    /// not need one; Grid pathfinding calls report a clear host error without it.
    navigation: Option<&'a dyn GridNavigationHost>,
    /// What the script said, in order. Drained by the caller after the call.
''',
    1,
)
old_ctor = '''    pub fn new(
        world: &'a mut World,
        entity: EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
    ) -> Self {
        Self {
            world,
            entity,
            context,
            blackboard,
            printed: Vec::new(),
        }
    }
'''
new_ctor = '''    pub fn new(
        world: &'a mut World,
        entity: EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
    ) -> Self {
        Self::with_navigation(world, entity, context, blackboard, None)
    }

    pub fn with_navigation(
        world: &'a mut World,
        entity: EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
        navigation: Option<&'a dyn GridNavigationHost>,
    ) -> Self {
        Self {
            world,
            entity,
            context,
            blackboard,
            navigation,
            printed: Vec::new(),
        }
    }
'''
if old_ctor not in text:
    raise SystemExit('missing inner WorldHost constructor')
text = text.replace(old_ctor, new_ctor, 1)
old_path = '''        let navigation = WorldGridNavigation::from_world(self.world, map)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
        let target_world = self
'''
new_path = '''        let navigation = self.navigation.ok_or_else(|| {
            RuntimeError::Host(format!(
                "{} needs the game host to provide grid navigation",
                path.dotted()
            ))
        })?;
        let target_world = self
'''
if old_path not in text:
    raise SystemExit('missing scene navigation construction')
text = text.replace(old_path, new_path, 1)
old_goal = '''        let goal = navigation
            .space()
            .plane_to_grid(local)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
        navigation
            .find_path(GridPathfinder::default(), entity, goal)
            .map(|route| route.map(|route| route.into_nodes()))
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))
'''
new_goal = '''        let goal = grid
            .space
            .plane_to_grid(local)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
        navigation
            .find_path(self.world, entity, map, goal)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))
'''
if old_goal not in text:
    raise SystemExit('missing navigation route call')
text = text.replace(old_goal, new_goal, 1)
p.write_text(text)

# The audio wrapper preserves its old constructor and offers a navigation-aware
# one for script runners that need synchronous Grid pathfinding.
p = Path('crates/sindri-decay/src/audio_host.rs')
text = p.read_text()
text = text.replace(
    'use crate::{Blackboard, ScriptContext};\n',
    'use crate::{Blackboard, GridNavigationHost, ScriptContext};\n',
    1,
)
old = '''    pub fn new(
        world: &'a mut sindri_core::World,
        entity: sindri_core::EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
        audio: &'a mut Vec<AudioCommand>,
    ) -> Self {
        Self {
            inner: crate::host::WorldHost::new(world, entity, context, blackboard),
            audio,
        }
    }
'''
new = '''    pub fn new(
        world: &'a mut sindri_core::World,
        entity: sindri_core::EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
        audio: &'a mut Vec<AudioCommand>,
    ) -> Self {
        Self::with_navigation(world, entity, context, blackboard, audio, None)
    }

    pub fn with_navigation(
        world: &'a mut sindri_core::World,
        entity: sindri_core::EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
        audio: &'a mut Vec<AudioCommand>,
        navigation: Option<&'a dyn GridNavigationHost>,
    ) -> Self {
        Self {
            inner: crate::host::WorldHost::with_navigation(
                world,
                entity,
                context,
                blackboard,
                navigation,
            ),
            audio,
        }
    }
'''
if old not in text:
    raise SystemExit('missing audio WorldHost constructor')
text = text.replace(old, new, 1)
p.write_text(text)

# Export the host boundary to consumers.
replace(
    'crates/sindri-decay/src/lib.rs',
    'pub use host::ScriptContext;\n',
    'pub use host::{GridNavigationHost, ScriptContext};\n',
)

# Keep the existing Scripts::advance API for all scripts that do not use
# pathfinding, and add an explicit navigation-aware entry point for real hosts.
p = Path('crates/sindri-decay/src/scripts.rs')
text = p.read_text()
text = text.replace(
    '    ScriptReport, WorldHost,\n',
    '    GridNavigationHost, ScriptReport, WorldHost,\n',
    1,
)
old_sig = '''    pub fn advance(
        &mut self,
        world: &mut World,
        components: &ComponentSchemaRegistry,
        sources: &ScriptSources,
        input: &InputState,
        delta_seconds: f32,
    ) -> ScriptReport {
        let mut report = ScriptReport::default();
'''
new_sig = '''    pub fn advance(
        &mut self,
        world: &mut World,
        components: &ComponentSchemaRegistry,
        sources: &ScriptSources,
        input: &InputState,
        delta_seconds: f32,
    ) -> ScriptReport {
        self.advance_with_navigation(world, components, sources, input, delta_seconds, None)
    }

    pub fn advance_with_navigation(
        &mut self,
        world: &mut World,
        components: &ComponentSchemaRegistry,
        sources: &ScriptSources,
        input: &InputState,
        delta_seconds: f32,
        navigation: Option<&dyn GridNavigationHost>,
    ) -> ScriptReport {
        let mut report = ScriptReport::default();
'''
if old_sig not in text:
    raise SystemExit('missing Scripts::advance signature')
text = text.replace(old_sig, new_sig, 1)
old_tick_call = '''                delta_seconds,
            ) {
'''
new_tick_call = '''                delta_seconds,
                navigation,
            ) {
'''
# Only the tick invocation inside advance; first occurrence after `match tick(`.
pos = text.index('            match tick(')
post = text[pos:]
if old_tick_call not in post:
    raise SystemExit('missing tick invocation tail')
post = post.replace(old_tick_call, new_tick_call, 1)
text = text[:pos] + post
old_tick_sig = '''    component: &ScriptComponent,
    delta_seconds: f32,
) -> Result<Vec<String>, ScriptFailure> {
'''
new_tick_sig = '''    component: &ScriptComponent,
    delta_seconds: f32,
    navigation: Option<&dyn GridNavigationHost>,
) -> Result<Vec<String>, ScriptFailure> {
'''
if old_tick_sig not in text:
    raise SystemExit('missing tick signature')
text = text.replace(old_tick_sig, new_tick_sig, 1)
text = text.replace(
    'WorldHost::new(world, entity, context, blackboard, audio),\n',
    'WorldHost::with_navigation(world, entity, context, blackboard, audio, navigation),\n',
    1,
)
p.write_text(text)

# The generic analyzer/host contract gets a tiny fake provider. It tests that
# Grid calls are wired, not the engine A* implementation (that has its own tests).
p = Path('crates/sindri-decay/src/surface.rs')
text = p.read_text()
text = text.replace(
    '    use sindri_core::{EntityData, Transform3D, World};\n',
    '    use sindri_core::{EntityData, EntityId, Transform3D, World};\n    use sindri_grid::GridCoord;\n',
    1,
)
text = text.replace(
    '    use crate::{ScriptContext, WorldHost, environment, surface::ENTITY};\n',
    '    use crate::{GridNavigationHost, ScriptContext, WorldHost, environment, surface::ENTITY};\n',
    1,
)
insert_at = '''    fn blackboard() -> crate::Blackboard {
        crate::Blackboard::new()
    }
'''
provider = insert_at + '''
    struct OpenNavigation;

    impl GridNavigationHost for OpenNavigation {
        fn find_path(
            &self,
            _world: &World,
            _mover: EntityId,
            _grid: EntityId,
            goal: GridCoord,
        ) -> Result<Option<Vec<GridCoord>>, String> {
            Ok(Some(vec![GridCoord::new(0, 0), goal]))
        }
    }
'''
if insert_at not in text:
    raise SystemExit('missing blackboard fixture')
text = text.replace(insert_at, provider, 1)
text = text.replace(
    'WorldHost::new(&mut world, entity, context(&input), &mut board, &mut audio);\n',
    'WorldHost::with_navigation(\n                        &mut world,\n                        entity,\n                        context(&input),\n                        &mut board,\n                        &mut audio,\n                        Some(&OpenNavigation),\n                    );\n',
    1,
)
p.write_text(text)

# Real hosts provide the scene adapter. The only repeated code is the five-line
# trait bridge; pathfinding semantics remain exclusively in WorldGridNavigation.
p = Path('game/src/lib.rs')
text = p.read_text()
text = text.replace(
    'use sindri_decay::{AudioCommand, ScriptComponent, ScriptSources, Scripts};\n',
    'use sindri_decay::{\n    AudioCommand, GridNavigationHost, ScriptComponent, ScriptSources, Scripts,\n};\n',
    1,
)
text = text.replace(
    'use sindri_scene::{\n    AudioSourceComponent, CameraView, SceneExtractor, SheetBindError, SpriteAnimations,\n    TextureBindings,\n};\n',
    'use sindri_scene::{\n    AudioSourceComponent, CameraView, SceneExtractor, SheetBindError, SpriteAnimations,\n    TextureBindings, WorldGridNavigation,\n};\nuse sindri_grid::{GridCoord, GridPathfinder};\n',
    1,
)
anchor = '''#[cfg(not(target_arch = "wasm32"))]
type GatherAudio = NativeAudioBackend;
'''
bridge = anchor + '''
struct SceneNavigation;

impl GridNavigationHost for SceneNavigation {
    fn find_path(
        &self,
        world: &World,
        mover: sindri_core::EntityId,
        grid: sindri_core::EntityId,
        goal: GridCoord,
    ) -> Result<Option<Vec<GridCoord>>, String> {
        WorldGridNavigation::from_world(world, grid)
            .map_err(|error| error.to_string())?
            .find_path(GridPathfinder::default(), mover, goal)
            .map(|path| path.map(|path| path.into_nodes()))
            .map_err(|error| error.to_string())
    }
}
'''
if anchor not in text:
    raise SystemExit('missing Gather audio type anchor')
text = text.replace(anchor, bridge, 1)
text = text.replace(
    '''            self.scripts
                .advance(world, &self.components, &self.sources, input, delta_seconds);
''',
    '''            self.scripts.advance_with_navigation(
                world,
                &self.components,
                &self.sources,
                input,
                delta_seconds,
                Some(&SceneNavigation),
            );
''',
    1,
)
p.write_text(text)

# Editor play mode uses the same scene adapter, so a pathfinding script behaves
# the same in Game view as in the shipped game.
p = Path('editor/src/scripts.rs')
text = p.read_text()
text = text.replace(
    'use sindri_decay::{\n    ScriptExport, ScriptFailure, ScriptReport, ScriptSources, Scripts, referenced_sources,\n};\n',
    'use sindri_decay::{\n    GridNavigationHost, ScriptExport, ScriptFailure, ScriptReport, ScriptSources, Scripts,\n    referenced_sources,\n};\nuse sindri_grid::{GridCoord, GridPathfinder};\nuse sindri_scene::WorldGridNavigation;\n',
    1,
)
anchor = '''const WATCH_INTERVAL: Duration = Duration::from_secs(1);
'''
bridge = anchor + '''
struct SceneNavigation;

impl GridNavigationHost for SceneNavigation {
    fn find_path(
        &self,
        world: &World,
        mover: sindri_core::EntityId,
        grid: sindri_core::EntityId,
        goal: GridCoord,
    ) -> Result<Option<Vec<GridCoord>>, String> {
        WorldGridNavigation::from_world(world, grid)
            .map_err(|error| error.to_string())?
            .find_path(GridPathfinder::default(), mover, goal)
            .map(|path| path.map(|path| path.into_nodes()))
            .map_err(|error| error.to_string())
    }
}
'''
if anchor not in text:
    raise SystemExit('missing editor watch anchor')
text = text.replace(anchor, bridge, 1)
text = text.replace(
    '''        self.scripts
            .advance(world, components, &self.sources, input, delta_seconds)
''',
    '''        self.scripts.advance_with_navigation(
            world,
            components,
            &self.sources,
            input,
            delta_seconds,
            Some(&SceneNavigation),
        )
''',
    1,
)
p.write_text(text)

# Record the boundary, since CLAUDE.md is the explicit dependency contract.
p = Path('CLAUDE.md')
text = p.read_text()
text = text.replace(
    'sindri-decay    -> sindri-core + sindri-grid + sindri-platform (for input) + the decay/\n                   language crates, one way only\n',
    'sindri-decay    -> sindri-core + sindri-grid + sindri-platform (for input) + the decay/\n                   language crates, one way only; scene navigation is injected by hosts\n',
    1,
)
p.write_text(text)
