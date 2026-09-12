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

use sindri_core::{
    AssetId, SceneComponent, SceneDocument, SpriteSheetDocument, UnknownComponentPolicy, World,
    sheet_id_for,
};
use sindri_decay::{ScriptComponent, ScriptSources, Scripts};
use sindri_gather::{
    FONTS, SHEETS, TEXTURE_IDS, TEXTURES, extractor, presented_world, sources, stylesheets, world,
};
use sindri_scene::{
    SceneExtractor, ShapeComponent, SpriteComponent, TilemapComponent, UiAnchor, UiTextComponent,
};

const SCENE: &str = include_str!("../assets/gather.scene.json");

#[test]
fn the_island_has_authored_regions() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let (_, floor) = extractor
        .components()
        .query::<TilemapComponent>(&world)
        .expect("the tilemap schema reads")
        .into_iter()
        .next()
        .expect("Gather has a floor");

    assert_eq!((floor.columns, floor.rows), (25, 25));
    assert_eq!(floor.tiles.len(), 625);
    assert!(
        floor.tiles.windows(2).any(|tiles| tiles[0] == tiles[1]),
        "the island uses authored regions rather than a full checkerboard"
    );

    let used = floor.tiles.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        used.len(),
        floor.palette.len(),
        "every ground state in the sheet earns a place on the farm"
    );

    let count = |tile| {
        floor
            .tiles
            .iter()
            .filter(|cell| **cell == Some(tile))
            .count()
    };
    assert!(count(5) >= 60, "the farm has substantial dry working plots");
    assert!(count(6) >= 15, "one working plot is visibly watered");
    assert!(count(7) >= 30, "paths connect the farm's regions");
    assert!(count(8) >= 20, "the farmhouse has a flagstone yard");
    assert!(
        count(9) >= 150,
        "an irregular shore makes the map an island"
    );
}

/// The authored IDs of every entity carrying `C`.
///
/// Generic because the point of the landmark test is comparing two component
/// sets, and a closure cannot be generic over the component it queries.
fn authored_ids<C: SceneComponent>(extractor: &SceneExtractor, world: &World) -> BTreeSet<String> {
    extractor
        .components()
        .query::<C>(world)
        .expect("the component schema reads")
        .into_iter()
        .filter_map(|(entity, _)| {
            world
                .get(entity)?
                .source_id
                .as_ref()
                .map(|id| id.as_str().to_owned())
        })
        .collect()
}

/// The landmarks are there, and each is made of the right thing.
///
/// The split is the point. Gather's world art is baked
/// (`tools/isometric-baker`) and drawn as ordinary sprites, so the shrine, the
/// waystones and the standing stones are sprites. What stayed procedural is
/// what a shape is genuinely better at: two small pieces that a script
/// animates every frame, which no baked frame could do.
#[test]
fn landmarks_make_the_world_and_navigation_readable() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");

    let shaped = authored_ids::<ShapeComponent>(&extractor, &world);
    let drawn = authored_ids::<SpriteComponent>(&extractor, &world);

    for expected in [
        "shrine",
        "waystone-west",
        "waystone-east",
        "ridge-stone-0",
        "ridge-stone-3",
        "tree-0",
        "outcrop-0",
    ] {
        assert!(
            drawn.contains(expected),
            "Gather's {expected} should be a baked sprite"
        );
        assert!(
            !shaped.contains(expected),
            "{expected} still carries the procedural placeholder it replaced"
        );
    }

    for animated in ["shrine-heart", "wisp-halo"] {
        assert!(
            shaped.contains(animated),
            "{animated} is animated every frame, so it stays a shape"
        );
    }
}

