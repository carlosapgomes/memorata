use crate::commands::transcription::StartSessionOptions;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, TranscriptionBackend};
use anyhow::{anyhow, Context, Result};
use hound::{SampleFormat, WavSpec, WavWriter};
use log::{debug, info};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::time::sleep;

const MIN_POLL_INTERVAL_MS: u64 = 500;
const MIN_TIMEOUT_SECONDS: u64 = 60;

/// Normalizes AssemblyAI language codes to their accepted format.
/// - "auto" => returns "auto" (enables language detection)
/// - "pt_br", "pt-BR", "pt_BR" => "pt"
/// - "zh-Hans", "zh-Hant" => "zh"
/// - Other codes are returned as-is if they look valid (lowercase letters, possibly with underscore)
/// - Invalid format returns an error
pub fn normalize_assembly_ai_language_code(code: &str) -> Result<String, String> {
    let trimmed = code.trim().to_lowercase();

    if trimmed == "auto" {
        return Ok("auto".to_string());
    }

    // Normalize Portuguese variants to "pt"
    if matches!(trimmed.as_str(), "pt_br" | "pt-br" | "pt") {
        return Ok("pt".to_string());
    }

    // Normalize Chinese variants to "zh"
    if matches!(trimmed.as_str(), "zh-hans" | "zh-hant" | "zh") {
        return Ok("zh".to_string());
    }

    // Validate: should be lowercase letters, optionally followed by underscore and more letters
    let parts: Vec<&str> = trimmed.split('_').collect();
    let valid = parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase()));

    if !valid || parts.is_empty() || parts[0].is_empty() {
        return Err(format!(
            "Invalid language code '{}'. Use 'auto' or a valid code like 'pt', 'en', 'en_us'.",
            code
        ));
    }

    Ok(trimmed)
}

/// Validates that the language code is acceptable for AssemblyAI.
/// Returns Ok(()) if valid, Err with message if invalid.
pub fn validate_assembly_ai_language_code(code: &str) -> Result<(), String> {
    normalize_assembly_ai_language_code(code)?;
    Ok(())
}

#[derive(Clone)]
pub struct TranscriptionService {
    app_handle: AppHandle,
    local_manager: Arc<TranscriptionManager>,
    client: Client,
}

#[derive(Debug, Deserialize)]
struct AssemblyUploadResponse {
    upload_url: String,
}

#[derive(Debug, Deserialize)]
struct AssemblyTranscriptResponse {
    id: Option<String>,
    status: String,
    text: Option<String>,
    error: Option<String>,
    utterances: Option<Vec<AssemblyUtterance>>,
}

#[derive(Debug, Deserialize, Clone)]
struct AssemblyUtterance {
    speaker: Option<String>,
    text: Option<String>,
    start: Option<i64>,
    end: Option<i64>,
}

#[derive(Debug, Clone)]
struct DiarizationConfig {
    enabled: bool,
    speakers_expected: Option<u8>,
}

fn diarization_config_from_session_options(
    session_options: Option<&StartSessionOptions>,
) -> DiarizationConfig {
    match session_options {
        Some(options) if options.enable_diarization => DiarizationConfig {
            enabled: true,
            speakers_expected: Some(options.speakers_expected),
        },
        _ => DiarizationConfig {
            enabled: false,
            speakers_expected: None,
        },
    }
}

fn build_assembly_transcript_payload(
    upload_url: &str,
    normalized_language_code: &str,
    diarization: &DiarizationConfig,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "audio_url": upload_url,
    });

    if normalized_language_code == "auto" {
        payload["language_detection"] = serde_json::Value::Bool(true);
    } else {
        payload["language_code"] = serde_json::Value::String(normalized_language_code.to_string());
    }

    if diarization.enabled {
        payload["speaker_labels"] = serde_json::Value::Bool(true);
        if let Some(speakers_expected) = diarization.speakers_expected {
            payload["speakers_expected"] = serde_json::Value::Number(speakers_expected.into());
        }
    }

    payload
}

