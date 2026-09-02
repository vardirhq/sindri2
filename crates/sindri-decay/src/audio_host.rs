//! Audio requests from Decay without giving the language a device.
//!
//! Scripts enqueue intent. The game host drains that intent after a script
//! step and performs it through `sindri-platform::AudioBackend`. Keeping the
//! command between the language and the platform is what preserves Decay's
//! no-I/O boundary and still lets a silent backend assert sound in tests.

use decay_ir::Path;
use decay_runtime::{Host, RuntimeError, Value};

use crate::{Blackboard, ScriptContext, host::Spawning};

pub(crate) const AUDIO: &str = "Audio";

/// How many unperformed requests a runner keeps.
///
/// A caller with no audio device never drains, so without a bound a script
/// calling `Audio.play` every frame grows the queue for as long as the runner
/// lives — which is what the editor's play mode does. Audio intent goes stale
/// in milliseconds, so the newest requests are the ones worth keeping and the
/// oldest are dropped to make room.
const PENDING_LIMIT: usize = 256;

fn enqueue(queue: &mut Vec<AudioCommand>, command: AudioCommand) {
    if queue.len() >= PENDING_LIMIT {
        queue.remove(0);
    }
    queue.push(command);
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioCommand {
    Play { clip: String, volume: f32 },
    Loop { clip: String, volume: f32 },
    StopAll,
    PauseAll,
    ResumeAll,
}

/// The ordinary world host plus the `Audio.*` namespace.
///
/// The queue is borrowed rather than global. It was a thread-local, which meant
/// every caller of `Scripts::advance` shared one queue and only the game ever
/// emptied it: the editor pushed a command per `Audio.play` per frame of play
/// mode and nothing ever drained them. Whoever runs the scripts owns the
/// requests they produce.
pub struct WorldHost<'a> {
    inner: crate::host::WorldHost<'a>,
    audio: &'a mut Vec<AudioCommand>,
}

impl<'a> WorldHost<'a> {
    pub fn new(
        world: &'a mut sindri_core::World,
        entity: sindri_core::EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
        spawning: Spawning<'a>,
        physics: Option<crate::Physics2d<'a>>,
        audio: &'a mut Vec<AudioCommand>,
    ) -> Self {
        Self {
            inner: crate::host::WorldHost::new(
                world, entity, context, blackboard, spawning, physics,
            ),
            audio,
        }
    }

    pub fn take_printed(&mut self) -> Vec<String> {
        self.inner.take_printed()
    }
}

impl Host for WorldHost<'_> {
    fn load(&mut self, subject: Option<u64>, path: &Path) -> Result<Option<Value>, RuntimeError> {
        self.inner.load(subject, path)
    }

    fn store(
        &mut self,
        subject: Option<u64>,
        path: &Path,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        self.inner.store(subject, path, value)
    }

    fn call(
        &mut self,
        subject: Option<u64>,
        path: &Path,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        if subject.is_none() {
            let parts: Vec<&str> = path.0.iter().map(String::as_str).collect();
            if let [namespace, name] = parts.as_slice()
                && *namespace == AUDIO
            {
                return audio_call(self.audio, name, path, args).map(Some);
            }
        }
        self.inner.call(subject, path, args)
    }
}

fn normalized_volume(path: &Path, value: Option<&Value>) -> Result<f32, RuntimeError> {
    let Some(Value::Number(volume)) = value else {
        return Err(RuntimeError::Host(format!(
            "{} takes a volume number between 0 and 1, and the script gave {value:?}",
            path.dotted()
        )));
    };
    if !volume.is_finite() || !(0.0..=1.0).contains(volume) {
        return Err(RuntimeError::Host(format!(
            "{} takes a finite volume number between 0 and 1, and the script gave {volume}",
            path.dotted()
        )));
    }

    // Decay numbers are f64 while the platform audio boundary intentionally
    // uses f32, matching Rodio and the rest of the real-time audio path. The
    // normalized range above guarantees this conversion cannot overflow.
    #[allow(clippy::cast_possible_truncation)]
    let volume = *volume as f32;
    Ok(volume)
}

