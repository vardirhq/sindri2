//! Public facade for the Sindri game engine.
//!
//! Feature crates will be re-exported here as they become stable. Keeping a
//! facade prevents games from depending on Sindri's internal crate layout.

pub use sindri_assets as assets;
pub use sindri_core as core;
pub use sindri_grid as grid;

#[cfg(feature = "render")]
pub use sindri_gpu as gpu;
#[cfg(feature = "render")]
pub use sindri_render as render;
#[cfg(feature = "render")]
pub use sindri_scene as scene;

pub mod prelude {
    pub use sindri_assets::{
        AssetDecoder, AssetLoadCompletion, AssetLoadQueue, AssetLoadQueueConfig, AssetLoadRequest,
        AssetSource, SceneAssetDecoder, TextureAsset, TextureAssetDecoder, decode_completion,
    };
    pub use sindri_core::prelude::*;
    #[cfg(feature = "render")]
    pub use sindri_gpu::{GpuContext, GpuRequestOptions, SurfaceProfile};
    pub use sindri_grid::{
        FootprintError, GridBounds, GridCoord, GridFootprint, GridMovement, GridOccupancy,
        GridPath, GridPathCosts, GridPathError, GridPathfinder, GridPlacementError, GridPoint,
        GridSpace, GridWallEdge, GridWallError, GridWalls, PlanePoint, PlaneYAxis, Projection,
    };
    #[cfg(feature = "render")]
    pub use sindri_scene::{CameraView, SceneExtractor, SpriteAnimations, WorldProjection};
}
