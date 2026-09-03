//! The editor shell's own tests, grouped by the surface they exercise.
//!
//! A child module sees its ancestors' private items, so these reach the
//! whole of `native` without anything being widened for them.

mod hierarchy;
mod hierarchy_input;
mod inspector;
mod inspector_add;
mod project_browser;
mod runtime;
mod scene;
mod shortcuts;
mod support;
mod viewport;
