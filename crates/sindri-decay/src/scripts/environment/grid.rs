//! The typed `Grid` namespace a script compiles against.
//!
//! Its own file because it describes two models of a cell rather than one: a
//! flat map's palette index at a column and row, and a stacked volume's named
//! tile at a column, row and level. Keeping both beside every other namespace
//! made the largest surface in the environment the hardest one to read.

use decay_semantic::{Environment, FunctionType, HostType, Type};

use crate::surface::{ENTITY, GRID, GRID_CALLS, GridCall};

pub(crate) fn add_grid_surface(environment: &mut Environment) {
    let mut grid = HostType::new();
    for (name, call) in GRID_CALLS {
        grid = grid.with_function(
            *name,
            FunctionType {
                params: match call {
                    GridCall::PositionX | GridCall::PositionY => vec![
                        Type::Named(ENTITY.to_owned()),
                        Type::Named(ENTITY.to_owned()),
                    ],
                    GridCall::Place => vec![
                        Type::Named(ENTITY.to_owned()),
                        Type::Named(ENTITY.to_owned()),
                        Type::F32,
                        Type::F32,
                    ],
                    GridCall::CanReach | GridCall::StepToward => vec![
                        Type::Named(ENTITY.to_owned()),
                        Type::Named(ENTITY.to_owned()),
                        Type::Named(ENTITY.to_owned()),
                    ],
                    GridCall::Tile => vec![Type::Named(ENTITY.to_owned()), Type::F32, Type::F32],
                    // A flat cell and a palette index, and a stacked cell and
                    // its level, happen to be the same four arguments.
                    GridCall::SetTile | GridCall::Block => vec![
                        Type::Named(ENTITY.to_owned()),
                        Type::F32,
                        Type::F32,
                        Type::F32,
                    ],
                    GridCall::Columns | GridCall::Rows => {
                        vec![Type::Named(ENTITY.to_owned())]
                    }
                    GridCall::SetBlock => vec![
                        Type::Named(ENTITY.to_owned()),
                        Type::F32,
                        Type::F32,
                        Type::F32,
                        Type::String,
                    ],
                },
                return_type: match call {
                    GridCall::PositionX
                    | GridCall::PositionY
                    | GridCall::Tile
                    | GridCall::Columns
                    | GridCall::Rows => Type::F32,
                    GridCall::Block => Type::String,
                    GridCall::Place | GridCall::SetTile | GridCall::SetBlock => Type::Unit,
                    GridCall::CanReach | GridCall::StepToward => Type::Bool,
                },
            },
        );
    }
    environment.add_type(GRID, grid);
    environment.add_value(GRID, Type::Named(GRID.to_owned()));
}
