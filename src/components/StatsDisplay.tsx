import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';

interface Stats {
  totalWords: number;
  totalSavedMinutes: number;
  averageWPM: number;
  totalDurationSec: number;
}

interface StatsPayload {
  word_count: number;
  duration_sec: number;
}

export function StatsDisplay() {
  const [stats, setStats] = useState<Stats>({
    totalWords: 0,
    totalSavedMinutes: 0,
    averageWPM: 0,
    totalDurationSec: 0
  });

  useEffect(() => {
    // Load from localStorage
    const saved = localStorage.getItem('speak-to-me-stats');
    if (saved) {
      setStats(JSON.parse(saved));
    }

    // Listen for new stats
    let unlisten: (() => void) | undefined;
    
    const setupListener = async () => {
      try {
        // Check if we are in a Tauri environment (optional, but listen might throw otherwise)
        if (!(window as any).__TAURI_INTERNALS__ && !(window as any).__TAURI__) {
             // We are likely in a browser preview, skip listener
             console.log("Browser preview detected: Skipping Tauri event listeners.");
             return;
        }
        
        const unlistenFn = await listen<StatsPayload>('transcription-stats', (event) => {
          const { word_count, duration_sec } = event.payload;
          
          setStats(prev => {
            const newTotalWords = prev.totalWords + word_count;
            const newTotalDuration = prev.totalDurationSec + duration_sec;
            
            // Saved minutes: (words / 40) - (duration / 60)
            // We assume 40 WPM typing speed.
            const typingTimeMin = word_count / 40;
            const dictationTimeMin = duration_sec / 60;
            const sessionSavedMinutes = Math.max(0, typingTimeMin - dictationTimeMin);
            
            const newSavedMinutes = prev.totalSavedMinutes + sessionSavedMinutes;
            
            // Average WPM: Total Words / (Total Duration / 60)
            const newAverageWPM = newTotalDuration > 0 
              ? Math.round(newTotalWords / (newTotalDuration / 60)) 
              : 0;
    
            const newStats = {
              totalWords: newTotalWords,
              totalSavedMinutes: parseFloat(newSavedMinutes.toFixed(1)),
              averageWPM: newAverageWPM,
              totalDurationSec: newTotalDuration
            };
            
            localStorage.setItem('speak-to-me-stats', JSON.stringify(newStats));
            return newStats;
          });
        });
        unlisten = unlistenFn;
      } catch (err) {
        console.warn("Failed to set up stats listener (this is expected in browser preview):", err);
      }
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  return (
    <div className="stats-container">
      <div className="stat-card">
        <div className="stat-icon">📝</div>
        <div className="stat-content">
          <div className="stat-value">{stats.totalWords.toLocaleString()}</div>
          <div className="stat-label">Words Unleashed</div>
        </div>
      </div>
      <div className="stat-card">
        <div className="stat-icon">⏱️</div>
        <div className="stat-content">
          <div className="stat-value">{stats.totalSavedMinutes}</div>
          <div className="stat-label">Minutes Reclaimed</div>
        </div>
      </div>
      <div className="stat-card">
        <div className="stat-icon">⚡</div>
        <div className="stat-content">
          <div className="stat-value">{stats.averageWPM}</div>
          <div className="stat-label">Flow State (WPM)</div>
        </div>
      </div>
    </div>
  );
}
