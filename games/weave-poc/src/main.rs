use sindri_core::{SceneDocument, World};
use sindri_weave::PresentationWorld;
use weave::{Viewport, parse};

const SCENE: &str = include_str!("../assets/demo.scene.json");
const STYLE: &str = include_str!("../assets/demo.weave");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = SceneDocument::from_json(SCENE)?;
    let authored = World::from_scene(&document)?.world;
    let stylesheet = parse(STYLE)?;

    for viewport in [
        Viewport {
            width: 1280.0,
            height: 720.0,
        },
        Viewport {
            width: 390.0,
            height: 844.0,
        },
    ] {
        let styled = PresentationWorld::resolve(&authored, &stylesheet, viewport)?;
        println!("{}x{}", viewport.width, viewport.height);
        for (_, data) in styled.world().entities() {
            let Some(id) = &data.source_id else { continue };
            let transform = data.transform_3d.unwrap_or_default();
            println!(
                "  {id:?}: position=({:.3}, {:.3}) size=({:.3}, {:.3})",
                transform.position[0],
                transform.position[1],
                transform.scale[0],
                transform.scale[1],
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sindri_core::{SceneDocument, World};
    use sindri_weave::PresentationWorld;
    use weave::{Viewport, parse};

    use super::{SCENE, STYLE};

    fn menu_width(world: &World) -> f32 {
        world
            .entities()
            .find(|(_, data)| {
                data.source_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == "menu")
            })
            .and_then(|(_, data)| data.transform_3d)
            .expect("menu transform")
            .scale[0]
    }

    #[test]
    fn mobile_and_desktop_resolve_different_geometry_without_touching_source() {
        let document = SceneDocument::from_json(SCENE).expect("scene parses");
        let authored = World::from_scene(&document).expect("scene loads").world;
        let source_width = menu_width(&authored);
        let stylesheet = parse(STYLE).expect("Weave parses");

        let desktop = PresentationWorld::resolve(
            &authored,
            &stylesheet,
            Viewport {
                width: 1280.0,
                height: 720.0,
            },
        )
        .expect("desktop resolves");
        let mobile = PresentationWorld::resolve(
            &authored,
            &stylesheet,
            Viewport {
                width: 390.0,
                height: 844.0,
            },
        )
        .expect("mobile resolves");

        let desktop_width = menu_width(desktop.world());
        let mobile_width = menu_width(mobile.world());
        assert!((desktop_width - mobile_width).abs() > f32::EPSILON);
        assert!((menu_width(&authored) - source_width).abs() <= f32::EPSILON);
    }
}
