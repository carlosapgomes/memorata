use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, write_settings, ModelUnloadTimeout, TranscriptionBackend};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};

#[derive(Serialize, Type)]
pub struct ModelLoadStatus {
    is_loaded: bool,
    current_model: Option<String>,
}

#[derive(Serialize, Type, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordingSessionState {
    Idle,
    Recording,
    Paused,
    Processing,
}

/// Session options captured at session start for diarization configuration.
/// These are snapshotted when Start is called and used throughout the session.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct StartSessionOptions {
    /// Whether speaker diarization is enabled for this session.
    pub enable_diarization: bool,
    /// Number of expected speakers (minimum 2 if diarization enabled, forced to 1 if disabled).
    pub speakers_expected: u8,
}

impl Default for StartSessionOptions {
    fn default() -> Self {
        Self {
            enable_diarization: true,
            speakers_expected: 2,
        }
    }
}

impl StartSessionOptions {
    /// Creates default options with diarization enabled and 2 speakers.
    pub fn new() -> Self {
        Self {
            enable_diarization: true,
            speakers_expected: 2,
        }
    }

    /// Validates the session options.
    /// Returns Err if diarization is enabled but speakers_expected < 2.
    /// Normalizes speakers_expected to 1 if diarization is disabled.
    pub fn validate(&mut self) -> Result<(), String> {
        if self.enable_diarization {
            if self.speakers_expected < 2 {
                return Err("assembly_ai_speakers_expected_invalid".to_string());
            }
        } else {
            // Force speakers to 1 when diarization is disabled
            self.speakers_expected = 1;
        }
        Ok(())
    }
}

fn get_recording_state(app: &AppHandle) -> RecordingSessionState {
    if let Some(coordinator) = app.try_state::<crate::TranscriptionCoordinator>() {
        if coordinator.is_processing() {
            return RecordingSessionState::Processing;
        }
        if coordinator.is_paused() {
            return RecordingSessionState::Paused;
        }
        if coordinator.is_recording() {
            return RecordingSessionState::Recording;
        }
    }

    RecordingSessionState::Idle
}

