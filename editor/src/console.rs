//! What the editor has to say, in the order it said it.
//!
//! The console was three fixed lines, two of which interpolated a real number.
//! That made it a status readout wearing a log's clothes: nothing the engine
//! reported ever reached it, so a failed save, a render error, and a texture
//! reference nothing had bound all happened silently or flashed past in a
//! single-line notice.
//!
//! This is a log. It is bounded, because an editor runs for hours, and it
//! collapses a message repeated back to back into a count, because the thing
//! most worth logging — a render failure — recurs every frame and would
//! otherwise fill the buffer sixty times a second and push everything that
//! explains it out of the top.

use std::collections::VecDeque;

/// How many entries the console keeps.
///
/// A window rather than everything: the interesting entries are the recent
/// ones, and an editor left open overnight must not grow without bound.
const CAPACITY: usize = 200;

/// How much attention an entry is asking for.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Level {
    #[default]
    Info,
    /// Something is wrong with the scene, and the editor carried on anyway —
    /// a texture reference nothing has bound draws the magenta checker rather
    /// than failing the frame, and saying so is the only way anyone finds out.
    Warning,
    /// Something the user asked for did not happen.
    Error,
}

impl Level {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// One thing the editor said.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub level: Level,
    pub message: String,
    /// How many times in a row this exact message was recorded.
    ///
    /// One for a message said once. A render error repeats every frame, and a
    /// count is both shorter and more informative than two hundred copies.
    pub count: usize,
}

/// The editor's log.
#[derive(Clone, Debug, Default)]
pub struct Console {
    entries: VecDeque<Entry>,
}

impl Console {
    /// Records a message, or counts it again if it is the one just recorded.
    pub fn record(&mut self, level: Level, message: impl Into<String>) {
        let message = message.into();
        if let Some(last) = self.entries.back_mut()
            && last.level == level
            && last.message == message
        {
            last.count += 1;
            return;
        }
        self.entries.push_back(Entry {
            level,
            message,
            count: 1,
        });
        while self.entries.len() > CAPACITY {
            self.entries.pop_front();
        }
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.record(Level::Info, message);
    }

    pub fn warning(&mut self, message: impl Into<String>) {
        self.record(Level::Warning, message);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.record(Level::Error, message);
    }

    /// Oldest first, which is the order a log is read in.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &Entry> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// How many errors and warnings the log holds, which is what the status bar
    /// has been claiming since before anything counted.
    ///
    /// A repeated message counts once: "1 Error" about a render failure that
    /// recurs every frame is one thing wrong, and counting frames would put a
    /// five-figure number in the corner of the window.
    pub fn counts(&self) -> Counts {
        let mut counts = Counts::default();
        for entry in &self.entries {
            match entry.level {
                Level::Info => {}
                Level::Warning => counts.warnings += 1,
                Level::Error => counts.errors += 1,
            }
        }
        counts
    }
}

/// What the status bar reports at a glance.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Counts {
    pub errors: usize,
    pub warnings: usize,
}

impl Counts {
    /// The status bar's one line about them.
    pub fn summary(self) -> String {
        format!(
            "{} {}, {} {}",
            self.errors,
            if self.errors == 1 { "Error" } else { "Errors" },
            self.warnings,
            if self.warnings == 1 {
                "Warning"
            } else {
                "Warnings"
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(console: &Console) -> Vec<(Level, String, usize)> {
        console
            .entries()
            .map(|entry| (entry.level, entry.message.clone(), entry.count))
            .collect()
    }

    /// The property that makes a console survivable: a render error recurs
    /// every frame, and two hundred copies of it would push the entry that
    /// explains what happened before it out of the top.
    #[test]
    fn a_message_repeated_back_to_back_is_counted_rather_than_repeated() {
        let mut console = Console::default();
        for _ in 0..500 {
            console.error("Surface lost");
        }
        assert_eq!(
            messages(&console),
            [(Level::Error, "Surface lost".to_owned(), 500)]
        );
        assert_eq!(
            console.counts(),
            Counts {
                errors: 1,
                warnings: 0
            },
            "one thing is wrong, however many frames said so"
        );
    }

    /// Only back to back. The same failure after something else happened is a
    /// second occurrence and reads as one.
    #[test]
    fn the_same_message_after_another_is_a_new_entry() {
        let mut console = Console::default();
        console.error("Surface lost");
        console.info("Opened demo.scene.json");
        console.error("Surface lost");
        assert_eq!(
            messages(&console),
            [
                (Level::Error, "Surface lost".to_owned(), 1),
                (Level::Info, "Opened demo.scene.json".to_owned(), 1),
                (Level::Error, "Surface lost".to_owned(), 1),
            ]
        );
    }

    /// A message differing only in level is a different message: the same text
    /// logged as a warning and then an error means something changed.
    #[test]
    fn a_level_change_starts_a_new_entry() {
        let mut console = Console::default();
        console.warning("textures/badge.png is not bound");
        console.error("textures/badge.png is not bound");
        assert_eq!(console.entries().len(), 2);
    }

    /// An editor left open overnight must not grow without bound.
    #[test]
    fn the_log_keeps_the_recent_entries_and_drops_the_oldest() {
        let mut console = Console::default();
        for index in 0..(CAPACITY + 50) {
            console.info(format!("Entry {index}"));
        }
        assert_eq!(console.entries().len(), CAPACITY);
        assert_eq!(
            console.entries().next().map(|entry| entry.message.clone()),
            Some("Entry 50".to_owned()),
            "the window keeps the end of the log, not the beginning"
        );
    }

    #[test]
    fn the_status_summary_counts_what_the_log_holds() {
        let mut console = Console::default();
        assert_eq!(console.counts().summary(), "0 Errors, 0 Warnings");
        console.error("Could not save");
        assert_eq!(console.counts().summary(), "1 Error, 0 Warnings");
        console.warning("badge.png is not bound");
        console.warning("tiles.png is not bound");
        assert_eq!(console.counts().summary(), "1 Error, 2 Warnings");
        console.clear();
        assert_eq!(console.counts().summary(), "0 Errors, 0 Warnings");
        assert!(console.is_empty());
    }
}
