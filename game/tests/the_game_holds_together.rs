//! The game is the engine's exit gate, so what would break it is checked here.
//!
//! Not a rendering test — the offscreen capture covers that. These are the
//! things that would leave the game unopenable or unplayable while every other
//! test in the workspace still passed: a scene that names a texture nobody
//! ships, a script that stopped compiling, a property set on a field that was
//! renamed.
//!
//! What happens once they load — draw order, collision, pathfinding, finishing
//! the game — is `the_game_plays.rs`.

use std::collections::{BTreeMap, BTreeSet};

use sindri_causeway::{
    FONTS, SHEETS, TEXTURE_IDS, TEXTURES, TILE_SETS, extractor, presented_world, sources,
    stylesheets, world,
};
use sindri_core::{
    AssetId, SceneComponent, SceneDocument, SpriteSheetDocument, TileSetDocument,
    UnknownComponentPolicy, sheet_id_for,
};
use sindri_decay::{ScriptComponent, ScriptSources, Scripts};
use sindri_scene::{SpriteComponent, UiAnchor, UiTextComponent};

const SCENE: &str = include_str!("../assets/causeway.scene.json");

#[test]
fn weave_reflows_the_hud_for_a_phone() {
    let (authored, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sheets = stylesheets().expect("the composed stylesheet parses");
    assert_eq!(sheets.len(), 1, "imports compose into one root");

    let wide = presented_world(
        &authored,
        &sheets,
        weave::Viewport {
            width: 960.0,
            height: 600.0,
        },
    )
    .expect("desktop presentation resolves");
    let phone = presented_world(
        &authored,
        &sheets,
        weave::Viewport {
            width: 390.0,
            height: 844.0,
        },
    )
    .expect("phone presentation resolves");
    let title = authored
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "title")
        })
        .map(|(entity, _)| entity)
        .expect("the game has a title");

    let wide_title = extractor
        .components()
        .get::<UiTextComponent>(&wide, title)
        .expect("the title schema reads")
        .expect("the title remains active");
    let phone_title = extractor
        .components()
        .get::<UiTextComponent>(&phone, title)
        .expect("the title schema reads")
        .expect("the title remains active");
    assert_eq!(wide_title.anchor, UiAnchor::TopLeft);
    assert_eq!(phone_title.anchor, UiAnchor::Top);
    assert!(
        (wide_title.font_size - phone_title.font_size).abs() > f32::EPSILON,
        "the phone media rule changes the title size"
    );
}

/// Every texture the scene names is one the binary carries.
///
/// A missing one draws the magenta checker rather than failing, so nothing
/// else would notice — the game would just look wrong.
#[test]
fn every_texture_the_scene_names_is_shipped() {
    let (world, _scenes) = world().expect("the scene loads");
    let mut referenced: BTreeSet<String> = sindri_scene::referenced_textures(&world)
        .into_iter()
        .collect();
    for (id, json) in TILE_SETS {
        let tile_set =
            TileSetDocument::from_json(json).unwrap_or_else(|error| panic!("{id} parses: {error}"));
        referenced.extend(sindri_scene::tile_set_textures(&tile_set));
    }
    // Against the list the binary actually embeds, rather than a third copy of
    // it written here: a hand-kept list in a test drifts from the one it is
    // meant to check, and the drift is invisible until a texture is missing.
    let shipped: BTreeSet<String> = TEXTURES.iter().map(|(id, _)| (*id).to_owned()).collect();
    assert_eq!(referenced, shipped);
}

/// The browser build fetches exactly what the native build embeds.
///
/// Two lists say which textures the game has — one of bytes for the native
/// binary, one of IDs for the browser to fetch — and a texture added to one
/// and not the other is a game that looks right in one target and wrong in the
/// other, with nothing failing.
#[test]
fn the_browser_fetches_every_texture_the_native_build_embeds() {
    let embedded: BTreeSet<&str> = TEXTURES.iter().map(|(id, _)| *id).collect();
    let fetched: BTreeSet<&str> = TEXTURE_IDS.iter().copied().collect();
    assert_eq!(embedded, fetched);
}

