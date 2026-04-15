import { useEffect, useMemo, useRef, useState } from "react";
import {
  commands,
  RecordingSessionState,
  StartSessionOptions,
} from "@/bindings";
import { Button } from "@/components/ui";
import { useSettings } from "@/hooks/useSettings";
import { toast } from "sonner";

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

const formatPostProcessError = (errorCode: string) => {
  switch (errorCode) {
    case "post_process_disabled":
      return "Ative o pós-processamento nas configurações para usar esta ação manual.";
    case "post_process_provider_missing":
      return "Selecione um provedor de pós-processamento (ex.: OpenRouter).";
    case "post_process_api_key_missing":
      return "A chave de API do provedor selecionado não está configurada.";
    case "post_process_model_missing":
      return "Selecione um modelo para o provedor de pós-processamento.";
    case "post_process_prompt_not_selected":
      return "Selecione um prompt de pós-processamento.";
    case "post_process_prompt_not_found":
      return "O prompt selecionado não foi encontrado. Selecione outro prompt.";
    case "post_process_prompt_empty":
      return "O prompt selecionado está vazio.";
    case "history_entry_not_found":
      return "Nenhuma transcrição concluída foi encontrada para pós-processar.";
    case "post_process_failed":
      return "Falha ao aplicar o pós-processamento. Verifique provedor, modelo e prompt.";
    default:
      return `Erro no pós-processamento: ${errorCode}`;
  }
};

export default function RecordingSessionControls() {
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [state, setState] = useState<RecordingSessionState>("idle");
  const [isLoading, setIsLoading] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);
  const [recordingSeconds, setRecordingSeconds] = useState(0);
  const [diarizationEnabled, setDiarizationEnabled] = useState(true);
  const [speakersExpected, setSpeakersExpected] = useState(2);
  const [isPostProcessing, setIsPostProcessing] = useState(false);
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

  const autoPostProcessOnStop =
    getSetting("auto_post_process_on_session_stop") ?? false;
  const isAutoPostProcessUpdating = isUpdating(
    "auto_post_process_on_session_stop",
  );

  const canManualPostProcess =
    !isLoading &&
    !isPostProcessing &&
    state === "idle" &&
    !autoPostProcessOnStop;

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

  const handleManualPostProcess = async () => {
    setIsPostProcessing(true);
    try {
      const result = await commands.postProcessLatestHistoryEntry();
      if (result.status === "ok") {
        toast.success("Pós-processamento concluído", {
          description:
            "A última transcrição foi atualizada com o resultado do prompt.",
        });
      } else {
        toast.error("Falha no pós-processamento", {
          description: formatPostProcessError(result.error),
        });
      }
    } catch (error) {
      toast.error("Falha no pós-processamento", {
        description: String(error),
      });
    } finally {
      setIsPostProcessing(false);
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

      <div className="mt-4 flex flex-col md:flex-row md:items-center gap-3">
        <label className="flex items-center gap-2 text-sm text-mid-gray">
          <input
            type="checkbox"
            checked={autoPostProcessOnStop}
            disabled={isAutoPostProcessUpdating || state !== "idle"}
            onChange={(event) =>
              void updateSetting(
                "auto_post_process_on_session_stop",
                event.target.checked,
              )
            }
          />
          Auto pós-processar ao clicar em Stop
        </label>

        <Button
          onClick={handleManualPostProcess}
          disabled={!canManualPostProcess}
          title={
            autoPostProcessOnStop
              ? "Desative o auto pós-processamento para usar este botão manual."
              : "Aplica o prompt na última transcrição concluída"
          }
        >
          {isPostProcessing ? "Pós-processando..." : "Pós-processar agora"}
        </Button>
      </div>

      {lastError ? (
        <p className="mt-3 text-xs text-red-400">
          {formatSessionError(lastError)}
        </p>
      ) : null}
    </div>
  );
}
