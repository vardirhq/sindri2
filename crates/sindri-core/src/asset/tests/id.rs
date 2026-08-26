//! What an asset id accepts, and what it refuses.

use crate::AssetId;

use super::support::texture_id;

#[test]
fn asset_ids_are_portable_and_serde_validated() {
    let id = texture_id();
    assert_eq!(id.as_str(), "textures/player.png");
    assert_eq!(
        serde_json::to_string(&id).unwrap(),
        r#""textures/player.png""#
    );
    assert_eq!(
        serde_json::from_str::<AssetId>(r#""textures/player.png""#).unwrap(),
        id
    );

    for invalid in [
        "",
        "/player.png",
        "textures\\player.png",
        "textures//player.png",
        "textures/../player.png",
        "C:/textures/player.png",
        "textures/player.png?version=1",
        "textures/player.png#fragment",
    ] {
        assert!(AssetId::new(invalid).is_err(), "accepted '{invalid}'");
    }
}
