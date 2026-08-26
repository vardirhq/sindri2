//! Headless coverage for world-to-frame extraction.
//!
//! Everything here runs without a GPU: a scene is loaded into a world,
//! the world is extracted, and the resulting passes are inspected
//! directly. One file per thing being extracted.

mod animation;
mod assets;
mod cameras;
mod passes;
mod sprites;
mod support;
mod text;
mod textures;
mod tilemap;
mod ui;
mod view;
