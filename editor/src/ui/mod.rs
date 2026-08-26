//! The Sindri editor's design system.
//!
//! Three things live here and nothing else: the tokens every panel draws with
//! (`theme`), the icon vocabulary they share (`icons`), and the controls built
//! from both (`widgets`). Nothing in here knows what a scene, an entity, or a
//! command is — it is the layer the panels sit on, so a panel can be read as
//! *what it does* rather than as a list of colours and offsets.
//!
//! The rule that keeps it worth having: a panel does not name a colour, a gap,
//! or a font size of its own. If it needs one that is not here, the token is
//! missing and belongs in `theme`.

pub mod icons;
pub mod theme;
pub mod widgets;
