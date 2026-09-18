//! Deriving a transform from a cell, and a depth from a position.
//!
//! One file per question the resolver answers: where a cell puts a thing on a
//! projected grid, where it puts one on a grid of boxes, and how often the
//! ground under them has to be worked out.

mod caching;
mod on_a_solid_grid;
mod projected;
mod support;
