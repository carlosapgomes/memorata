use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, write_settings, ModelUnloadTimeout, TranscriptionBackend};
use serde::Serialize;
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
pub fn start_recording_session(app: AppHandle) -> Result<RecordingSessionState, String> {
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

    let Some(coordinator) = app.try_state::<crate::TranscriptionCoordinator>() else {
        return Err("coordinator_unavailable".to_string());
    };

    coordinator.start_session();
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
