import { useEffect, useMemo, useState } from "react";
import { commands, RecordingSessionState } from "@/bindings";
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
      return "AssemblyAI sem chave de API. Configure assembly_ai_api_key no settings_store.json.";
    case "assembly_ai_base_url_invalid":
      return "assembly_ai_base_url inválida. Use URL http(s) válida no settings_store.json.";
    case "local_model_not_selected":
      return "Nenhum modelo local selecionado. Defina selected_model nas configurações.";
    default:
      return `Erro de ação: ${errorCode}`;
  }
};

export default function RecordingSessionControls() {
  const [state, setState] = useState<RecordingSessionState>("idle");
  const [isLoading, setIsLoading] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  const canStart = useMemo(() => !isLoading && state === "idle", [isLoading, state]);
  const canPause = useMemo(() => !isLoading && state === "recording", [isLoading, state]);
  const canResume = useMemo(() => !isLoading && state === "paused", [isLoading, state]);
  const canStop = useMemo(
    () => !isLoading && (state === "recording" || state === "paused"),
    [isLoading, state],
  );

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

  const handleStart = async () => {
    setIsLoading(true);
    setLastError(null);
    try {
      const next = await commands.startRecordingSession();
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
        <div>
          <div className="text-sm text-mid-gray">Recording session</div>
          <div className="text-lg font-semibold">{STATE_LABEL[state]}</div>
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

      {lastError ? (
        <p className="mt-3 text-xs text-red-400">{formatSessionError(lastError)}</p>
      ) : null}
    </div>
  );
}
