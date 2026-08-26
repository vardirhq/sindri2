//! The controls the editor is built from.
//!
//! Each module here exists because the thing it draws appears in more than one
//! panel and has to look and behave the same in all of them. A wrapper that only
//! renames an egui call does not belong here; a widget that centralises
//! painting, spacing, interaction, or meaning does.

pub mod asset;
pub mod button;
pub mod panel;
pub mod property;
pub mod section;
pub mod tabs;
pub mod toolbar;
pub mod tree;
pub mod vector;