fn validate_session_start_preconditions(app: &AppHandle) -> Result<(), String> {
    let settings = get_settings(app);

    match settings.transcription_backend {
        TranscriptionBackend::AssemblyAi => {
            if settings.assembly_ai_api_key.trim().is_empty() {
                return Err("assembly_ai_api_key_missing".to_string());
            }

            let base_url = settings.assembly_ai_base_url.trim();
            if base_url.is_empty()
                || !(base_url.starts_with("http://") || base_url.starts_with("https://"))
            {
                return Err("assembly_ai_base_url_invalid".to_string());
            }

            // Validate language code
            if crate::managers::transcription_service::validate_assembly_ai_language_code(
                &settings.assembly_ai_language_code,
            )
            .is_err()
            {
                return Err("assembly_ai_language_code_invalid".to_string());
            }
        }
        TranscriptionBackend::Local => {
            if settings.selected_model.trim().is_empty() {
                return Err("local_model_not_selected".to_string());
            }
        }
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_model_unload_timeout(app: AppHandle, timeout: ModelUnloadTimeout) {
    let mut settings = get_settings(&app);
    settings.model_unload_timeout = timeout;
    write_settings(&app, settings);
}

#[tauri::command]
#[specta::specta]
pub fn get_model_load_status(
    transcription_manager: State<TranscriptionManager>,
) -> Result<ModelLoadStatus, String> {
    Ok(ModelLoadStatus {
        is_loaded: transcription_manager.is_model_loaded(),
        current_model: transcription_manager.get_current_model(),
    })
}

#[tauri::command]
#[specta::specta]
pub fn unload_model_manually(
    transcription_manager: State<TranscriptionManager>,
) -> Result<(), String> {
    transcription_manager
        .unload_model()
        .map_err(|e| format!("Failed to unload model: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn get_recording_session_state(app: AppHandle) -> RecordingSessionState {
    get_recording_state(&app)
}

#[tauri::command]
#[specta::specta]
pub fn start_recording_session(
    app: AppHandle,
    options: Option<StartSessionOptions>,
) -> Result<RecordingSessionState, String> {
    match get_recording_state(&app) {
        RecordingSessionState::Processing => {
            return Err("processing_in_progress".to_string());
        }
        RecordingSessionState::Recording => {
            return Ok(RecordingSessionState::Recording);
        }
        RecordingSessionState::Paused => {
            return Err("session_paused_use_resume".to_string());
        }
        RecordingSessionState::Idle => {}
    }

    validate_session_start_preconditions(&app)?;

    // Validate and normalize session options
    let mut session_options = options.unwrap_or_default();
    session_options.validate()?;

    let Some(coordinator) = app.try_state::<crate::TranscriptionCoordinator>() else {
        return Err("coordinator_unavailable".to_string());
    };

    coordinator.start_session_with_options(session_options);
    Ok(RecordingSessionState::Recording)
}

#[tauri::command]
#[specta::specta]
pub fn pause_recording_session(app: AppHandle) -> Result<RecordingSessionState, String> {
    match get_recording_state(&app) {
        RecordingSessionState::Processing => {
            return Err("processing_in_progress".to_string());
        }
        RecordingSessionState::Idle => {
            return Err("recording_not_active".to_string());
        }
        RecordingSessionState::Paused => {
            return Ok(RecordingSessionState::Paused);
        }
        RecordingSessionState::Recording => {}
    }

    let Some(coordinator) = app.try_state::<crate::TranscriptionCoordinator>() else {
        return Err("coordinator_unavailable".to_string());
    };

    coordinator.pause_session();
    Ok(RecordingSessionState::Paused)
}

#[tauri::command]
#[specta::specta]
pub fn resume_recording_session(app: AppHandle) -> Result<RecordingSessionState, String> {
    match get_recording_state(&app) {
        RecordingSessionState::Processing => {
            return Err("processing_in_progress".to_string());
        }
        RecordingSessionState::Idle => {
            return Err("recording_not_active".to_string());
        }
        RecordingSessionState::Recording => {
            return Ok(RecordingSessionState::Recording);
        }
        RecordingSessionState::Paused => {}
    }

    let Some(coordinator) = app.try_state::<crate::TranscriptionCoordinator>() else {
        return Err("coordinator_unavailable".to_string());
    };

    coordinator.resume_session();
    Ok(RecordingSessionState::Recording)
}

#[tauri::command]
#[specta::specta]
pub fn stop_recording_session(app: AppHandle) -> Result<RecordingSessionState, String> {
    match get_recording_state(&app) {
        RecordingSessionState::Processing => {
            return Err("processing_in_progress".to_string());
        }
        RecordingSessionState::Idle => {
            return Err("recording_not_active".to_string());
        }
        RecordingSessionState::Recording | RecordingSessionState::Paused => {}
    }

    let Some(coordinator) = app.try_state::<crate::TranscriptionCoordinator>() else {
        return Err("coordinator_unavailable".to_string());
    };

    coordinator.stop_session();
    Ok(RecordingSessionState::Processing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_session_options_default() {
        let opts = StartSessionOptions::default();
        assert!(opts.enable_diarization);
        assert_eq!(opts.speakers_expected, 2);
    }

    #[test]
    fn start_session_options_validate_diarization_on_valid() {
        let mut opts = StartSessionOptions {
            enable_diarization: true,
            speakers_expected: 2,
        };
        assert!(opts.validate().is_ok());
        assert_eq!(opts.speakers_expected, 2);

        let mut opts = StartSessionOptions {
            enable_diarization: true,
            speakers_expected: 5,
        };
        assert!(opts.validate().is_ok());
        assert_eq!(opts.speakers_expected, 5);
    }

    #[test]
    fn start_session_options_validate_diarization_on_invalid() {
        let mut opts = StartSessionOptions {
            enable_diarization: true,
            speakers_expected: 1,
        };
        assert!(opts.validate().is_err());
        assert_eq!(
            opts.validate().unwrap_err(),
            "assembly_ai_speakers_expected_invalid"
        );

        let mut opts = StartSessionOptions {
            enable_diarization: true,
            speakers_expected: 0,
        };
        assert!(opts.validate().is_err());
    }

    #[test]
    fn start_session_options_validate_diarization_off_forces_one() {
        let mut opts = StartSessionOptions {
            enable_diarization: false,
            speakers_expected: 5,
        };
        assert!(opts.validate().is_ok());
        assert_eq!(opts.speakers_expected, 1);

        let mut opts = StartSessionOptions {
            enable_diarization: false,
            speakers_expected: 2,
        };
        assert!(opts.validate().is_ok());
        assert_eq!(opts.speakers_expected, 1);
    }
}
