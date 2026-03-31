import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import React, { useEffect, useRef, useState } from "react";
import {
  MicrophoneIcon,
  TranscriptionIcon,
  CancelIcon,
} from "../components/icons";
import "./RecordingOverlay.css";

type OverlayState = "recording" | "transcribing" | "highlighting";

const RecordingOverlay: React.FC = () => {
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  const [levels, setLevels] = useState<number[]>(Array(16).fill(0));
  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));

  useEffect(() => {
    const setupEventListeners = async () => {
      // Listen for show-overlay event from Rust
      const unlistenShow = await listen("show-overlay", async (event) => {
        const overlayState = event.payload as OverlayState;
        setState(overlayState);
        setIsVisible(true);
      });

      // Listen for hide-overlay event from Rust
      const unlistenHide = await listen("hide-overlay", () => {
        setIsVisible(false);
      });

      // Listen for mic-level updates
      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const payload = event.payload as number[];
        // console.log("Mic level:", payload); // Debug logging
        let newLevels: number[];

        // If we only get one value (RMS), generate a fake visualization
        if (payload.length === 1) {
          const rms = payload[0];
          // Create a symmetric wave pattern based on RMS
          newLevels = Array(16).fill(0).map((_, i) => {
            // Center is index 7.5
            const dist = Math.abs(i - 7.5);
            // Bell curve shape
            const shape = Math.exp(-(dist * dist) / 20); 
            // Add some noise for liveness
            const noise = 0.8 + Math.random() * 0.4;
            return rms * shape * noise;
          });
        } else {
          newLevels = payload;
        }

        // Apply smoothing to reduce jitter
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3; // Smooth transition
        });

        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed.slice(0, 16));
      });

      // Cleanup function
      return () => {
        unlistenShow();
        unlistenHide();
        unlistenLevel();
      };
    };

    setupEventListeners();
  }, []);

  const getIcon = () => {
    if (state === "recording") {
      return <MicrophoneIcon color="white" />;
    } else if (state === "highlighting") {
      return <MicrophoneIcon color="#d8b4fe" />;
    } else {
      return <TranscriptionIcon color="white" />;
    }
  };

  // Calculate loudness for text animation
  const maxLevel = levels.length > 0 ? Math.max(...levels) : 0;
  // Scale between 1.0 and 1.3 based on volume
  const scale = 1 + Math.min(0.3, maxLevel * 0.05);

  return (
    <div className={`recording-overlay ${isVisible ? "fade-in" : ""} ${state === "highlighting" ? "highlighting" : ""}`}>
      <div className="overlay-left">{getIcon()}</div>

      <div className="overlay-middle">
        {state === "recording" && (
          <div
            style={{
              color: "white",
              fontSize: "13px",
              fontWeight: 500,
              transform: `scale(${scale})`,
              transition: "transform 80ms ease-out",
              whiteSpace: "nowrap",
              textAlign: "center",
            }}
          >
            Listening...
          </div>
        )}
        {state === "highlighting" && (
          <div
            className="highlighting-text"
            style={{
              transform: `scale(${scale})`,
              transition: "transform 80ms ease-out",
            }}
          >
            Highlighting...
          </div>
        )}
        {state === "transcribing" && (
          <div className="transcribing-text">Transcribing...</div>
        )}
      </div>

      <div className="overlay-right">
        {(state === "recording" || state === "highlighting") && (
          <div
            className="cancel-button"
            onClick={() => {
              invoke("cancel_recording");
              setIsVisible(false);
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
