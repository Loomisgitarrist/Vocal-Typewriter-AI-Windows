import React, { useState, useEffect } from 'react';
import ReactDOM from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import './HighlightOverlay.css';

const HighlightOverlay = () => {
  const [isDrawing, setIsDrawing] = useState(false);
  const [startPos, setStartPos] = useState({ x: 0, y: 0 });
  const [currentPos, setCurrentPos] = useState({ x: 0, y: 0 });
  
  // Listen for Escape key to close
  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        await getCurrentWindow().hide();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const handleMouseDown = (e: React.MouseEvent) => {
    setIsDrawing(true);
    setStartPos({ x: e.screenX, y: e.screenY }); // Use screen coordinates for capture
    setCurrentPos({ x: e.screenX, y: e.screenY });
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (!isDrawing) return;
    setCurrentPos({ x: e.screenX, y: e.screenY });
  };

  const handleMouseUp = async () => {
    if (!isDrawing) return;
    setIsDrawing(false);

    // Calculate rect
    const x = Math.min(startPos.x, currentPos.x);
    const y = Math.min(startPos.y, currentPos.y);
    const width = Math.abs(currentPos.x - startPos.x);
    const height = Math.abs(currentPos.y - startPos.y);

    // Only capture if area is significant
    if (width > 5 && height > 5) {
      try {
        console.log(`Capturing region: ${x}, ${y}, ${width}x${height}`);
        await getCurrentWindow().hide(); // Hide immediately so overlay doesn't block capture (though screenshot crate might handle this)
        
        // Wait a tiny bit for window to disappear completely if needed, 
        // but since we want to capture what's UNDER it, hiding is good.
        // Invoke backend command
        await invoke('capture_screen_region', { x, y, width, height });
      } catch (error) {
        console.error('Failed to capture region:', error);
      }
    } else {
        await getCurrentWindow().hide();
    }
  };

  // Calculate selection box style for display (relative to window)
  // We need to map screen coordinates to window coordinates if they differ.
  // But for a fullscreen overlay, clientX/Y should match relative position if window is at 0,0.
  // However, we are using e.screenX/Y for logic. 
  // Let's use e.clientX/Y for rendering the box.
  
  const [renderStart, setRenderStart] = useState({ x: 0, y: 0 });
  const [renderCurrent, setRenderCurrent] = useState({ x: 0, y: 0 });

  // Listen for reset event
  useEffect(() => {
    const unlistenPromise = listen('reset-highlight-overlay', () => {
      console.log("Resetting highlight overlay");
      setIsDrawing(false);
      setStartPos({ x: 0, y: 0 });
      setCurrentPos({ x: 0, y: 0 });
      setRenderStart({ x: 0, y: 0 });
      setRenderCurrent({ x: 0, y: 0 });
    });
    
    return () => {
      unlistenPromise.then(unlisten => unlisten());
    };
  }, []);

  const handleMouseDownRender = (e: React.MouseEvent) => {
    setRenderStart({ x: e.clientX, y: e.clientY });
    setRenderCurrent({ x: e.clientX, y: e.clientY });
    handleMouseDown(e);
  };

  const handleMouseMoveRender = (e: React.MouseEvent) => {
    if (isDrawing) {
      setRenderCurrent({ x: e.clientX, y: e.clientY });
    }
    handleMouseMove(e);
  };

  const selectionStyle = {
    left: Math.min(renderStart.x, renderCurrent.x),
    top: Math.min(renderStart.y, renderCurrent.y),
    width: Math.abs(renderCurrent.x - renderStart.x),
    height: Math.abs(renderCurrent.y - renderStart.y),
  };

  return (
    <div 
      className="highlight-overlay"
      onMouseDown={handleMouseDownRender}
      onMouseMove={handleMouseMoveRender}
      onMouseUp={handleMouseUp}
    >
      <div className="crosshair-h" style={{ top: renderCurrent.y }}></div>
      <div className="crosshair-v" style={{ left: renderCurrent.x }}></div>
      
      {isDrawing && (
        <div className="selection-box" style={selectionStyle}></div>
      )}
      
      <div className="instruction-tooltip" style={{ left: renderCurrent.x + 20, top: renderCurrent.y + 20 }}>
        Drag to select area
        <br/>
        <span className="sub-text">Press Esc to cancel</span>
      </div>
    </div>
  );
};

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <HighlightOverlay />
  </React.StrictMode>
);
