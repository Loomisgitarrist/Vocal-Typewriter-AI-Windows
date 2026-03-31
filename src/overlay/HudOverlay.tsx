import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useState } from "react";
import "./HudOverlay.css";

const HudOverlay: React.FC = () => {
  const [message, setMessage] = useState("");

  useEffect(() => {
    // Expose setter to window for direct calling from Rust (more reliable than events for hidden windows)
    (window as any).setHudMessage = (msg: string) => {
      setMessage(msg);
    };

    const unlisten = listen<string>("hud-message", (event) => {
      setMessage(event.payload);
    });
    return () => {
      unlisten.then((f) => f());
      delete (window as any).setHudMessage;
    };
  }, []);

  return (
    <div className="hud-container">
      <div className="hud-box">
        {message}
      </div>
    </div>
  );
};

export default HudOverlay;
