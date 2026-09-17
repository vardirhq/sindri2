//! Standing everywhere on the farm and checking what gets drawn over you.
//!
//! The tests beside this one put a walker in one place and assert one thing
//! about it. That is how the bug this sweep exists for reached a phone: the
//! ordering was right everywhere anybody had thought to look, and wrong on the
//! open ground where the player actually walks.
//!
//! So this stands on every column of the island in turn -- all six hundred and
//! something of them -- and asks the renderer's own two questions at each one.
//! A cell whose ground is no higher than the probe's feet cannot cover it, and
//! if such a cell is drawn after the probe anyway, that is the farm reproducing
//! the bug rather than a unit test reproducing it.

use sindri_gather::{bind_tile_sets, extractor, world};
use sindri_scene::{OcclusionProbe, sweep_occlusion};

/// The player, near enough: `player.png` drawn at the scale the scene gives it.
///
/// Its own bounds rather than the default probe, because how far an actor
/// reaches is what decides which cells can reach it back, and a sweep run with
/// the wrong actor proves something about a different game.
fn player() -> OcclusionProbe {
    OcclusionProbe {
        size: [0.62, 0.62],
        offset: [0.0, 0.31],
    }
}

#[test]
fn nothing_the_walker_stands_above_is_drawn_over_it() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let tile_sets = bind_tile_sets().expect("the tile sets decode");

    let report = sweep_occlusion(&world, extractor.components(), &tile_sets, player())
        .expect("the farm sweeps");

    // Fewer than the volume's six hundred and sixty cells, and it should be:
    // water supports without being walkable, so the moat and the pond hold a
    // boat up and nobody stands on them, and the outcrop stacks two cells into
    // one column.
    assert!(
        report.probes > 350,
        "the sweep should stand everywhere on the island, not in a corner of \
         it: {} probes",
        report.probes
    );
    let unexplained = report.unexplained().collect::<Vec<_>>();
    assert!(
        unexplained.is_empty(),
        "{} of {} probes found ground drawn over the walker with nothing to \
         account for it; first few: {:?}",
        unexplained.len(),
        report.probes,
        unexplained.iter().take(5).collect::<Vec<_>>()
    );

    // The farm does hit the documented corner, in twenty-two places, all of
    // them a walker standing beside the raised edge of a plot. One depth cannot
    // cover the walker with the wall and leave the open ground behind them at
    // the same time, and it chooses the wall.
    //
    // A budget rather than zero, because zero would be a lie and a silent
    // sweep would be worse than a counted one. If this number grows, something
    // made the corner more common; if it falls, something fixed it, and either
    // way it should be noticed here. `docs/parity.md` carries the row, and the
    // way out is ordering derived per overlapping pair rather than per entity.
    assert!(
        report.findings.len() <= 22,
        "the corner between a wall and open ground is reached {} times, up \
         from twenty-two: {:?}",
        report.findings.len(),
        report.findings.iter().take(5).collect::<Vec<_>>()
    );
}

/// A taller actor reaches further and is the harder question.
///
/// Not asserted clean: the default probe is two cells tall, which reaches cells
/// the rule deliberately does not consult, and `docs/parity.md` carries that
/// row. What is asserted is that the sweep runs over the real farm and that a
/// taller actor never finds *fewer* problems than a short one, since the area
/// it covers strictly contains theirs.
#[test]
fn a_taller_actor_is_swept_too() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let tile_sets = bind_tile_sets().expect("the tile sets decode");

    let short = sweep_occlusion(&world, extractor.components(), &tile_sets, player())
        .expect("the farm sweeps");
    let tall = sweep_occlusion(
        &world,
        extractor.components(),
        &tile_sets,
        OcclusionProbe::default(),
    )
    .expect("the farm sweeps");

    assert_eq!(short.probes, tall.probes, "both stand in the same places");
    assert!(
        tall.findings.len() >= short.findings.len(),
        "a taller actor covers everything a shorter one does: {} vs {}",
        tall.findings.len(),
        short.findings.len()
    );
}
