//! Audio requests from Decay without giving the language a device.
//!
//! Scripts enqueue intent. The game host drains that intent after a script
//! step and performs it through `sindri-platform::AudioBackend`. Keeping the
//! command between the language and the platform is what preserves Decay's
//! no-I/O boundary and still lets a silent backend assert sound in tests.

use std::cell::RefCell;

use decay_ir::Path;
use decay_runtime::{Host, RuntimeError, Value};

use crate::{Blackboard, ScriptContext};

pub(crate) const AUDIO: &str = "Audio";

#[derive(Clone, Debug, PartialEq)]
pub enum AudioCommand {
    Play { clip: String, volume: f32 },
    Loop { clip: String, volume: f32 },
    StopAll,
    PauseAll,
    ResumeAll,
}

thread_local! {
    static COMMANDS: RefCell<Vec<AudioCommand>> = const { RefCell::new(Vec::new()) };
}

/// Takes every audio request scripts have emitted since the previous drain.
///
/// Script execution is currently single-threaded. A thread-local queue keeps
/// separate worlds on separate runners from sharing commands without teaching
/// `World` about a platform service.
pub fn drain_audio_commands() -> Vec<AudioCommand> {
    COMMANDS.with(|commands| std::mem::take(&mut *commands.borrow_mut()))
}

fn push(command: AudioCommand) {
    COMMANDS.with(|commands| commands.borrow_mut().push(command));
}

/// The ordinary world host plus the `Audio.*` namespace.
pub struct WorldHost<'a> {
    inner: crate::host::WorldHost<'a>,
}

impl<'a> WorldHost<'a> {
    pub fn new(
        world: &'a mut sindri_core::World,
        entity: sindri_core::EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
    ) -> Self {
        Self {
            inner: crate::host::WorldHost::new(world, entity, context, blackboard),
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
                return audio_call(name, path, args).map(Some);
            }
        }
        self.inner.call(subject, path, args)
    }
}

fn audio_call(name: &str, path: &Path, args: &[Value]) -> Result<Value, RuntimeError> {
    match name {
        "play" | "loop" => {
            let Some(Value::String(clip)) = args.first() else {
                return Err(RuntimeError::Host(format!(
                    "{} takes an audio asset id as text",
                    path.dotted()
                )));
            };
            let volume = match args.get(1) {
                Some(Value::Number(volume)) if volume.is_finite() => *volume as f32,
                other => {
                    return Err(RuntimeError::Host(format!(
                        "{} takes a finite volume number, and the script gave {other:?}",
                        path.dotted()
                    )));
                }
            };
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
            push(command);
            Ok(Value::Unit)
        }
        "stop_all" => {
            push(AudioCommand::StopAll);
            Ok(Value::Unit)
        }
        "pause_all" => {
            push(AudioCommand::PauseAll);
            Ok(Value::Unit)
        }
        "resume_all" => {
            push(AudioCommand::ResumeAll);
            Ok(Value::Unit)
        }
        _ => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use decay_ir::Path;
    use decay_runtime::{Host, Value};
    use sindri_core::{EntityData, World};
    use sindri_platform::InputState;

    use super::{AudioCommand, WorldHost, drain_audio_commands};
    use crate::{Blackboard, ScriptContext};

    #[test]
    fn audio_call_emits_intent_without_a_device() {
        let _ = drain_audio_commands();
        let mut world = World::default();
        let entity = world.spawn(EntityData::default());
        let input = InputState::default();
        let mut board = Blackboard::new();
        let mut host = WorldHost::new(
            &mut world,
            entity,
            ScriptContext {
                input: &input,
                delta_seconds: 0.0,
                elapsed_seconds: 0.0,
            },
            &mut board,
        );
        host.call(
            None,
            &Path(vec!["Audio".to_owned(), "play".to_owned()]),
            &[Value::String("audio/pickup.wav".to_owned()), Value::Number(0.8)],
        )
        .expect("audio call");
        assert_eq!(
            drain_audio_commands(),
            [AudioCommand::Play {
                clip: "audio/pickup.wav".to_owned(),
                volume: 0.8,
            }]
        );
    }
}
