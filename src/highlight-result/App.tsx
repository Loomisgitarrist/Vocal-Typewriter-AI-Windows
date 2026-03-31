import React, { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import './HighlightResult.css';

type HighlightStatus = 
  | "Idle" 
  | "Processing" 
  | { Success: string } 
  | { Error: string };

export default function App() {
  const [content, setContent] = useState('');
  const [image, setImage] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState('');
  const [debugInfo, setDebugInfo] = useState('');

  const fetchStatus = React.useCallback(async () => {
      try {
          const status = await invoke<HighlightStatus>('get_highlight_status');
          console.log("Polled status:", status);
          
          if (status === "Processing") {
              setLoading(true);
              setError('');
              setDebugInfo("Processing (polled)...");
          } else if (typeof status === 'object' && 'Success' in status) {
              setLoading(false);
              setError('');
              setContent(status.Success);
              setDebugInfo(`Received result (polled): ${status.Success.length} chars`);
          } else if (typeof status === 'object' && 'Error' in status) {
              setLoading(false);
              setError(status.Error);
              setDebugInfo(`Error (polled): ${status.Error}`);
          }
      } catch (e) {
          console.error("Failed to poll status:", e);
          setDebugInfo(`Poll failed: ${e}`);
      }
  }, []);

  // Poll when loading is true
  useEffect(() => {
    let intervalId: number | undefined;
    if (loading) {
        intervalId = window.setInterval(fetchStatus, 500);
    }
    return () => {
        if (intervalId) clearInterval(intervalId);
    };
  }, [loading, fetchStatus]);

  useEffect(() => {
    // Initial poll
    fetchStatus();

    // Poll on focus
    const onFocus = () => fetchStatus();
    window.addEventListener('focus', onFocus);
    
    // Timeout safety net
    let timeoutId: number;

    const unlistenPromise = listen<string | { text: string, image?: string | null }>('show-highlight-result', (event) => {
      console.log("Received result:", event.payload);
      
      let newContent = '';
      let newImage = null;
      let newError = '';

      if (!event.payload) {
          newError = "Received empty result";
      } else if (typeof event.payload === 'string') {
          // Legacy/Simple string payload
          if (event.payload.startsWith("Error: ")) {
              newError = event.payload;
          } else {
              newContent = event.payload;
          }
      } else {
          // Object payload (ResultPayload)
          newContent = event.payload.text;
          newImage = event.payload.image || null;
      }

      setDebugInfo(`Received result length: ${newContent.length} chars, Image: ${!!newImage}`);
      
      if (timeoutId) clearTimeout(timeoutId);
      
      setError(newError);
      setContent(newContent);
      // We need a state for image. I'll add it below.
      setImage(newImage); 
      
      setLoading(false);
      setCopied(false);
      getCurrentWindow().setFocus();
    });
    
    const unlistenProcessing = listen('processing-started', () => {
        console.log("Processing started...");
        setDebugInfo("Processing started...");
        setContent('');
        setImage(null);
        setError('');
        setLoading(true);
        setCopied(false);
        getCurrentWindow().setFocus();
        
        // Safety timeout
        if (timeoutId) clearTimeout(timeoutId);
        timeoutId = window.setTimeout(() => {
            // We use functional update or ref to check current loading state if needed,
            // but here we just assume if timeout fires, we are still waiting.
            // However, inside setTimeout, 'loading' is stale (closure).
            // So we just set error. If we already got result, we should have cleared timeout.
            setError("Request timed out. Please check your internet connection or API key.");
            setDebugInfo("Timed out waiting for result");
            setLoading(false);
        }, 70000); // 70s timeout
    });

    const unlistenError = listen<string>('processing-error', (event) => {
        console.log("Processing error:", event.payload);
        setDebugInfo(`Error: ${event.payload}`);
        if (timeoutId) clearTimeout(timeoutId);
        
        setError(event.payload);
        setLoading(false);
        setCopied(false);
        getCurrentWindow().setFocus();
    });

    return () => { 
        window.removeEventListener('focus', onFocus);
        unlistenPromise.then(u => u()); 
        unlistenProcessing.then(u => u());
        unlistenError.then(u => u());
        if (timeoutId) clearTimeout(timeoutId);
    };
  }, []);

  const handleClose = async () => {
    console.log("Closing result window...");
    setContent('');
    setImage(null);
    setError('');
    // Try both methods for robustness
    try {
        await invoke('close_highlight_result');
    } catch (e) {
        console.error("Failed to invoke close command:", e);
        await getCurrentWindow().hide();
    }
  };
  
  const handleCopy = () => {
     if (!content) return;
     navigator.clipboard.writeText(content);
     setCopied(true);
     setTimeout(() => setCopied(false), 2000);
  };

  const handlePrint = () => {
    if (!content) return;
    window.print();
  };

  // Close on Escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
            handleClose();
        }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <div className="result-container">
        <div className="header" data-tauri-drag-region>
            <h2 className="title">AI Result</h2>
            <div className="header-actions">
                {content && !loading && !error && (
                    <>
                        <button 
                            onClick={handleCopy} 
                            className={`icon-btn ${copied ? 'active' : ''}`} 
                            title="Copy to Clipboard"
                        >
                            {copied ? (
                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                    <polyline points="20 6 9 17 4 12"></polyline>
                                </svg>
                            ) : (
                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                                </svg>
                            )}
                        </button>
                        <button 
                            onClick={handlePrint} 
                            className="icon-btn" 
                            title="Print"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <polyline points="6 9 6 2 18 2 18 9"></polyline>
                                <path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"></path>
                                <rect x="6" y="14" width="12" height="8"></rect>
                            </svg>
                        </button>
                    </>
                )}
                <button onClick={handleClose} className="icon-btn" title="Close">
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <line x1="18" y1="6" x2="6" y2="18"></line>
                        <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                </button>
            </div>
        </div>
        
        <div className="content-area">
            {loading ? (
                <div className="placeholder">
                    <div className="spinner"></div>
                    <span>Processing...</span>
                </div>
            ) : error ? (
                <div className="error-message">
                    <div className="error-icon">⚠️</div>
                    {error}
                    {debugInfo && (
                         <div style={{ fontSize: '10px', color: '#94a3b8', marginTop: '10px', fontFamily: 'monospace' }}>
                             Debug: {debugInfo}
                         </div>
                     )}
                </div>
            ) : (content || image) ? (
                <div className="result-body">
                    {image && (
                        <div className="result-image-container" style={{ marginBottom: '1rem' }}>
                            <img 
                                src={`data:image/png;base64,${image}`} 
                                alt="Context" 
                                style={{ maxWidth: '100%', maxHeight: '300px', objectFit: 'contain', borderRadius: '4px', border: '1px solid #334155' }} 
                            />
                        </div>
                    )}
                    <div className="result-text" style={{ whiteSpace: 'pre-wrap' }}>{content}</div>
                </div>
            ) : (
                <div className="placeholder">
                    <span>No content received</span>
                    <div style={{ fontSize: '10px', color: '#94a3b8', marginTop: '10px', fontFamily: 'monospace' }}>
                        Debug: {debugInfo || "Waiting for data..."}
                    </div>
                </div>
            )}
        </div>
        
        {copied && (
            <div className="copy-toast">
                Copied to clipboard!
            </div>
        )}
    </div>
  );
}
