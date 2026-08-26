//! Renderer-independent coordinates and projection math for grid-based games.
//!
//! This crate deliberately knows nothing about scenes, cameras, sprites, or
//! editors. [`GridSpace`] maps a logical grid onto a two-dimensional plane; a
//! consumer decides whether that plane is world XY, world XZ, or screen space.

mod geometry;
mod occupancy;
mod pathfinding;
mod walls;

pub use geometry::{
    GridBounds, GridCoord, GridError, GridPoint, GridSpace, PlanePoint, PlaneYAxis, Projection,
};
pub use occupancy::{FootprintError, GridFootprint, GridOccupancy, GridPlacementError};
pub use pathfinding::{GridMovement, GridPath, GridPathCosts, GridPathError, GridPathfinder};
pub use walls::{GridWallEdge, GridWallError, GridWalls};
