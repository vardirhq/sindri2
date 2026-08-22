from pathlib import Path

p = Path("editor/src/native.rs")
text = p.read_text()

old = '''/// Stateful authoring surfaces shared across component sections.
struct InspectorTools<'a> {
    animation: &'a mut AnimationTool,
    tilemap: &'a mut TilemapTool,
}

/// Draws every component on an entity, editable, and reports what changed.
'''
new = '''/// Stateful authoring surfaces shared across component sections.
struct InspectorTools<'a> {
    animation: &'a mut AnimationTool,
    tilemap: &'a mut TilemapTool,
}

/// Project resources component authoring may offer without making the section
/// renderer carry one argument per resource kind.
struct ComponentAuthoringContext<'a> {
    project_root: Option<&'a Path>,
    fonts: &'a [String],
    animation_texture: Option<&'a str>,
    grids: &'a [(String, String)],
}

/// Draws every component on an entity, editable, and reports what changed.
'''
if old not in text:
    raise SystemExit("missing InspectorTools anchor")
text = text.replace(old, new, 1)

old = '''fn components_sections(
    ui: &mut egui::Ui,
    components: &mut BTreeMap<String, Value>,
    scripts: &SceneScripts,
    project_root: Option<&Path>,
    fonts: &[String],
    animation_texture: Option<&str>,
    grids: &[(String, String)],
    tools: &mut InspectorTools<'_>,
) -> Option<String> {'''
new = '''fn components_sections(
    ui: &mut egui::Ui,
    components: &mut BTreeMap<String, Value>,
    scripts: &SceneScripts,
    authoring: ComponentAuthoringContext<'_>,
    tools: &mut InspectorTools<'_>,
) -> Option<String> {'''
if old not in text:
    raise SystemExit("missing components_sections signature")
text = text.replace(old, new, 1)

text = text.replace('            text_section(ui, payload, fonts);', '            text_section(ui, payload, authoring.fonts);', 1)
text = text.replace(
    '''                project_root,
                animation_texture,
                tools.animation,''',
    '''                authoring.project_root,
                authoring.animation_texture,
                tools.animation,''',
    1,
)
text = text.replace(
    '            tilemap_section(ui, payload, project_root, tools.tilemap);',
    '            tilemap_section(ui, payload, authoring.project_root, tools.tilemap);',
    1,
)
text = text.replace(
    '            grid_occupant_section(ui, payload, grids);',
    '            grid_occupant_section(ui, payload, authoring.grids);',
    1,
)

old = '''                        removed = components_sections(
                            ui,
                            &mut components,
                            scripts,
                            project_root.as_deref(),
                            &fonts,
                            animation_texture.as_deref(),
                            &grids,
                            &mut tools,
                        );'''
new = '''                        removed = components_sections(
                            ui,
                            &mut components,
                            scripts,
                            ComponentAuthoringContext {
                                project_root: project_root.as_deref(),
                                fonts: &fonts,
                                animation_texture: animation_texture.as_deref(),
                                grids: &grids,
                            },
                            &mut tools,
                        );'''
if old not in text:
    raise SystemExit("missing components_sections caller")
text = text.replace(old, new, 1)

p.write_text(text)
