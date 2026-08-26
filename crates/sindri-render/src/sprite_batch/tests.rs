//! What the depth key sorts by.

use super::*;

#[test]
fn empty_batch_emits_no_draw_calls() {
    let stats = SpriteBatchStats::for_sprite_count(0);
    assert_eq!(stats.draw_calls(), 0);
    assert_eq!(stats.draw_calls_saved(), 0);
}

#[test]
fn batch_reduces_many_sprites_to_one_draw_call() {
    let stats = SpriteBatchStats::for_sprite_count(128);
    assert_eq!(stats.sprite_count(), 128);
    assert_eq!(stats.draw_calls(), 1);
    assert_eq!(stats.draw_calls_saved(), 127);
}
