//! The Sindri editor.
//!
//! This crate is an application, and the binary in `main.rs` is all of it that
//! ships. The library exists so the parts that are not drawing — opening and
//! saving scene files, remembering preferences, the fixture scene — can be
//! reached from `tests/`. A binary-only crate cannot have integration tests,
//! and an editor with no way to test a save is an editor whose saves are
//! tested by opening it and looking.

#[cfg(not(target_arch = "wasm32"))]
pub mod console;
#[cfg(not(target_arch = "wasm32"))]
pub mod fixture;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(not(target_arch = "wasm32"))]
pub mod preferences;
#[cfg(not(target_arch = "wasm32"))]
pub mod project;
#[cfg(not(target_arch = "wasm32"))]
pub mod scene_file;
#[cfg(not(target_arch = "wasm32"))]
pub mod textures;
