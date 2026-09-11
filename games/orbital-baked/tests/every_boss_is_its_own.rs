//! No two bosses are the same boss.
//!
//! Ten of the twelve used to share one prefab, one script and one sprite sheet,
//! separated only by a `kind` number and a tint. That is a shape a codebase
//! slides back into the moment somebody needs an eleventh boss in a hurry, and
//! nothing fails when it does — the game runs perfectly well with every boss
//! wearing the same hull, which is exactly the problem.
//!
//! So the roster is asserted rather than trusted: its own script, its own
//! sheet, and clips that name frames the sheet actually holds.

use std::collections::HashSet;

use orbital_baked::Run;

/// The roster, in the order a run meets them. Index is what `boss_kind` and the
/// title's picker count in.
const ROSTER: [&str; 12] = [
    "warden",
    "harrower",
    "prism",
    "singularity",
    "crown",
    "brood",
    "mirror",
    "architect",
    "spine",
    "leviathan",
    "last-light",
    "aegis",
];

fn prefab_json(run: &Run, ident: &str) -> serde_json::Value {
    let prefab = run
        .prefabs
        .get(&format!("prefabs/{ident}.prefab.json"))
        .unwrap_or_else(|| panic!("{ident} ships a prefab"));
    serde_json::to_value(prefab).expect("the prefab serialises")
}

fn component<'a>(json: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    &json["entities"][0]["components"][name]
}

#[test]
fn every_boss_runs_its_own_script() {
    let run = Run::open().expect("the project opens");
    let mut sources = HashSet::new();
    let mut names = HashSet::new();
    for ident in ROSTER {
        let json = prefab_json(&run, ident);
        let script = component(&json, "sindri.script");
        let source = script["source"].as_str().expect("a script source");
        let name = script["script"].as_str().expect("a script name");
        assert!(
            sources.insert(source.to_owned()),
            "{ident} shares {source} with another boss"
        );
        assert!(
            names.insert(name.to_owned()),
            "{ident} shares the script name {name} with another boss"
        );
        assert!(
            !script["properties"]
                .as_object()
                .is_some_and(|properties| properties.contains_key("kind")),
            "{ident} still selects itself out of a shared script with a kind number"
        );
    }
}

#[test]
fn every_boss_wears_its_own_sheet() {
    let run = Run::open().expect("the project opens");
    let mut sheets = HashSet::new();
    for ident in ROSTER {
        let json = prefab_json(&run, ident);
        let texture = component(&json, "sindri.sprite")["texture"]
            .as_str()
            .expect("a sprite texture")
            .to_owned();
        // `textures/prism.png#body-0` — the sheet is the part before the frame.
        let sheet = texture
            .split('#')
            .next()
            .expect("a texture path")
            .to_owned();
        assert!(
            sheets.insert(sheet.clone()),
            "{ident} wears {sheet}, which another boss is already wearing"
        );
    }
}

/// A clip naming a frame the sheet does not hold is the failure this pipeline
/// makes easiest to write and hardest to see: nothing errors, and the boss just
/// never animates.
#[test]
fn every_clip_names_frames_its_own_sheet_holds() {
    let run = Run::open().expect("the project opens");
    for ident in ROSTER {
        let json = prefab_json(&run, ident);
        let texture = component(&json, "sindri.sprite")["texture"]
            .as_str()
            .expect("a sprite texture");
        let stem = texture
            .split('#')
            .next()
            .and_then(|path| path.strip_prefix("textures/"))
            .and_then(|file| file.strip_suffix(".png"))
            .expect("a sheet name");

        let text = std::fs::read_to_string(
            orbital_baked::project().join(format!("assets/textures/{stem}.sheet.json")),
        )
        .unwrap_or_else(|_| panic!("{ident}: {stem}.sheet.json reads"));
        let sheet: serde_json::Value = serde_json::from_str(&text).expect("the sheet parses");
        let held: Vec<&str> = sheet["grid"]["names"]
            .as_array()
            .expect("frame names")
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();

        let clips = component(&json, "sindri.animation.sprite")["clips"]
            .as_object()
            .unwrap_or_else(|| panic!("{ident} has clips"));
        assert!(!clips.is_empty(), "{ident} has no clips");
        for (clip, body) in clips {
            for frame in body["frames"].as_array().expect("frames") {
                let frame = frame.as_str().expect("a frame name");
                assert!(
                    held.contains(&frame),
                    "{ident}: clip {clip} names frame {frame}, \
                     which {stem}.sheet.json does not hold"
                );
            }
        }
    }
}

/// The director has to be able to reach all twelve, or a boss nobody ever
/// fights is a boss that may as well not exist.
#[test]
fn the_director_can_send_every_boss() {
    let text = std::fs::read_to_string(orbital_baked::project().join("assets/orbital.scene.json"))
        .expect("the scene reads");
    let scene: serde_json::Value = serde_json::from_str(&text).expect("the scene parses");
    let director = scene["entities"]
        .as_array()
        .expect("entities")
        .iter()
        .find(|entity| {
            entity["components"]["sindri.script"]["source"]
                .as_str()
                .is_some_and(|source| source.ends_with("director.decay"))
        })
        .expect("the scene has a director");
    let properties = director["components"]["sindri.script"]["properties"]
        .as_object()
        .expect("director properties");

    for ident in ROSTER {
        let key = ident.replace('-', "_");
        let wanted = format!("prefabs/{ident}.prefab.json");
        assert_eq!(
            properties.get(&key).and_then(serde_json::Value::as_str),
            Some(wanted.as_str()),
            "the director cannot send the {ident}"
        );
    }
}
