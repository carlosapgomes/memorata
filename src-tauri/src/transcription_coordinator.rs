use crate::actions::ACTION_MAP;
use crate::commands::transcription::StartSessionOptions;
use crate::managers::audio::AudioRecordingManager;
use log::{debug, error, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const DEBOUNCE: Duration = Duration::from_millis(30);

/// Commands processed sequentially by the coordinator thread.
enum Command {
    Input {
        binding_id: String,
        hotkey_string: String,
        is_pressed: bool,
        push_to_talk: bool,
    },
    StartSession {
        options: StartSessionOptions,
    },
    PauseSession,
    ResumeSession,
    StopSession,
    Cancel {
        recording_was_active: bool,
    },
    ProcessingFinished,
}

/// Pipeline lifecycle, owned exclusively by the coordinator thread.
enum Stage {
    Idle,
    Recording(String), // binding_id
    Paused(String),
    Processing,
}

/// Serialises all transcription lifecycle events through a single thread
/// to eliminate race conditions between keyboard shortcuts, signals, and
/// the async transcribe-paste pipeline.
pub struct TranscriptionCoordinator {
    tx: Sender<Command>,
    is_recording: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
    is_processing: Arc<AtomicBool>,
    /// Session options snapshotted at session start.
    /// Uses RwLock for safe concurrent access from both coordinator thread and transcription service.
    session_options: Arc<RwLock<Option<StartSessionOptions>>>,
}

pub fn is_transcribe_binding(id: &str) -> bool {
    id == "transcribe" || id == "transcribe_with_post_process"
}

impl TranscriptionCoordinator {
    pub fn new(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel();
        let is_recording = Arc::new(AtomicBool::new(false));
        let is_paused = Arc::new(AtomicBool::new(false));
        let is_processing = Arc::new(AtomicBool::new(false));
        let session_options = Arc::new(RwLock::new(None));

        let is_recording_clone = Arc::clone(&is_recording);
        let is_paused_clone = Arc::clone(&is_paused);
        let is_processing_clone = Arc::clone(&is_processing);
        let session_options_clone = Arc::clone(&session_options);

        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut stage = Stage::Idle;
                let mut last_press: Option<Instant> = None;
                update_stage_flags(
                    &stage,
                    &is_recording_clone,
                    &is_paused_clone,
                    &is_processing_clone,
                );

                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        Command::Input {
                            binding_id,
                            hotkey_string,
                            is_pressed,
                            push_to_talk,
                        } => {
                            // Debounce rapid-fire press events (key repeat / double-tap).
                            // Releases always pass through for push-to-talk.
                            if is_pressed {
                                let now = Instant::now();
                                if last_press.map_or(false, |t| now.duration_since(t) < DEBOUNCE) {
                                    debug!("Debounced press for '{binding_id}'");
                                    continue;
                                }
                                last_press = Some(now);
                            }

                            if push_to_talk {
                                if is_pressed && matches!(stage, Stage::Idle) {
                                    start(&app, &mut stage, &binding_id, &hotkey_string);
                                } else if !is_pressed
                                    && matches!(&stage, Stage::Recording(id) if id == &binding_id)
                                {
                                    stop(&app, &mut stage, &binding_id, &hotkey_string);
                                }
                            } else if is_pressed {
                                match &stage {
                                    Stage::Idle => {
                                        start(&app, &mut stage, &binding_id, &hotkey_string);
                                    }
                                    Stage::Recording(id) if id == &binding_id => {
                                        stop(&app, &mut stage, &binding_id, &hotkey_string);
                                    }
                                    _ => {
                                        debug!("Ignoring press for '{binding_id}': pipeline busy")
                                    }
                                }
                            }
                        }
                        Command::StartSession { options } => {
                            if matches!(stage, Stage::Idle) {
                                start(&app, &mut stage, "transcribe", "UI:start");

                                // Keep snapshot only if start actually entered Recording.
                                if let Ok(mut opts) = session_options_clone.write() {
                                    if matches!(stage, Stage::Recording(_)) {
                                        *opts = Some(options);
                                    } else {
                                        *opts = None;
                                    }
                                }
                            }
                        }
                        Command::PauseSession => {
                            let active_binding = match &stage {
                                Stage::Recording(binding_id) => Some(binding_id.clone()),
                                _ => None,
                            };
                            if let Some(binding_id) = active_binding {
                                pause(&app, &mut stage, &binding_id);
                            }
                        }
                        Command::ResumeSession => {
                            let active_binding = match &stage {
                                Stage::Paused(binding_id) => Some(binding_id.clone()),
                                _ => None,
                            };
                            if let Some(binding_id) = active_binding {
                                resume(&app, &mut stage, &binding_id);
                            }
                        }
                        Command::StopSession => {
                            let active_binding = match &stage {
                                Stage::Recording(binding_id) | Stage::Paused(binding_id) => {
                                    Some(binding_id.clone())
                                }
                                _ => None,
                            };
                            if let Some(binding_id) = active_binding {
                                stop(&app, &mut stage, &binding_id, "UI:stop");
                            }
                        }
                        Command::Cancel {
                            recording_was_active,
                        } => {
                            // Don't reset during processing — wait for the pipeline to finish.
                            if !matches!(stage, Stage::Processing)
                                && (recording_was_active
                                    || matches!(stage, Stage::Recording(_) | Stage::Paused(_)))
                            {
                                stage = Stage::Idle;
                                // Clear session options on cancel
                                if let Ok(mut opts) = session_options_clone.write() {
                                    *opts = None;
                                }
                            }
                        }
                        Command::ProcessingFinished => {
                            stage = Stage::Idle;
                            // Clear session options when processing finishes
                            if let Ok(mut opts) = session_options_clone.write() {
                                *opts = None;
                            }
                        }
                    }

                    update_stage_flags(
                        &stage,
                        &is_recording_clone,
                        &is_paused_clone,
                        &is_processing_clone,
                    );
                }
                debug!("Transcription coordinator exited");
            }));
            if let Err(e) = result {
                error!("Transcription coordinator panicked: {e:?}");
            }
        });

        Self {
            tx,
            is_recording,
            is_paused,
            is_processing,
            session_options,
        }
    }

    /// Send a keyboard/signal input event for a transcribe binding.
    /// For signal-based toggles, use `is_pressed: true` and `push_to_talk: false`.
    pub fn send_input(
        &self,
        binding_id: &str,
        hotkey_string: &str,
        is_pressed: bool,
        push_to_talk: bool,
    ) {
        if self
            .tx
            .send(Command::Input {
                binding_id: binding_id.to_string(),
                hotkey_string: hotkey_string.to_string(),
                is_pressed,
                push_to_talk,
            })
            .is_err()
        {
            warn!("Transcription coordinator channel closed");
        }
    }

    /// Start a session with explicit options (from UI).
    pub fn start_session_with_options(&self, options: StartSessionOptions) {
        if self
            .tx
            .send(Command::StartSession { options })
            .is_err()
        {
            warn!("Transcription coordinator channel closed");
        }
    }

    /// Start a session with default options (for keyboard shortcut compatibility).
    pub fn start_session(&self) {
        let options = StartSessionOptions::default();
        self.start_session_with_options(options);
    }

    pub fn pause_session(&self) {
        if self.tx.send(Command::PauseSession).is_err() {
            warn!("Transcription coordinator channel closed");
        }
    }

    pub fn resume_session(&self) {
        if self.tx.send(Command::ResumeSession).is_err() {
            warn!("Transcription coordinator channel closed");
        }
    }

    pub fn stop_session(&self) {
        if self.tx.send(Command::StopSession).is_err() {
            warn!("Transcription coordinator channel closed");
        }
    }

    pub fn notify_cancel(&self, recording_was_active: bool) {
        if self
            .tx
            .send(Command::Cancel {
                recording_was_active,
            })
            .is_err()
        {
            warn!("Transcription coordinator channel closed");
        }
    }

    pub fn notify_processing_finished(&self) {
        if self.tx.send(Command::ProcessingFinished).is_err() {
            warn!("Transcription coordinator channel closed");
        }
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::Relaxed)
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::Relaxed)
    }

    pub fn is_processing(&self) -> bool {
        self.is_processing.load(Ordering::Relaxed)
    }

    /// Get the current session options snapshot.
    /// Returns None if no session is active or options haven't been set.
    pub fn get_session_options(&self) -> Option<StartSessionOptions> {
        self.session_options.read().ok()?.clone()
    }
}

