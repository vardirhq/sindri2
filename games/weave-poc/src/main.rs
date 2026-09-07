use sindri_core::{SceneDocument, World};
use sindri_scene::SceneExtractor;
use sindri_weave::PresentationWorld;
use weave::{Viewport, parse};

const SCENE: &str = include_str!("../assets/weave-poc.scene.json");
const STYLE: &str = include_str!("../assets/ui/demo.weave");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let extractor = SceneExtractor::new()?;
    let document = SceneDocument::from_json(SCENE)?;
    let loaded = World::from_scene(&document)?;
    let stylesheet = parse(STYLE)?;

    for viewport in [
        Viewport { width: 1280.0, height: 720.0 },
        Viewport { width: 390.0, height: 844.0 },
    ] {
        let presentation = PresentationWorld::resolve(
            &loaded.world,
            extractor.components(),
            &stylesheet,
            viewport,
        )?;
        println!("{}x{}", viewport.width, viewport.height);
        for (_, data) in presentation.world().entities() {
            if let Some(scene_id) = &data.scene_id {
                let transform = data.transform_3d.unwrap_or_default();
                println!(
                    "  {} position=({:.3},{:.3}) size=({:.3},{:.3})",
                    scene_id,
                    transform.position[0],
                    transform.position[1],
                    transform.scale[0],
                    transform.scale[1]
                );
            }
        }
    }
    Ok(())
}