#[test]
fn weave_reflows_the_hud_for_a_phone() {
    let (authored, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sheets = stylesheets().expect("the composed Gather stylesheet parses");
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
        .expect("Gather has a title");

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
    let referenced: BTreeSet<String> = sindri_scene::referenced_textures(&world)
        .into_iter()
        .collect();
    // Against the list the binary actually embeds, rather than a third copy of
    // it written here: a hand-kept list in a test drifts from the one it is
    // meant to check, and the drift is invisible until a texture is missing.
    let shipped: BTreeSet<String> = TEXTURES.iter().map(|(id, _)| (*id).to_owned()).collect();
    assert_eq!(referenced, shipped);
}

/// The browser build fetches exactly what the native build embeds.
///
/// Two lists say which textures Gather has — one of bytes for the native
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

/// The authored properties reach fields the scripts actually declare.
///
/// A property naming a renamed field is refused at runtime rather than ignored,
/// so this would show up as a game that reports an error every frame — worth
/// catching here instead.
#[test]
fn every_authored_property_names_a_field_its_script_exports() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sources = sources();
    let mut scripts = Scripts::new();
    scripts.compile(&world, extractor.components(), &sources);

    let scripted = extractor
        .components()
        .query::<ScriptComponent>(&world)
        .expect("sindri.script is registered");
    assert!(!scripted.is_empty(), "the game has scripts");

    for (_, component) in scripted {
        let exports = scripts
            .exports(&component.source, &component.script)
            .unwrap_or_else(|| panic!("{} did not compile", component.source));
        let declared: BTreeSet<&str> = exports.iter().map(|export| export.name.as_str()).collect();
        for name in component.properties.keys() {
            assert!(
                declared.contains(name.as_str()),
                "{}'s {} sets `{name}`, which it does not @export -- declared: {declared:?}",
                component.source,
                component.script,
            );
        }
    }
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
/// `SINDRI_UPDATE_GATHER_SCENE=1 cargo test --package sindri-gather`.
#[test]
fn the_scene_file_is_canonical() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("gather.scene.json");
    let stored = std::fs::read_to_string(&path).expect("the scene is readable");
    let canonical = SceneDocument::from_json(&stored)
        .expect("the scene parses")
        .to_canonical_json()
        .expect("the scene serializes");
    if std::env::var_os("SINDRI_UPDATE_GATHER_SCENE").is_some() {
        std::fs::write(&path, &canonical).expect("the scene is writable");
        return;
    }
    assert_eq!(
        stored, canonical,
        "gather.scene.json is not canonical; rerun with SINDRI_UPDATE_GATHER_SCENE=1"
    );
}

/// Every texture drawn in the world says where it meets the ground.
///
/// A quad is drawn centred on its entity, so a sprite that declares nothing is
/// drawn by its middle. For baked art that is right — the baker pads each frame
/// so its middle *is* the floor of the tile — and for hand-drawn art it is a
/// coin toss nobody is asked to call. The player was drawn a third of a ball low
/// for exactly that reason, and nothing failed.
///
/// So the game requires the answer to be written down rather than defaulted.
/// `"center"` is a perfectly good answer — the orbs float and say so — but it
/// has to be said, because a sheet that says nothing cannot be told apart from
/// one whose author never considered the question.
#[test]
fn every_texture_drawn_in_the_world_declares_where_it_meets_the_ground() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sheets: BTreeMap<&str, &str> = SHEETS.iter().copied().collect();

    // World sprites only. A UI image is placed by its own anchor against the
    // viewport and never stands on anything, so asking where it meets the
    // ground is a question about the wrong space.
    let drawn: BTreeSet<String> = extractor
        .components()
        .query::<SpriteComponent>(&world)
        .expect("sprites read")
        .into_iter()
        .map(|(_, sprite)| {
            sprite
                .reference()
                .expect("a drawn sprite names a texture")
                .texture()
                .to_owned()
        })
        .collect();

    let mut silent = Vec::new();
    for texture in drawn {
        let texture_id = AssetId::new(&texture).expect("a drawn texture is a valid id");
        let id = sheet_id_for(&texture_id).expect("a texture has a sheet id");
        let Some(json) = sheets.get(id.as_str()) else {
            silent.push(format!("{texture} has no sheet at all"));
            continue;
        };
        let sheet = SpriteSheetDocument::from_json(json).expect("a shipped sheet parses");
        if sheet.anchor.is_none() {
            silent.push(format!("{texture} declares no anchor"));
        }
    }

    assert!(
        silent.is_empty(),
        "these are drawn in the world without saying where they touch it: {silent:?}"
    );
}