fn update_stage_flags(
    stage: &Stage,
    is_recording: &AtomicBool,
    is_paused: &AtomicBool,
    is_processing: &AtomicBool,
) {
    match stage {
        Stage::Idle => {
            is_recording.store(false, Ordering::Relaxed);
            is_paused.store(false, Ordering::Relaxed);
            is_processing.store(false, Ordering::Relaxed);
        }
        Stage::Recording(_) => {
            is_recording.store(true, Ordering::Relaxed);
            is_paused.store(false, Ordering::Relaxed);
            is_processing.store(false, Ordering::Relaxed);
        }
        Stage::Paused(_) => {
            is_recording.store(false, Ordering::Relaxed);
            is_paused.store(true, Ordering::Relaxed);
            is_processing.store(false, Ordering::Relaxed);
        }
        Stage::Processing => {
            is_recording.store(false, Ordering::Relaxed);
            is_paused.store(false, Ordering::Relaxed);
            is_processing.store(true, Ordering::Relaxed);
        }
    }
}

fn start(app: &AppHandle, stage: &mut Stage, binding_id: &str, hotkey_string: &str) {
    let Some(action) = ACTION_MAP.get(binding_id) else {
        warn!("No action in ACTION_MAP for '{binding_id}'");
        return;
    };
    action.start(app, binding_id, hotkey_string);
    if app
        .try_state::<Arc<AudioRecordingManager>>()
        .map_or(false, |a| a.is_recording())
    {
        *stage = Stage::Recording(binding_id.to_string());
    } else {
        debug!("Start for '{binding_id}' did not begin recording; staying idle");
    }
}

fn pause(app: &AppHandle, stage: &mut Stage, binding_id: &str) {
    if let Some(audio_manager) = app.try_state::<Arc<AudioRecordingManager>>() {
        match audio_manager.pause_recording(binding_id) {
            Ok(()) => {
                *stage = Stage::Paused(binding_id.to_string());
                crate::overlay::show_paused_overlay(app);
            }
            Err(err) => {
                warn!("Failed to pause session for '{binding_id}': {err}");
            }
        }
    }
}

fn resume(app: &AppHandle, stage: &mut Stage, binding_id: &str) {
    if let Some(audio_manager) = app.try_state::<Arc<AudioRecordingManager>>() {
        match audio_manager.resume_recording(binding_id) {
            Ok(()) => {
                *stage = Stage::Recording(binding_id.to_string());
                crate::overlay::show_recording_overlay(app);
            }
            Err(err) => {
                warn!("Failed to resume session for '{binding_id}': {err}");
            }
        }
    }
}

fn stop(app: &AppHandle, stage: &mut Stage, binding_id: &str, hotkey_string: &str) {
    let Some(action) = ACTION_MAP.get(binding_id) else {
        warn!("No action in ACTION_MAP for '{binding_id}'");
        return;
    };
    action.stop(app, binding_id, hotkey_string);
    *stage = Stage::Processing;
}