fn audio_call(
    queue: &mut Vec<AudioCommand>,
    name: &str,
    path: &Path,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match name {
        "play" | "loop" => {
            let Some(Value::String(clip)) = args.first() else {
                return Err(RuntimeError::Host(format!(
                    "{} takes an audio asset id as text",
                    path.dotted()
                )));
            };
            let volume = normalized_volume(path, args.get(1))?;
            let command = if name == "play" {
                AudioCommand::Play {
                    clip: clip.clone(),
                    volume,
                }
            } else {
                AudioCommand::Loop {
                    clip: clip.clone(),
                    volume,
                }
            };
            enqueue(queue, command);
            Ok(Value::Unit)
        }
        "stop_all" => {
            enqueue(queue, AudioCommand::StopAll);
            Ok(Value::Unit)
        }
        "pause_all" => {
            enqueue(queue, AudioCommand::PauseAll);
            Ok(Value::Unit)
        }
        "resume_all" => {
            enqueue(queue, AudioCommand::ResumeAll);
            Ok(Value::Unit)
        }
        _ => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use decay_ir::Path;
    use decay_runtime::{Host, RuntimeError, Value};
    use sindri_core::{EntityData, World};
    use sindri_platform::InputState;

    use super::{AudioCommand, WorldHost};
    use crate::{Blackboard, ScriptContext, host::Spawning};

    /// A spawning context for a test that is not about spawning.
    fn nothing_to_spawn() -> (
        crate::PrefabSources,
        std::collections::BTreeSet<sindri_core::EntityId>,
        Vec<sindri_core::EntityId>,
    ) {
        (
            crate::PrefabSources::new(),
            std::collections::BTreeSet::new(),
            Vec::new(),
        )
    }

    fn spawning<'a>(
        prefabs: &'a crate::PrefabSources,
        started: &'a std::collections::BTreeSet<sindri_core::EntityId>,
        spawned: &'a mut Vec<sindri_core::EntityId>,
    ) -> Spawning<'a> {
        Spawning {
            prefabs,
            started,
            spawned,
        }
    }

    #[test]
    fn audio_call_emits_intent_without_a_device() {
        let mut world = World::default();
        let entity = world.spawn(EntityData::default());
        let input = InputState::default();
        let mut board = Blackboard::new();
        let mut queue = Vec::new();
        let (prefabs, started, mut spawned) = nothing_to_spawn();
        let mut host = WorldHost::new(
            &mut world,
            entity,
            ScriptContext {
                input: &input,
                delta_seconds: 0.0,
                elapsed_seconds: 0.0,
            },
            &mut board,
            spawning(&prefabs, &started, &mut spawned),
            None,
            &mut queue,
        );
        host.call(
            None,
            &Path(vec!["Audio".to_owned(), "play".to_owned()]),
            &[
                Value::String("audio/pickup.wav".to_owned()),
                Value::Number(0.8),
            ],
        )
        .expect("audio call");
        assert_eq!(
            queue,
            [AudioCommand::Play {
                clip: "audio/pickup.wav".to_owned(),
                volume: 0.8,
            }]
        );
    }

    /// A runner nobody drains does not grow without end.
    #[test]
    fn an_undrained_queue_keeps_only_the_newest_requests() {
        let mut queue = Vec::new();
        for index in 0..(super::PENDING_LIMIT + 10) {
            super::enqueue(
                &mut queue,
                AudioCommand::Play {
                    clip: format!("audio/{index}.wav"),
                    volume: 1.0,
                },
            );
        }
        assert_eq!(queue.len(), super::PENDING_LIMIT);
        assert_eq!(
            queue.first(),
            Some(&AudioCommand::Play {
                clip: "audio/10.wav".to_owned(),
                volume: 1.0,
            }),
            "the oldest requests are the ones dropped"
        );
    }

    #[test]
    fn audio_call_rejects_volume_outside_normalized_range() {
        let mut world = World::default();
        let entity = world.spawn(EntityData::default());
        let input = InputState::default();
        let mut board = Blackboard::new();
        let mut queue = Vec::new();
        let (prefabs, started, mut spawned) = nothing_to_spawn();
        let mut host = WorldHost::new(
            &mut world,
            entity,
            ScriptContext {
                input: &input,
                delta_seconds: 0.0,
                elapsed_seconds: 0.0,
            },
            &mut board,
            spawning(&prefabs, &started, &mut spawned),
            None,
            &mut queue,
        );
        let error = host
            .call(
                None,
                &Path(vec!["Audio".to_owned(), "play".to_owned()]),
                &[
                    Value::String("audio/pickup.wav".to_owned()),
                    Value::Number(1.5),
                ],
            )
            .expect_err("volume outside 0..=1 must fail");
        assert!(matches!(
            error,
            RuntimeError::Host(message) if message.contains("between 0 and 1")
        ));
        assert!(queue.is_empty());
    }
}
