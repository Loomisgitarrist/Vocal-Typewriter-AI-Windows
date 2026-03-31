import React from "react";
import ReactDOM from "react-dom/client";
import StyleSelector from "./StyleSelector";
// import "../index.css"; // Removed to prevent global style conflicts

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <StyleSelector />
  </React.StrictMode>
);
