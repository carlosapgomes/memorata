import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  MicrophoneIcon,
  TranscriptionIcon,
  CancelIcon,
} from "../components/icons";
import "./RecordingOverlay.css";
import { commands } from "@/bindings";
import i18n, { syncLanguageFromSettings } from "@/i18n";
import { getLanguageDirection } from "@/lib/utils/rtl";
import type { ProcessingStage } from "@/lib/types/events";

type OverlayState = "recording" | "paused" | "transcribing" | "processing";

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  const [processingStage, setProcessingStage] = useState<ProcessingStage | null>(null);
  const [levels, setLevels] = useState<number[]>(Array(16).fill(0));
  const [recordingSeconds, setRecordingSeconds] = useState(0);
  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const direction = getLanguageDirection(i18n.language);

  useEffect(() => {
    const setupEventListeners = async () => {
      // Listen for show-overlay event from Rust
      const unlistenShow = await listen("show-overlay", async (event) => {
        // Sync language from settings each time overlay is shown
        await syncLanguageFromSettings();
        const overlayState = event.payload as OverlayState;
        setState(overlayState);
        setIsVisible(true);
        
        // Reset timer when starting a new recording (not on pause/resume)
        if (overlayState === "recording" && !timerRef.current) {
          setRecordingSeconds(0);
        }
      });

      // Listen for hide-overlay event from Rust
      const unlistenHide = await listen("hide-overlay", () => {
        setIsVisible(false);
        setProcessingStage(null);
        // Stop and reset timer
        if (timerRef.current) {
          clearInterval(timerRef.current);
          timerRef.current = null;
        }
        setRecordingSeconds(0);
      });

      // Listen for mic-level updates
      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const newLevels = event.payload as number[];

        // Apply smoothing to reduce jitter
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3; // Smooth transition
        });

        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed.slice(0, 9));
      });

      // Listen for processing stage changes
      const unlistenStage = await listen<{ stage: ProcessingStage }>(
        "processing-stage-changed",
        (event) => {
          setProcessingStage(event.payload.stage);
        }
      );

      // Cleanup function
      return () => {
        unlistenShow();
        unlistenHide();
        unlistenLevel();
        unlistenStage();
      };
    };

    setupEventListeners();
  }, []);

  // Timer effect - runs when state is recording or paused
  useEffect(() => {
    if (state === "recording") {
      // Start timer if not running
      if (!timerRef.current) {
        timerRef.current = setInterval(() => {
          setRecordingSeconds((prev) => prev + 1);
        }, 1000);
      }
    } else if (state === "paused") {
      // Keep timer frozen (don't clear interval, just don't increment)
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    } else {
      // Stop timer for other states
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

  // Format seconds to hh:mm:ss
  const formatTime = (totalSeconds: number): string => {
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;
    
    return `${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
  };

  const getIcon = () => {
    if (state === "recording" || state === "paused") {
      return <MicrophoneIcon />;
    } else {
      return <TranscriptionIcon />;
    }
  };

  const getStageText = (): string | null => {
    if (state === "paused") {
      return t("overlay.paused");
    }
    if (state !== "transcribing" && state !== "processing") return null;
    if (!processingStage) return t(`overlay.${state}`);
    
    const stageKeyMap: Record<ProcessingStage, string> = {
      preparing_audio: "overlay.stages.preparingAudio",
      transcribing: "overlay.stages.transcribing",
      saving: "overlay.stages.saving",
    };
    
    return t(stageKeyMap[processingStage]);
  };

  // Pulse icon during recording or processing (not during paused)
  const showPulsingIcon = state === "recording" || (processingStage !== null && state !== "paused");

  return (
    <div
      dir={direction}
      className={`recording-overlay ${isVisible ? "fade-in" : ""}`}
    >
      <div className={`overlay-left ${showPulsingIcon ? "icon-pulse" : ""} ${state === "paused" ? "icon-paused" : ""}`}>
        {getIcon()}
      </div>

      <div className="overlay-middle">
        {(state === "recording" || state === "paused") && (
          <>
            <div className="bars-container">
              {levels.map((v, i) => (
                <div
                  key={i}
                  className="bar"
                  style={{
                    height: `${Math.min(20, 4 + Math.pow(v, 0.7) * 16)}px`, // Cap at 20px max height
                    transition: "height 60ms ease-out, opacity 120ms ease-out",
                    opacity: Math.max(0.2, v * 1.7), // Minimum opacity for visibility
                  }}
                />
              ))}
            </div>
            <div className={`timer ${state === "paused" ? "timer-paused" : ""}`}>
              {formatTime(recordingSeconds)}
            </div>
          </>
        )}
        {(state === "transcribing" || state === "processing") && (
          <div className="transcribing-text">{getStageText()}</div>
        )}
      </div>

      <div className="overlay-right">
        {state === "recording" && (
          <div
            className="cancel-button"
            onClick={() => {
              commands.cancelOperation();
            }}
          >
            <CancelIcon />
          </div>
        )}
      </div>
    </div>
  );
};

export default RecordingOverlay;