fn format_diarized_transcript(utterances: &[AssemblyUtterance]) -> String {
    let mut sorted: Vec<AssemblyUtterance> = utterances
        .iter()
        .filter(|u| {
            u.text
                .as_deref()
                .map(|text| !text.trim().is_empty())
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    sorted.sort_by(|a, b| {
        let a_start = a.start.unwrap_or(i64::MAX);
        let b_start = b.start.unwrap_or(i64::MAX);
        a_start
            .cmp(&b_start)
            .then_with(|| a.end.unwrap_or(i64::MAX).cmp(&b.end.unwrap_or(i64::MAX)))
    });

    let mut speaker_map: HashMap<String, usize> = HashMap::new();
    let mut next_speaker_idx: usize = 0;
    let mut lines = Vec::new();

    for utterance in sorted {
        let text = match utterance.text {
            Some(text) if !text.trim().is_empty() => text.trim().to_string(),
            _ => continue,
        };

        let speaker_key = utterance
            .speaker
            .as_deref()
            .map(str::trim)
            .filter(|speaker| !speaker.is_empty())
            .unwrap_or("unknown")
            .to_string();

        let speaker_idx = *speaker_map.entry(speaker_key).or_insert_with(|| {
            let idx = next_speaker_idx;
            next_speaker_idx += 1;
            idx
        });

        let speaker_label = if speaker_idx < 26 {
            ((b'A' + speaker_idx as u8) as char).to_string()
        } else {
            format!("{}", speaker_idx + 1)
        };

        lines.push(format!("Speaker {}: {}", speaker_label, text));
    }

    lines.join("\n")
}

fn format_completed_transcript(
    payload: AssemblyTranscriptResponse,
    diarization: &DiarizationConfig,
) -> Result<String> {
    if !diarization.enabled {
        return Ok(payload.text.unwrap_or_default());
    }

    let utterances = payload.utterances.unwrap_or_default();
    let formatted = format_diarized_transcript(&utterances);

    if formatted.trim().is_empty() {
        return Err(anyhow!(
            "AssemblyAI diarization enabled but utterances are empty or invalid"
        ));
    }

    Ok(formatted)
}

impl TranscriptionService {
    pub fn new(app_handle: &AppHandle, local_manager: Arc<TranscriptionManager>) -> Result<Self> {
        Ok(Self {
            app_handle: app_handle.clone(),
            local_manager,
            client: Client::new(),
        })
    }

    pub fn initiate_model_load(&self) {
        if self.selected_backend() == TranscriptionBackend::Local {
            self.local_manager.initiate_model_load();
        }
    }

    pub async fn transcribe(
        &self,
        audio: Vec<f32>,
        session_options: Option<StartSessionOptions>,
    ) -> Result<String> {
        match self.selected_backend() {
            TranscriptionBackend::Local => self.transcribe_local(audio).await,
            TranscriptionBackend::AssemblyAi => {
                self.transcribe_assembly_ai(audio, session_options.as_ref()).await
            }
        }
    }

    fn selected_backend(&self) -> TranscriptionBackend {
        get_settings(&self.app_handle).transcription_backend
    }

    async fn transcribe_local(&self, audio: Vec<f32>) -> Result<String> {
        let local_manager = Arc::clone(&self.local_manager);
        tauri::async_runtime::spawn_blocking(move || local_manager.transcribe(audio))
            .await
            .map_err(|e| anyhow!("Local transcription task panicked: {e}"))?
    }

    async fn transcribe_assembly_ai(
        &self,
        audio: Vec<f32>,
        session_options: Option<&StartSessionOptions>,
    ) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let settings = get_settings(&self.app_handle);
        let api_key = settings.assembly_ai_api_key.trim().to_string();
        if api_key.is_empty() {
            return Err(anyhow!(
                "AssemblyAI API key is empty. Configure settings.assembly_ai_api_key"
            ));
        }

        let base_url_raw = settings.assembly_ai_base_url.trim();
        if base_url_raw.is_empty()
            || !(base_url_raw.starts_with("http://") || base_url_raw.starts_with("https://"))
        {
            return Err(anyhow!(
                "AssemblyAI base URL is invalid. Configure settings.assembly_ai_base_url with a valid http(s) URL"
            ));
        }

        let base_url = base_url_raw.trim_end_matches('/').to_string();
        let poll_interval = Duration::from_millis(
            settings
                .assembly_ai_poll_interval_ms
                .max(MIN_POLL_INTERVAL_MS),
        );
        let timeout = Duration::from_secs(
            settings
                .assembly_ai_timeout_seconds
                .max(MIN_TIMEOUT_SECONDS),
        );

        let diarization = diarization_config_from_session_options(session_options);

        let wav_bytes = samples_to_wav_bytes(&audio)?;
        let upload_url = self
            .assembly_ai_upload(&base_url, &api_key, wav_bytes)
            .await
            .context("AssemblyAI upload failed")?;

        let transcript_id = self
            .assembly_ai_start_transcript(&base_url, &api_key, &upload_url, &diarization)
            .await
            .context("AssemblyAI transcript start failed")?;

        self.assembly_ai_poll_transcript(
            &base_url,
            &api_key,
            &transcript_id,
            poll_interval,
            timeout,
            &diarization,
        )
        .await
    }

    async fn assembly_ai_upload(
        &self,
        base_url: &str,
        api_key: &str,
        wav_bytes: Vec<u8>,
    ) -> Result<String> {
        let response = self
            .client
            .post(format!("{base_url}/upload"))
            .header("authorization", api_key)
            .header("content-type", "application/octet-stream")
            .body(wav_bytes)
            .send()
            .await
            .context("HTTP request to AssemblyAI /upload failed")?
            .error_for_status()
            .context("AssemblyAI /upload returned non-success status")?;

        let payload: AssemblyUploadResponse = response
            .json()
            .await
            .context("Failed to parse AssemblyAI /upload response")?;

        Ok(payload.upload_url)
    }

    async fn assembly_ai_start_transcript(
        &self,
        base_url: &str,
        api_key: &str,
        upload_url: &str,
        diarization: &DiarizationConfig,
    ) -> Result<String> {
        let settings = get_settings(&self.app_handle);
        let language_code_raw = settings.assembly_ai_language_code.trim();
        let normalized = normalize_assembly_ai_language_code(language_code_raw)
            .map_err(|e| anyhow!("{}", e))?;

        let payload = build_assembly_transcript_payload(upload_url, &normalized, diarization);

        let response = self
            .client
            .post(format!("{base_url}/transcript"))
            .header("authorization", api_key)
            .json(&payload)
            .send()
            .await
            .context("HTTP request to AssemblyAI /transcript failed")?
            .error_for_status()
            .context("AssemblyAI /transcript returned non-success status")?;

        let payload: AssemblyTranscriptResponse = response
            .json()
            .await
            .context("Failed to parse AssemblyAI /transcript response")?;

        let id = payload
            .id
            .ok_or_else(|| anyhow!("AssemblyAI /transcript response missing id"))?;

        Ok(id)
    }

    async fn assembly_ai_poll_transcript(
        &self,
        base_url: &str,
        api_key: &str,
        transcript_id: &str,
        poll_interval: Duration,
        timeout: Duration,
        diarization: &DiarizationConfig,
    ) -> Result<String> {
        let started_at = Instant::now();

        loop {
            if started_at.elapsed() > timeout {
                return Err(anyhow!(
                    "AssemblyAI polling timed out after {}s for transcript {}",
                    timeout.as_secs(),
                    transcript_id
                ));
            }

            let response = self
                .client
                .get(format!("{base_url}/transcript/{transcript_id}"))
                .header("authorization", api_key)
                .send()
                .await
                .context("HTTP request to AssemblyAI transcript polling failed")?
                .error_for_status()
                .context("AssemblyAI transcript polling returned non-success status")?;

            let payload: AssemblyTranscriptResponse = response
                .json()
                .await
                .context("Failed to parse AssemblyAI transcript polling response")?;

            match payload.status.as_str() {
                "completed" => {
                    let text = format_completed_transcript(payload, diarization)?;
                    info!("AssemblyAI transcription completed. chars={}", text.len());
                    return Ok(text);
                }
                "error" | "failed" => {
                    let err = payload
                        .error
                        .unwrap_or_else(|| "unknown AssemblyAI error".to_string());
                    return Err(anyhow!("AssemblyAI transcription failed: {err}"));
                }
                "queued" | "processing" => {
                    debug!(
                        "AssemblyAI transcript {} still {}",
                        transcript_id, payload.status
                    );
                }
                other => {
                    debug!(
                        "AssemblyAI transcript {} unexpected status={}",
                        transcript_id, other
                    );
                }
            }

            sleep(poll_interval).await;
        }
    }
}

fn samples_to_wav_bytes(samples: &[f32]) -> Result<Vec<u8>> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer =
            WavWriter::new(&mut cursor, spec).context("Failed to create WAV writer")?;

        for sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let sample_i16 = (clamped * i16::MAX as f32) as i16;
            writer
                .write_sample(sample_i16)
                .context("Failed writing WAV sample")?;
        }

        writer.finalize().context("Failed to finalize WAV bytes")?;
    }

    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_to_wav_bytes_produces_valid_mono_16khz_wav() {
        let samples = vec![0.0_f32, 0.25, -0.25, 0.75, -0.75];
        let bytes = samples_to_wav_bytes(&samples).expect("wav conversion failed");

        let reader = hound::WavReader::new(Cursor::new(bytes)).expect("invalid wav bytes");
        let spec = reader.spec();

        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 16_000);
        assert_eq!(spec.bits_per_sample, 16);
        assert_eq!(spec.sample_format, SampleFormat::Int);
    }

    #[test]
    fn payload_diarization_off_does_not_send_speaker_fields() {
        let diarization = DiarizationConfig {
            enabled: false,
            speakers_expected: None,
        };
        let payload = build_assembly_transcript_payload("https://audio", "auto", &diarization);

        assert_eq!(payload["audio_url"], "https://audio");
        assert_eq!(payload["language_detection"], true);
        assert!(payload.get("speaker_labels").is_none());
        assert!(payload.get("speakers_expected").is_none());
    }

    #[test]
    fn payload_diarization_on_sends_speaker_fields() {
        let diarization = DiarizationConfig {
            enabled: true,
            speakers_expected: Some(2),
        };
        let payload = build_assembly_transcript_payload("https://audio", "pt", &diarization);

        assert_eq!(payload["audio_url"], "https://audio");
        assert_eq!(payload["language_code"], "pt");
        assert_eq!(payload["speaker_labels"], true);
        assert_eq!(payload["speakers_expected"], 2);
    }

    #[test]
    fn diarized_formatting_preserves_temporal_order_and_labels() {
        let utterances = vec![
            AssemblyUtterance {
                speaker: Some("spk_1".to_string()),
                text: Some("segunda fala".to_string()),
                start: Some(200),
                end: Some(260),
            },
            AssemblyUtterance {
                speaker: Some("spk_0".to_string()),
                text: Some("primeira fala".to_string()),
                start: Some(100),
                end: Some(180),
            },
        ];

        let formatted = format_diarized_transcript(&utterances);
        let expected = "Speaker A: primeira fala\nSpeaker B: segunda fala";
        assert_eq!(formatted, expected);
    }

    #[test]
    fn diarization_on_without_utterances_returns_explicit_error() {
        let payload = AssemblyTranscriptResponse {
            id: Some("abc".to_string()),
            status: "completed".to_string(),
            text: Some("fallback text".to_string()),
            error: None,
            utterances: None,
        };
        let diarization = DiarizationConfig {
            enabled: true,
            speakers_expected: Some(2),
        };

        let err = format_completed_transcript(payload, &diarization)
            .expect_err("expected diarization validation error");
        assert!(err
            .to_string()
            .contains("diarization enabled but utterances are empty or invalid"));
    }

    // Language code normalization tests
    #[test]
    fn normalize_language_code_auto() {
        assert_eq!(normalize_assembly_ai_language_code("auto").unwrap(), "auto");
        assert_eq!(normalize_assembly_ai_language_code("AUTO").unwrap(), "auto");
        assert_eq!(normalize_assembly_ai_language_code("  auto  ").unwrap(), "auto");
    }

    #[test]
    fn normalize_language_code_portuguese_variants() {
        assert_eq!(normalize_assembly_ai_language_code("pt").unwrap(), "pt");
        assert_eq!(normalize_assembly_ai_language_code("pt-BR").unwrap(), "pt");
        assert_eq!(normalize_assembly_ai_language_code("pt_br").unwrap(), "pt");
        assert_eq!(normalize_assembly_ai_language_code("pt_BR").unwrap(), "pt");
        assert_eq!(normalize_assembly_ai_language_code("PT-BR").unwrap(), "pt");
    }

    #[test]
    fn normalize_language_code_chinese_variants() {
        assert_eq!(normalize_assembly_ai_language_code("zh").unwrap(), "zh");
        assert_eq!(normalize_assembly_ai_language_code("zh-Hans").unwrap(), "zh");
        assert_eq!(normalize_assembly_ai_language_code("zh-Hant").unwrap(), "zh");
        assert_eq!(normalize_assembly_ai_language_code("zh-hans").unwrap(), "zh");
    }

    #[test]
    fn normalize_language_code_valid_codes() {
        assert_eq!(normalize_assembly_ai_language_code("en").unwrap(), "en");
        assert_eq!(normalize_assembly_ai_language_code("en_us").unwrap(), "en_us");
        assert_eq!(normalize_assembly_ai_language_code("en_uk").unwrap(), "en_uk");
        assert_eq!(normalize_assembly_ai_language_code("es").unwrap(), "es");
        assert_eq!(normalize_assembly_ai_language_code("de").unwrap(), "de");
    }

    #[test]
    fn normalize_language_code_invalid() {
        assert!(normalize_assembly_ai_language_code("").is_err());
        assert!(normalize_assembly_ai_language_code("   ").is_err());
        assert!(normalize_assembly_ai_language_code("123").is_err());
        assert!(normalize_assembly_ai_language_code("pt-BR-xx").is_err());
        assert!(normalize_assembly_ai_language_code("invalid!code").is_err());
    }

    #[test]
    fn validate_language_code_delegates_to_normalize() {
        assert!(validate_assembly_ai_language_code("auto").is_ok());
        assert!(validate_assembly_ai_language_code("pt-BR").is_ok());
        assert!(validate_assembly_ai_language_code("invalid!").is_err());
    }
}
