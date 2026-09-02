//! Renderer- and platform-independent foundations for Sindri.
//!
//! This crate deliberately has no dependency on a window, GPU, browser, editor,
//! physics engine, scripting runtime, or async executor.

mod asset;
mod command;
mod component;
mod engine;
mod entity;
mod lifecycle;
mod migration;
mod prefab;
mod random;
mod save;
mod scene;
mod sheet;
mod tags;
mod time;
mod transform;
mod world;

pub use asset::{
    AssetHandle, AssetId, AssetIdError, AssetLoadError, AssetLoadErrorKind, AssetStatus,
    AssetStore, AssetStoreError, SpriteRef, SpriteRefError, WeakAssetHandle,
};
pub use command::{CommandBuffer, CommandError, CommandHistory, Transaction, WorldCommand};
pub use component::{
    ComponentMetadata, ComponentRegistryError, ComponentSchemaRegistry, SceneComponent,
    UnknownComponentPolicy,
};
pub use engine::{EngineCore, EngineError, EngineFrame};
pub use entity::EntityId;
pub use lifecycle::{EngineLifecycle, EngineState, LifecycleError};
pub use migration::{SceneMigrationError, SceneMigrationStep, SceneMigrator};
pub use prefab::{PREFAB_FORMAT_VERSION, PrefabDocument, PrefabError, PrefabJsonError};
pub use random::Rng;
pub use save::{SAVE_FORMAT_VERSION, SaveDocument, SaveReadError, SaveState, SaveStore, SaveValue};
pub use scene::{
    SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, SceneEntityId, SceneError, SceneJsonError,
    SceneMetadata,
};
pub use sheet::{SHEET_FORMAT_VERSION, SheetError, SheetGrid, SpriteSheetDocument, sheet_id_for};
pub use tags::TagsComponent;
pub use time::{FixedStepClock, FixedStepConfig, FrameSteps, TimeError, TimeScale};
pub use transform::Transform3D;
pub use world::{EntityData, LoadedScene, SpawnedPrefab, World, WorldError};

/// Common imports for native Sindri game code.
pub mod prelude {
    pub use crate::{
        AssetHandle, AssetId, AssetLoadErrorKind, AssetStatus, AssetStore, CommandBuffer,
        CommandHistory, ComponentSchemaRegistry, EngineCore, EngineLifecycle, EngineState,
        EntityData, EntityId, FixedStepClock, FixedStepConfig, SceneComponent, SceneDocument,
        SceneEntity, SceneEntityId, SceneMetadata, SceneMigrator, TimeScale, Transform3D,
        UnknownComponentPolicy, WeakAssetHandle, World, WorldCommand,
    };
}
