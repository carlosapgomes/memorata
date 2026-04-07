import { useEffect, useMemo, useRef, useState } from "react";
import {
  commands,
  RecordingSessionState,
  StartSessionOptions,
} from "@/bindings";
import { Button } from "@/components/ui";

const STATE_LABEL: Record<RecordingSessionState, string> = {
  idle: "Idle",
  recording: "Recording",
  paused: "Paused",
  processing: "Processing",
};

const formatSessionError = (errorCode: string) => {
  switch (errorCode) {
    case "processing_in_progress":
      return "Aguarde: já existe um processamento em andamento.";
    case "session_paused_use_resume":
      return "A sessão está pausada. Use Resume ou Stop.";
    case "recording_not_active":
      return "Não há gravação ativa para essa ação.";
    case "coordinator_unavailable":
      return "Falha interna ao iniciar controle de sessão. Reinicie o app.";
    case "assembly_ai_api_key_missing":
      return "AssemblyAI sem chave de API. Configure a chave em Settings > General.";
    case "assembly_ai_base_url_invalid":
      return "assembly_ai_base_url inválida. Use uma URL http(s) válida nas configurações do app.";
    case "local_model_not_selected":
      return "Nenhum modelo local selecionado. Defina selected_model nas configurações.";
    case "assembly_ai_speakers_expected_invalid":
      return "Speakers inválido: com diarização ativa, informe pelo menos 2 speakers.";
    default:
      return `Erro de ação: ${errorCode}`;
  }
};

export default function RecordingSessionControls() {
  const [state, setState] = useState<RecordingSessionState>("idle");
  const [isLoading, setIsLoading] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);
  const [recordingSeconds, setRecordingSeconds] = useState(0);
  const [diarizationEnabled, setDiarizationEnabled] = useState(true);
  const [speakersExpected, setSpeakersExpected] = useState(2);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const recordingStartRef = useRef<number | null>(null);

  const canStart = useMemo(
    () => !isLoading && state === "idle",
    [isLoading, state],
  );
  const canPause = useMemo(
    () => !isLoading && state === "recording",
    [isLoading, state],
  );
  const canResume = useMemo(
    () => !isLoading && state === "paused",
    [isLoading, state],
  );
  const canStop = useMemo(
    () => !isLoading && (state === "recording" || state === "paused"),
    [isLoading, state],
  );
  const sessionControlsDisabled = useMemo(
    () => isLoading || state !== "idle",
    [isLoading, state],
  );

  const formatTime = (totalSeconds: number): string => {
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;

    return `${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
  };

  const refreshState = async () => {
    try {
      const next = await commands.getRecordingSessionState();
      setState(next);
    } catch {
      // Keep silent here to avoid noisy UI during startup.
    }
  };

  useEffect(() => {
    refreshState();
    const timer = window.setInterval(refreshState, 800);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!diarizationEnabled) {
      setSpeakersExpected(1);
      return;
    }

    setSpeakersExpected((current) => (current < 2 ? 2 : current));
  }, [diarizationEnabled]);

  useEffect(() => {
    if (state === "recording") {
      if (!timerRef.current) {
        if (recordingStartRef.current === null) {
          recordingStartRef.current = Date.now() - recordingSeconds * 1000;
        }
        timerRef.current = setInterval(() => {
          if (recordingStartRef.current !== null) {
            const elapsed = Math.floor(
              (Date.now() - recordingStartRef.current) / 1000,
            );
            setRecordingSeconds(elapsed);
          }
        }, 1000);
      }
    } else if (state === "paused") {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    } else if (state === "idle") {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
      setRecordingSeconds(0);
      recordingStartRef.current = null;
    } else {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    }

    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [state]);

  const handleStart = async () => {
    setIsLoading(true);
    setLastError(null);

    const options: StartSessionOptions = {
      enable_diarization: diarizationEnabled,
      speakers_expected: diarizationEnabled ? Math.max(2, speakersExpected) : 1,
    };

    try {
      const next = await commands.startRecordingSession(options);
      if (next.status === "ok") {
        setState(next.data);
      } else {
        setLastError(next.error);
        await refreshState();
      }
    } finally {
      setIsLoading(false);
    }
  };

  const handlePause = async () => {
    setIsLoading(true);
    setLastError(null);
    try {
      const next = await commands.pauseRecordingSession();
      if (next.status === "ok") {
        setState(next.data);
      } else {
        setLastError(next.error);
        await refreshState();
      }
    } finally {
      setIsLoading(false);
    }
  };

  const handleResume = async () => {
    setIsLoading(true);
    setLastError(null);
    try {
      const next = await commands.resumeRecordingSession();
      if (next.status === "ok") {
        setState(next.data);
      } else {
        setLastError(next.error);
        await refreshState();
      }
    } finally {
      setIsLoading(false);
    }
  };

  const handleStop = async () => {
    setIsLoading(true);
    setLastError(null);
    try {
      const next = await commands.stopRecordingSession();
      if (next.status === "ok") {
        setState(next.data);
      } else {
        setLastError(next.error);
        await refreshState();
      }
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="w-full max-w-[720px] rounded-xl border border-mid-gray/20 p-4 bg-black/10">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <div
            className={`w-3 h-3 rounded-full ${
              state === "recording"
                ? "bg-red-500 animate-pulse"
                : state === "paused"
                  ? "bg-yellow-500"
                  : state === "processing"
                    ? "bg-blue-500"
                    : "bg-gray-500"
            }`}
          />
          <div>
            <div className="text-sm text-mid-gray">Recording session</div>
            <div className="text-lg font-semibold flex items-center gap-2">
              {STATE_LABEL[state]}
              {(state === "recording" || state === "paused") && (
                <span
                  className={`text-sm font-mono ${state === "paused" ? "text-yellow-400" : "text-mid-gray"}`}
                >
                  {formatTime(recordingSeconds)}
                </span>
              )}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Button onClick={handleStart} disabled={!canStart}>
            Start
          </Button>
          <Button onClick={handlePause} disabled={!canPause}>
            Pause
          </Button>
          <Button onClick={handleResume} disabled={!canResume}>
            Resume
          </Button>
          <Button onClick={handleStop} disabled={!canStop}>
            Stop
          </Button>
        </div>
      </div>

      <div className="mt-3 grid grid-cols-1 md:grid-cols-2 gap-3">
        <label className="flex items-center gap-2 text-sm text-mid-gray">
          <input
            type="checkbox"
            checked={diarizationEnabled}
            disabled={sessionControlsDisabled}
            onChange={(event) => setDiarizationEnabled(event.target.checked)}
          />
          Diarização
        </label>

        <label className="flex items-center gap-2 text-sm text-mid-gray">
          <span>Speakers</span>
          <input
            type="number"
            className="w-24 rounded border border-mid-gray/40 bg-black/20 px-2 py-1 text-sm"
            min={diarizationEnabled ? 2 : 1}
            step={1}
            value={speakersExpected}
            disabled={sessionControlsDisabled || !diarizationEnabled}
            onChange={(event) => {
              const raw = Number.parseInt(event.target.value, 10);
              if (Number.isNaN(raw)) {
                setSpeakersExpected(diarizationEnabled ? 2 : 1);
                return;
              }
              setSpeakersExpected(diarizationEnabled ? Math.max(2, raw) : 1);
            }}
          />
        </label>
      </div>

      {lastError ? (
        <p className="mt-3 text-xs text-red-400">
          {formatSessionError(lastError)}
        </p>
      ) : null}
    </div>
  );
}
