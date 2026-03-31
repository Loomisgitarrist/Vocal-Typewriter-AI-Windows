import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import './style.css';


type ResizeHandle = 'nw' | 'ne' | 'sw' | 'se' | 'n' | 's' | 'w' | 'e';
type DragMode = 'create' | 'move' | 'resize' | 'none';

export default function App() {
  const [selection, setSelection] = useState<{x: number, y: number, w: number, h: number} | null>(null);
  const [dragMode, setDragMode] = useState<DragMode>('none');
  const [activeHandle, setActiveHandle] = useState<ResizeHandle | null>(null);
  const startPos = useRef<{x: number, y: number} | null>(null);
  const initialSelection = useRef<{x: number, y: number, w: number, h: number} | null>(null);

  useEffect(() => {
    const unlisten = listen('reset-selection', () => {
      setSelection(null);
      setDragMode('none');
    });
    
    // Global key listener for Enter/Escape
    const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
            handleCancel();
        } else if (e.key === 'Enter' && selection) {
            handleConfirm();
        }
    };
    window.addEventListener('keydown', handleKeyDown);
    
    return () => { 
        unlisten.then(u => u()); 
        window.removeEventListener('keydown', handleKeyDown);
    };
  }, [selection]);

  const handleContainerMouseDown = (e: React.MouseEvent) => {
    console.log('Starting create drag');
    setDragMode('create');
    startPos.current = { x: e.clientX, y: e.clientY };
    setSelection({
      x: e.clientX,
      y: e.clientY,
      w: 0,
      h: 0
    });
  };

  const handleBoxMouseDown = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!selection) return;

    // Don't start drag if clicking toolbar or buttons
    if ((e.target as HTMLElement).closest('.selection-toolbar') || 
        (e.target as HTMLElement).closest('button')) {
        return;
    }

    console.log('Starting move drag');
    setDragMode('move');
    startPos.current = { x: e.clientX, y: e.clientY };
    initialSelection.current = { ...selection };
  };

  const handleHandleMouseDown = (e: React.MouseEvent, handle: ResizeHandle) => {
      e.stopPropagation(); // Prevent container creation logic
      setDragMode('resize');
      setActiveHandle(handle);
      startPos.current = { x: e.clientX, y: e.clientY };
      if (selection) {
          initialSelection.current = { ...selection };
      }
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (dragMode === 'none' || !startPos.current) return;

    const currentX = e.clientX;
    const currentY = e.clientY;
    const dx = currentX - startPos.current.x;
    const dy = currentY - startPos.current.y;

    if (dragMode === 'create') {
        const startX = startPos.current.x;
        const startY = startPos.current.y;
        const x = Math.min(currentX, startX);
        const y = Math.min(currentY, startY);
        const w = Math.abs(currentX - startX);
        const h = Math.abs(currentY - startY);
        setSelection({ x, y, w, h });
    } else if (dragMode === 'move' && initialSelection.current) {
        setSelection({
            ...initialSelection.current,
            x: initialSelection.current.x + dx,
            y: initialSelection.current.y + dy
        });
    } else if (dragMode === 'resize' && initialSelection.current && activeHandle) {
        const s = initialSelection.current;
        let newS = { ...s };

        // Simple resizing logic
        switch (activeHandle) {
            case 'e': newS.w = s.w + dx; break;
            case 'w': newS.x = s.x + dx; newS.w = s.w - dx; break;
            case 's': newS.h = s.h + dy; break;
            case 'n': newS.y = s.y + dy; newS.h = s.h - dy; break;
            case 'se': newS.w = s.w + dx; newS.h = s.h + dy; break;
            case 'sw': newS.x = s.x + dx; newS.w = s.w - dx; newS.h = s.h + dy; break;
            case 'ne': newS.y = s.y + dy; newS.h = s.h - dy; newS.w = s.w + dx; break;
            case 'nw': newS.x = s.x + dx; newS.w = s.w - dx; newS.y = s.y + dy; newS.h = s.h - dy; break;
        }
        
        // Prevent negative width/height
        if (newS.w < 0) {
            newS.x = newS.x + newS.w;
            newS.w = Math.abs(newS.w);
        }
        if (newS.h < 0) {
            newS.y = newS.y + newS.h;
            newS.h = Math.abs(newS.h);
        }

        setSelection(newS);
    }
  };

  const handleMouseUp = () => {
    setDragMode('none');
    setActiveHandle(null);
  };

  const handleConfirm = () => {
      if (selection && selection.w > 0 && selection.h > 0) {
          invoke('update_highlight_rect', { 
            x: Math.round(selection.x), 
            y: Math.round(selection.y), 
            width: Math.round(selection.w), 
            height: Math.round(selection.h) 
          }).catch(console.error);
      }
  };

  const handleCancel = async () => {
      invoke('clear_screenshot').catch(console.error);
      setSelection(null);
      await getCurrentWindow().hide();
  };

  return (
    <div 
      className="overlay-container"
      onMouseDown={handleContainerMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
    >
      {/* Window Drag Handle */}
      <div className="window-drag-handle" data-tauri-drag-region>
        <div className="window-title">Selection Overlay</div>
        <div className="window-controls">
            <button className="window-close-btn" onClick={handleCancel}>✕</button>
        </div>
      </div>

      {!selection && <div className="initial-dimmer"></div>}
      
      {selection && (
        <div 
          className="selection-box"
          onMouseDown={handleBoxMouseDown}
          style={{
            left: selection.x,
            top: selection.y,
            width: selection.w,
            height: selection.h
          }}
        >
          {/* Resize Handles */}
          {['nw', 'ne', 'sw', 'se', 'n', 's', 'w', 'e'].map((h) => (
              <div 
                key={h} 
                className={`resize-handle handle-${h}`}
                onMouseDown={(e) => handleHandleMouseDown(e, h as ResizeHandle)}
              />
          ))}

          <div className="selection-border"></div>
          
          {/* Dimensions */}
          <div className="dimensions-label">
            {Math.round(selection.w)} x {Math.round(selection.h)}
          </div>

          {/* Toolbar */}
          <div className="selection-toolbar">
              <button onClick={handleCancel} className="btn-cancel">✕</button>
              <button onClick={handleConfirm} className="btn-confirm">✓</button>
          </div>
        </div>
      )}
      
      {!selection && dragMode === 'none' && (
        <div className="instruction-toast">
          Click and drag to select area
        </div>
      )}
    </div>
  );
}
