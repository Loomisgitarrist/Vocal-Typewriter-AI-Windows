import React from "react";
import ReactDOM from "react-dom/client";
import RecordingOverlay from "./RecordingOverlay";
import HudOverlay from "./HudOverlay";

const params = new URLSearchParams(window.location.search);
const type = params.get("type");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {type === "hud" ? <HudOverlay /> : <RecordingOverlay />}
  </React.StrictMode>,
);