/// Every font is embedded too; an absent font deliberately draws no text
/// rather than falling back to a machine-dependent face.
#[test]
fn every_font_the_scene_names_is_shipped() {
    let (world, _scenes) = world().expect("the scene loads");
    let referenced = sindri_scene::referenced_fonts(&world);
    let shipped = FONTS
        .iter()
        .map(|(id, _)| (*id).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(referenced, shipped);
}

/// Every script the scene names is one the binary carries, and it compiles.
#[test]
fn every_script_the_scene_names_compiles() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let named = sindri_decay::referenced_sources(&world, extractor.components());
    let sources = sources();
    for id in &named {
        assert!(sources.get(id).is_some(), "{id} is named but not shipped");
    }

    let failures = Scripts::new().compile(&world, extractor.components(), &sources);
    assert!(failures.is_empty(), "{failures:?}");
}

/// The scene uses only components the game understands, so nothing in it is
/// carried along doing nothing.
#[test]
fn the_scene_holds_no_component_the_game_cannot_run() {
    let document = SceneDocument::from_json(SCENE).expect("the scene parses");
    extractor()
        .expect("the schemas register")
        .validate(&document, UnknownComponentPolicy::Reject)
        .expect("every component is one the game runs");
}

/// A sprite drawn from a sheet names which part of it to draw.
///
/// A sheet drawn whole is every frame at once, and that is what a sprite naming
/// no part of its own falls back to. While a clip plays nothing shows, because
/// the animation names the frame; the moment one stops -- or before any script
/// has run at all, which is the title screen -- the fallback is the entire
/// strip, drawn as one squashed picture with nothing failing.
///
/// That is exactly what shipped. The player stood on the title screen as four
/// overlapping copies of itself, because `Animation.stop` is what standing
/// still now means and the texture named no frame to fall back to. Every other
/// sheet-drawn sprite in the scene already carried its `#frame`; this one did
/// not, and while the clip was unconditionally playing nothing revealed it.
///
/// So the rule belongs to the scene rather than to the script: anything drawn
/// from a sheet that cuts named frames must name one. Then no animation state
/// -- playing, stopped, or never started -- can leave a sprite undefined.
#[test]
fn every_sprite_drawn_from_a_sheet_names_a_frame_of_it() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");

    let cut_by_sheet: BTreeMap<String, BTreeSet<String>> = SHEETS
        .iter()
        .filter_map(|(id, json)| {
            let document: SpriteSheetDocument =
                serde_json::from_str(json).unwrap_or_else(|error| panic!("{id} parses: {error}"));
            let names: BTreeSet<String> = document
                .grid
                .map(|grid| grid.names)
                .unwrap_or_default()
                .into_iter()
                .chain(document.sprites.into_keys())
                .collect();
            (!names.is_empty()).then(|| ((*id).to_owned(), names))
        })
        .collect();

    for (entity, sprite) in extractor
        .components()
        .query::<SpriteComponent>(&world)
        .expect("the scene's sprites read")
    {
        let reference = sprite.reference().expect("the sprite reference parses");
        let Some(sheet) =
            sheet_id_for(&AssetId::new(reference.texture().to_owned()).expect("an asset id"))
        else {
            continue;
        };
        let Some(frames) = cut_by_sheet.get(sheet.as_str()) else {
            continue;
        };
        let name = world
            .get(entity)
            .and_then(|data| data.name.clone())
            .unwrap_or_else(|| format!("{entity:?}"));
        let Some(drawn) = reference.sprite() else {
            panic!(
                "{name} draws {} and names no frame of it, so whenever no clip \
                 is playing it draws all of {frames:?} at once",
                reference.texture(),
            );
        };
        assert!(
            frames.contains(drawn),
            "{name} draws '{drawn}', which {} does not cut: {frames:?}",
            reference.texture(),
        );
    }
}

fn _sources_are_used(_: &ScriptSources) {}
const _: fn() = || {
    let _ = ScriptComponent::TYPE_NAME;
};

/// The scene ships in canonical form, so editing it in the editor and saving
/// produces the file that is already committed rather than a whole-file diff.
///
/// Regenerate deliberately with
/// `SINDRI_UPDATE_CAUSEWAY_SCENE=1 cargo test --package sindri-gather`.
#[test]
fn the_scene_file_is_canonical() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("causeway.scene.json");
    let stored = std::fs::read_to_string(&path).expect("the scene is readable");
    let canonical = SceneDocument::from_json(&stored)
        .expect("the scene parses")
        .to_canonical_json()
        .expect("the scene serializes");
    if std::env::var_os("SINDRI_UPDATE_CAUSEWAY_SCENE").is_some() {
        std::fs::write(&path, &canonical).expect("the scene is writable");
        return;
    }
    assert_eq!(
        stored, canonical,
        "causeway.scene.json is not canonical; regenerate it with `python3 tools/level.py`"
    );
}
