export interface ModelStateEvent {
  event_type: string;
  model_id?: string;
  model_name?: string;
  error?: string;
}

export interface RecordingErrorEvent {
  error_type: string;
  detail?: string;
}

export interface TranscriptionErrorEvent {
  error_type: string;
  detail?: string;
}

export type ProcessingStage = "preparing_audio" | "transcribing" | "saving";

export interface ProcessingStageEvent {
  stage: ProcessingStage;
}
