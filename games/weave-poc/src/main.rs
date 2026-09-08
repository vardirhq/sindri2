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
    fn portrait_and_landscape_resolve_different_geometry_without_touching_source() {
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
            // Wider than the old 700px cutoff on purpose: portrait layout is
            // about the viewport's shape, not a guess at the device class.
            Viewport {
                width: 980.0,
                height: 1800.0,
            },
        )
        .expect("portrait resolves");

        let desktop_width = menu_width(desktop.world());
        let mobile_width = menu_width(mobile.world());
        assert!((desktop_width - mobile_width).abs() > f32::EPSILON);
        let ninety_two_vw = 0.92 * 2.0 * 980.0 / 1800.0;
        assert!((mobile_width - ninety_two_vw).abs() < 1.0e-6);
        assert!((menu_width(&authored) - source_width).abs() <= f32::EPSILON);
    }

    fn string_field<'a>(world: &'a World, id: &str, component: &str, field: &str) -> &'a str {
        world
            .entities()
            .find(|(_, data)| {
                data.source_id
                    .as_ref()
                    .is_some_and(|source_id| source_id.as_str() == id)
            })
            .and_then(|(_, data)| data.components.get(component))
            .and_then(|payload| payload.get(field))
            .and_then(|value| value.as_str())
            .expect("string component field")
    }

    fn number_field(world: &World, id: &str, component: &str, field: &str) -> f64 {
        world
            .entities()
            .find(|(_, data)| {
                data.source_id
                    .as_ref()
                    .is_some_and(|source_id| source_id.as_str() == id)
            })
            .and_then(|(_, data)| data.components.get(component))
            .and_then(|payload| payload.get(field))
            .and_then(serde_json::Value::as_f64)
            .expect("numeric component field")
    }

    #[test]
    fn demo_resolves_labels_colors_and_responsive_type() {
        let document = SceneDocument::from_json(SCENE).expect("scene parses");
        let authored = World::from_scene(&document).expect("scene loads").world;
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

        assert_eq!(
            string_field(desktop.world(), "title", "sindri.ui.text", "text"),
            "ONE SCENE. EVERY VIEWPORT."
        );
        assert_eq!(
            string_field(desktop.world(), "title", "sindri.ui.text", "font"),
            "fonts/ChakraPetch-Regular.ttf"
        );
        assert_ne!(
            desktop.world().entities().find_map(|(_, data)| {
                (data.source_id.as_ref()?.as_str() == "play")
                    .then(|| data.components["sindri.ui.shape"]["fill"].clone())
            }),
            desktop.world().entities().find_map(|(_, data)| {
                (data.source_id.as_ref()?.as_str() == "source")
                    .then(|| data.components["sindri.ui.shape"]["fill"].clone())
            })
        );

        assert_eq!(
            string_field(desktop.world(), "hero", "sindri.ui.layout", "direction"),
            "row"
        );
        assert_eq!(
            string_field(mobile.world(), "hero", "sindri.ui.layout", "direction"),
            "column"
        );
        assert_eq!(
            string_field(desktop.world(), "hero-copy", "sindri.ui.layout", "align"),
            "start"
        );
        assert_eq!(
            string_field(mobile.world(), "hero-copy", "sindri.ui.layout", "align"),
            "center"
        );

        let desktop_size = number_field(desktop.world(), "title", "sindri.ui.text", "font_size");
        let mobile_size = number_field(mobile.world(), "title", "sindri.ui.text", "font_size");
        assert!((desktop_size - 84.0 / 720.0).abs() < 1.0e-6);
        assert!((mobile_size - 58.0 / 844.0).abs() < 1.0e-6);
    }
}
