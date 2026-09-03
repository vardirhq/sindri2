//! One module per format change, oldest first.
//!
//! A new format version is a new file here and one more `register` line
//! in `SceneMigrator::builtin`. Nothing already written moves: a step
//! that has ever run against a file someone wrote must keep producing
//! exactly what it produced then.

pub(super) mod camera;
pub(super) mod namespace;
pub(super) mod sprites;
pub(super) mod text;
pub(super) mod transform;
pub(super) mod ui;
pub(super) mod vector;
