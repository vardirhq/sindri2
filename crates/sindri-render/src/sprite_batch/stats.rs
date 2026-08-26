//! What a frame's batching actually did, for anything asserting on it.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpriteBatchStats {
    sprite_count: u32,
    draw_calls: u32,
}

impl SpriteBatchStats {
    pub(super) fn for_sprite_count(sprite_count: u32) -> Self {
        Self {
            sprite_count,
            draw_calls: u32::from(sprite_count > 0),
        }
    }

    pub const fn sprite_count(self) -> u32 {
        self.sprite_count
    }

    pub const fn draw_calls(self) -> u32 {
        self.draw_calls
    }

    pub const fn draw_calls_saved(self) -> u32 {
        self.sprite_count.saturating_sub(self.draw_calls)
    }

    /// One frame's running total, batch by batch.
    pub(super) const fn and(self, other: Self) -> Self {
        Self {
            sprite_count: self.sprite_count.saturating_add(other.sprite_count),
            draw_calls: self.draw_calls.saturating_add(other.draw_calls),
        }
    }
}
