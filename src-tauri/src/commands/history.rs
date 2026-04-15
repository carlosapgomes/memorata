use crate::actions::process_transcription_output;
use crate::managers::{
    history::{HistoryEntry, HistoryManager, PaginatedHistory},
    transcription_service::TranscriptionService,
};
use crate::settings::{get_settings, APPLE_INTELLIGENCE_PROVIDER_ID};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

/// Result of download_transcript_file operation
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DownloadResult {
    /// Final path where the file was saved
    pub path: String,
    /// Whether the file was saved to a user-selected location (true) or to the default recordings folder (false)
    pub user_selected: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn get_history_entries(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    cursor: Option<i64>,
    limit: Option<usize>,
) -> Result<PaginatedHistory, String> {
    history_manager
        .get_history_entries(cursor, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_history_entry_saved(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .toggle_saved_status(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_audio_file_path(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    file_name: String,
) -> Result<String, String> {
    let path = history_manager.get_audio_file_path(&file_name);
    path.to_str()
        .ok_or_else(|| "Invalid file path".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_history_entry(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .delete_entry(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn export_transcript_file(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<String, String> {
    let entry = history_manager
        .get_entry_by_id(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))?;

    let transcript_text = entry
        .post_processed_text
        .as_deref()
        .unwrap_or(&entry.transcription_text)
        .trim()
        .to_string();

    if transcript_text.is_empty() {
        return Err("No transcript available for this entry".to_string());
    }

    let artifact_path = history_manager
        .save_transcript_artifact(&entry.file_name, &transcript_text)
        .map_err(|e| e.to_string())?;

    artifact_path
        .to_str()
        .ok_or_else(|| "Invalid transcript file path".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn retry_history_entry_transcription(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    transcription_manager: State<'_, Arc<TranscriptionService>>,
    id: i64,
) -> Result<(), String> {
    let entry = history_manager
        .get_entry_by_id(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))?;

    let audio_path = history_manager.get_audio_file_path(&entry.file_name);
    let samples = crate::audio_toolkit::read_wav_samples(&audio_path)
        .map_err(|e| format!("Failed to load audio: {}", e))?;

    if samples.is_empty() {
        return Err("Recording has no audio samples".to_string());
    }

    transcription_manager.initiate_model_load();

    let transcription = transcription_manager
        .transcribe(samples, None)
        .await
        .map_err(|e| e.to_string())?;

    if transcription.is_empty() {
        return Err("Recording contains no speech".to_string());
    }

    let processed =
        process_transcription_output(&app, &transcription, entry.post_process_requested).await;
    history_manager
        .update_transcription(
            id,
            transcription,
            processed.post_processed_text,
            processed.post_process_prompt,
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn post_process_latest_history_entry(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
) -> Result<HistoryEntry, String> {
    let settings = get_settings(&app);

    if !settings.post_process_enabled {
        return Err("post_process_disabled".to_string());
    }

    let provider = settings
        .active_post_process_provider()
        .ok_or_else(|| "post_process_provider_missing".to_string())?;

    let model = settings
        .post_process_models
        .get(&provider.id)
        .map(|m| m.trim())
        .unwrap_or("");
    if model.is_empty() {
        return Err("post_process_model_missing".to_string());
    }

    let prompt_id = settings
        .post_process_selected_prompt_id
        .as_ref()
        .ok_or_else(|| "post_process_prompt_not_selected".to_string())?;

    let prompt = settings
        .post_process_prompts
        .iter()
        .find(|p| &p.id == prompt_id)
        .ok_or_else(|| "post_process_prompt_not_found".to_string())?;

    if prompt.prompt.trim().is_empty() {
        return Err("post_process_prompt_empty".to_string());
    }

    if provider.id != "custom" && provider.id != APPLE_INTELLIGENCE_PROVIDER_ID {
        let api_key = settings
            .post_process_api_keys
            .get(&provider.id)
            .map(|k| k.trim())
            .unwrap_or("");
        if api_key.is_empty() {
            return Err("post_process_api_key_missing".to_string());
        }
    }

    let entry = history_manager
        .get_latest_completed_entry()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "history_entry_not_found".to_string())?;

    let processed = process_transcription_output(&app, &entry.transcription_text, true).await;
    let post_processed_text = processed
        .post_processed_text
        .ok_or_else(|| "post_process_failed".to_string())?;

    history_manager
        .save_transcript_artifact(&entry.file_name, &processed.final_text)
        .map_err(|e| e.to_string())?;

    history_manager
        .update_post_process_result(
            entry.id,
            post_processed_text,
            processed.post_process_prompt,
            true,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_history_limit(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    limit: usize,
) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.history_limit = limit;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn update_recording_retention_period(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    period: String,
) -> Result<(), String> {
    use crate::settings::RecordingRetentionPeriod;

    let retention_period = match period.as_str() {
        "never" => RecordingRetentionPeriod::Never,
        "preserve_limit" => RecordingRetentionPeriod::PreserveLimit,
        "days3" => RecordingRetentionPeriod::Days3,
        "weeks2" => RecordingRetentionPeriod::Weeks2,
        "months3" => RecordingRetentionPeriod::Months3,
        _ => return Err(format!("Invalid retention period: {}", period)),
    };

    let mut settings = crate::settings::get_settings(&app);
    settings.recording_retention_period = retention_period;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Download a transcript file with a save dialog.
/// This is more reliable than the blob URL approach and gives the user control over where to save.
#[tauri::command]
#[specta::specta]
pub async fn download_transcript_file(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
    suggested_name: String,
) -> Result<DownloadResult, String> {
    let entry = history_manager
        .get_entry_by_id(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))?;

    let transcript_text = entry
        .post_processed_text
        .as_deref()
        .unwrap_or(&entry.transcription_text)
        .trim()
        .to_string();

    if transcript_text.is_empty() {
        return Err("No transcript available for this entry".to_string());
    }

    // Prepare suggested filename
    let suggested_filename = suggested_name
        .strip_suffix(".wav")
        .unwrap_or(&suggested_name)
        .to_string()
        + ".txt";

    // Show save dialog using the correct tauri-plugin-dialog v2 API
    let file_path = app
        .dialog()
        .file()
        .set_title("Save Transcript")
        .set_file_name(suggested_filename)
        .add_filter("Text File", &["txt"])
        .blocking_save_file();

    let Some(save_path) = file_path else {
        // User cancelled the dialog
        return Err("Download cancelled by user".to_string());
    };

    // Extract the PathBuf from FilePath enum
    let save_path = match save_path {
        tauri_plugin_dialog::FilePath::Url(_) => {
            return Err("URL paths are not supported for saving".to_string());
        }
        tauri_plugin_dialog::FilePath::Path(path) => path,
    };

    // Write the transcript to the chosen location
    std::fs::write(&save_path, &transcript_text).map_err(|e| {
        format!(
            "Failed to write transcript to {}: {}",
            save_path.display(),
            e
        )
    })?;

    Ok(DownloadResult {
        path: save_path.to_string_lossy().to_string(),
        user_selected: true,
    })
}
