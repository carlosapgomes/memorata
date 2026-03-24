use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, TranscriptionBackend};
use anyhow::{anyhow, Context, Result};
use hound::{SampleFormat, WavSpec, WavWriter};
use log::{debug, info};
use reqwest::Client;
use serde::Deserialize;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::time::sleep;

const MIN_POLL_INTERVAL_MS: u64 = 500;
const MIN_TIMEOUT_SECONDS: u64 = 60;

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

    pub async fn transcribe(&self, audio: Vec<f32>) -> Result<String> {
        match self.selected_backend() {
            TranscriptionBackend::Local => self.transcribe_local(audio).await,
            TranscriptionBackend::AssemblyAi => self.transcribe_assembly_ai(audio).await,
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

    async fn transcribe_assembly_ai(&self, audio: Vec<f32>) -> Result<String> {
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

        let base_url = settings
            .assembly_ai_base_url
            .trim_end_matches('/')
            .to_string();
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

        let wav_bytes = samples_to_wav_bytes(&audio)?;
        let upload_url = self
            .assembly_ai_upload(&base_url, &api_key, wav_bytes)
            .await
            .context("AssemblyAI upload failed")?;

        let transcript_id = self
            .assembly_ai_start_transcript(&base_url, &api_key, &upload_url)
            .await
            .context("AssemblyAI transcript start failed")?;

        self.assembly_ai_poll_transcript(
            &base_url,
            &api_key,
            &transcript_id,
            poll_interval,
            timeout,
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
    ) -> Result<String> {
        let response = self
            .client
            .post(format!("{base_url}/transcript"))
            .header("authorization", api_key)
            .json(&serde_json::json!({
                "audio_url": upload_url,
            }))
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
                    let text = payload.text.unwrap_or_default();
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

    let cursor = Cursor::new(Vec::new());
    let mut writer = WavWriter::new(cursor, spec).context("Failed to create WAV writer")?;

    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * i16::MAX as f32) as i16;
        writer
            .write_sample(sample_i16)
            .context("Failed writing WAV sample")?;
    }

    let cursor = writer.finalize().context("Failed to finalize WAV bytes")?;
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
}
