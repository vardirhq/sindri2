//! Renderer- and platform-independent foundations for Sindri.
//!
//! This crate deliberately has no dependency on a window, GPU, browser, editor,
//! physics engine, scripting runtime, or async executor.

mod asset;
mod component;
mod engine;
mod entity;
mod lifecycle;
mod scene;
mod time;
mod transform;
mod world;

pub use asset::{
    AssetHandle, AssetId, AssetIdError, AssetLoadError, AssetLoadErrorKind, AssetStatus,
    AssetStore, AssetStoreError, WeakAssetHandle,
};
pub use component::{
    ComponentMetadata, ComponentRegistryError, ComponentSchemaRegistry, SceneComponent,
    UnknownComponentPolicy,
};
pub use engine::{EngineCore, EngineError, EngineFrame};
pub use entity::EntityId;
pub use lifecycle::{EngineLifecycle, EngineState, LifecycleError};
pub use scene::{
    SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, SceneEntityId, SceneError, SceneMetadata,
};
pub use time::{FixedStepClock, FixedStepConfig, FrameSteps, TimeError};
pub use transform::{Transform2D, Transform3D};
pub use world::{EntityData, LoadedScene, World, WorldError};

/// Common imports for native Sindri game code.
pub mod prelude {
    pub use crate::{
        AssetHandle, AssetId, AssetLoadErrorKind, AssetStatus, AssetStore, ComponentSchemaRegistry,
        EngineCore, EngineLifecycle, EngineState, EntityData, EntityId, FixedStepClock,
        FixedStepConfig, SceneComponent, SceneDocument, SceneEntity, SceneEntityId, Transform2D,
        Transform3D, UnknownComponentPolicy, WeakAssetHandle, World,
    };
}
