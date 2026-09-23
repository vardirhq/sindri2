//! Drawing around a component that cannot be drawn.
//!
//! Extraction used to be all or nothing: one invalid environment or voxel
//! world failed the frame. A game that ships such a scene should fail loudly,
//! and still does. An editor is different. The invalid value is the one
//! somebody is in the middle of typing or dragging, and failing the frame left
//! the viewport showing its last good image while the gizmos drawn over it kept
//! moving -- a scene that looked frozen, with nothing on it saying why.
//!
//! So tolerance is something a caller asks for. With it, a component that
//! cannot be used is drawn as it last could be, or left out, and the reason is
//! recorded as a problem about that entity for the caller to show.

use sindri_core::{EntityId, SceneComponent, World};

use super::{SceneExtractError, SceneExtractor};
use crate::{EnvironmentComponent, EnvironmentError, environments_in};

/// A component the extractor could not use this frame, and drew around.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractProblem {
    /// The entity carrying the component.
    pub entity: EntityId,
    /// The component's type name, such as `sindri.voxel_world`.
    pub component: &'static str,
    /// What was wrong, in the words the component's own error uses.
    pub message: String,
}

impl SceneExtractor {
    /// Draws around invalid environments and voxel worlds instead of failing.
    ///
    /// For editors, where a value is often invalid for the moment it takes to
    /// finish typing it. Each one skipped is reported by [`Self::problems`]
    /// after the frame that skipped it.
    pub fn tolerate_invalid_components(&mut self) {
        self.tolerant = true;
    }

    pub(super) const fn tolerant(&self) -> bool {
        self.tolerant
    }

    /// What the most recent extraction drew around, oldest first.
    ///
    /// Empty unless [`Self::tolerate_invalid_components`] was called: without
    /// it, the same failures fail the frame instead.
    pub fn problems(&self) -> Vec<ExtractProblem> {
        self.problems.borrow().clone()
    }

    /// Forgets the previous frame's problems.
    pub(super) fn begin_frame(&self) {
        self.problems.borrow_mut().clear();
    }

    /// Records a component drawn around, once per frame however often it is
    /// asked about.
    pub(super) fn record(
        &self,
        entity: EntityId,
        component: &'static str,
        error: &dyn std::fmt::Display,
    ) {
        let problem = ExtractProblem {
            entity,
            component,
            message: error.to_string(),
        };
        let mut problems = self.problems.borrow_mut();
        if !problems.contains(&problem) {
            problems.push(problem);
        }
    }

    /// The environment a frame of `world` is drawn with.
    ///
    /// Strictly, this is [`crate::environment_of`]. Tolerantly, an invalid
    /// environment is recorded as a problem and replaced by the last valid one
    /// the same entity held, so dragging a value out of range leaves the light
    /// as it was rather than resetting it; an entity with no valid past falls
    /// back to no environment. A second environment is recorded and ignored.
    pub fn environment(
        &self,
        world: &World,
    ) -> Result<Option<EnvironmentComponent>, SceneExtractError> {
        if !self.tolerant {
            return Ok(crate::environment_of(world)?);
        }
        let mut chosen = None;
        let mut seen = false;
        for (entity, environment) in environments_in(world) {
            if seen {
                self.record(
                    entity,
                    EnvironmentComponent::TYPE_NAME,
                    &EnvironmentError::MultipleEnvironments,
                );
                continue;
            }
            seen = true;
            let mut last = self.last_environment.borrow_mut();
            match environment {
                Ok(environment) => {
                    *last = Some((entity, environment));
                    chosen = Some(environment);
                }
                Err(error) => {
                    self.record(entity, EnvironmentComponent::TYPE_NAME, &error);
                    chosen = last
                        .filter(|(remembered, _)| *remembered == entity)
                        .map(|(_, environment)| environment);
                }
            }
        }
        Ok(chosen)
    }
}
