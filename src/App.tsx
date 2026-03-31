import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { StatsDisplay } from "./components/StatsDisplay";
import logo from "./assets/logo.png";

import { TONE_OPTIONS } from "./data/toneOptions";

const DEFAULT_SYSTEM_PROMPT = `You are a professional editor. Your ONLY job is to fix grammar and format the input text.

RULES:
1. Do NOT use markdown bolding (asterisks).
2. Use bullet points or numbered lists where appropriate for professional formatting.
3. Do NOT add ANY conversational filler (e.g., "Here is the text", "Let me know if...").
4. Do NOT output anything other than the corrected text itself.
5. Remove filler words like "um", "uh", "you know".

Example Input:
I have two tasks um buy milk and walk the dog
Example Output:
I have two tasks:
- Buy milk
- Walk the dog

Input:`;

const VISION_SYSTEM_PROMPT = `You are a helpful assistant with vision capabilities. You can see a screenshot of what the user is working on.

Your task:
1. Analyze the screenshot to understand the context (document type, formatting style, surrounding text)
2. Based on the user's spoken instructions, provide text that matches the document's tone and style
3. Format output to match the existing document formatting:
   - Use numbered or bulleted lists where appropriate
   - Use **bold** markdown for emphasis if it matches the style
   - Ensure proper paragraph spacing
4. Output ONLY the final text - no explanations, no markdown code blocks
5. Do NOT include "Here is the text" or similar prefixes

The output must be immediately usable and match the professional style of the document.`;



interface Settings {
  shortcut: string;
  push_to_talk_key: string;
  auto_copy: boolean;
  correction_mode: string;
  ollama_model: string;
  ollama_vision_model: string;
  context_mode: string;
  auto_start: boolean;
  system_prompt: string;
  input_device_id: string;
  overlay_position: "Top" | "Bottom" | "None";
  openai_api_key?: string;
  openai_model: string;
  openrouter_api_key?: string;
  openrouter_model: string;
  transcription_engine: string;
  active_model_id: string;
  style_selector_key?: string;
  user_name?: string;
  raw_mode_toggle_key?: string;
  highlight_shortcut?: string;
  highlight_mode?: string;
  last_llm_mode?: string;
  target_language?: string;
  tone?: string;
  custom_models_path?: string;
}

interface OpenRouterModel {
  id: string;
  name: string;
  pricing: {
    prompt: string;
    completion: string;
  };
}

interface OpenRouterResponse {
  data: OpenRouterModel[];
}

const OPENAI_MODELS = {
  'OpenAI Gpt 5.2 Models': [
      { value: 'gpt-5.2', label: 'gpt-5.2 (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5.2-chat-latest', label: 'gpt-5.2-chat-latest (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5.2-pro', label: 'gpt-5.2-pro (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' }
  ],
  'OpenAI Gpt 5.1 Models': [
      { value: 'gpt-5.1', label: 'gpt-5.1 (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5.1-chat-latest', label: 'gpt-5.1-chat-latest (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5.1-codex-max', label: 'gpt-5.1-codex-max (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5.1-codex', label: 'gpt-5.1-codex (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5.1-codex-mini', label: 'gpt-5.1-codex-mini (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' }
  ],
  'OpenAI Gpt 5 Models': [
      { value: 'gpt-5', label: 'gpt-5 (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5-mini', label: 'gpt-5-mini (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5-nano', label: 'gpt-5-nano (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5-chat-latest', label: 'gpt-5-chat-latest (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5-codex', label: 'gpt-5-codex (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5-pro', label: 'gpt-5-pro (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' },
      { value: 'gpt-5-search-api', label: 'gpt-5-search-api (Context window: 400,000 tokens, Max output tokens: 128,000 tokens)' }
  ],
  'OpenAI 4.1 Models': [
      { value: 'gpt-4.1', label: 'Gpt-4.1 (Context window: 1,047,576 tokens, Max output tokens: 32,768 max output tokens)' },
      { value: 'gpt-4.1-2025-04-14', label: 'Gpt-4.1-2025-04-14 (Context window: 1,047,576 tokens, Max output tokens: 32,768 max output tokens)' },
      { value: 'gpt-4.1-mini', label: 'Gpt-4.1 Mini (Context window: 1,047,576 tokens, Max output tokens: 32,768 max output tokens)' },
      { value: 'gpt-4.1-mini-2025-04-14', label: 'Gpt-4.1 Mini-2025-04-14 (Context window: 1,047,576 tokens, Max output tokens: 32,768 max output tokens)' },
      { value: 'gpt-4.1-nano', label: 'Gpt-4.1 Nano (Context window: 1,047,576 tokens, Max output tokens: 32,768 max output tokens)' },
      { value: 'gpt-4.1-nano-2025-04-14', label: 'Gpt-4.1 Nano-2025-04-14 (Context window: 1,047,576 tokens, Max output tokens: 32,768 max output tokens)' }
  ],
  'OpenAI o4 Models': [
      { value: 'o4-mini', label: 'o4-mini (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'o4-mini-2025-04-16', label: 'o4-mini-2025-04-16 (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'o4-mini-deep-research', label: 'o4-mini-deep-research (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'o4-mini-deep-research-2025-06-26', label: 'o4-mini-deep-research-2025-06-26 (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' }
  ],
  'OpenAI o3 Models': [
      { value: 'o3', label: 'o3 (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'o3-2025-04-16', label: 'o3-2025-04-16 (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'o3-pro', label: 'o3-pro (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'o3-pro-2025-06-10', label: 'o3-pro-2025-06-10 (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'o3-mini', label: 'o3-mini (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'o3-mini-2025-01-31', label: 'o3-mini-2025-01-31 (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'o3-deep-research', label: 'o3-deep-research (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'o3-deep-research-2025-06-26', label: 'o3-deep-research-2025-06-26 (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' }
  ],
  'OpenAI o1 Models': [
      { value: 'o1', label: 'o1 (200,000 tokens, 100,000 tokens)' },
      { value: 'o1-2024-12-17', label: 'o1-2024-12-17 (200,000 tokens / 100,000 output tokens)' },
      { value: 'o1-pro', label: 'o1-pro (200,000 tokens, 100,000 tokens)' },
      { value: 'o1-pro-2025-03-19', label: 'o1-pro-2025-03-19 (200,000 tokens, 100,000 tokens)' },
      { value: 'o1-mini', label: 'o1-mini (128,000 tokens / 65,536 output tokens)' },
      { value: 'o1-mini-2024-09-12', label: 'o1-mini-2024-09-12 (128,000 tokens / 65,536 output tokens)' },
      { value: 'o1-preview', label: 'o1-preview (128,000 tokens / 32,768 output tokens)' },
      { value: 'o1-preview-2024-09-12', label: 'o1-preview-2024-09-12 (128,000 tokens / 32,768 output tokens)' }
  ],
  'OpenAI 4o Models': [
      { value: 'gpt-4o', label: 'Gpt-4o (4,096 tokens)' },
      { value: 'gpt-4o-2024-08-06', label: 'GPT-4o 2024-08-06 (16,384 tokens)' },
      { value: 'gpt-4o-2024-05-13', label: 'GPT-4o 2024-05-13 (4,096 tokens)' },
      { value: 'gpt-4o-mini', label: 'GPT-4o Mini (16,384 tokens)' },
      { value: 'gpt-4o-mini-2024-07-18', label: 'GPT-4o Mini 2024-07-18 (16,384 tokens)' },
      { value: 'gpt-4o-audio-preview', label: 'gpt-4o-audio-preview (4,096 tokens)' },
      { value: 'gpt-4o-audio-preview-2025-06-03', label: 'gpt-4o-audio-preview-2025-06-03 (4,096 tokens)' },
      { value: 'gpt-4o-mini-audio-preview', label: 'gpt-4o-mini-audio-preview (16,384 tokens)' },
      { value: 'gpt-4o-mini-audio-preview-2024-12-17', label: 'gpt-4o-mini-audio-preview-2024-12-17 (16,384 tokens)' },
      { value: 'gpt-4o-realtime-preview', label: 'gpt-4o-realtime-preview (4,096 tokens)' },
      { value: 'gpt-4o-realtime-preview-2025-06-03', label: 'gpt-4o-realtime-preview-2025-06-03 (4,096 tokens)' },
      { value: 'gpt-4o-mini-realtime-preview', label: 'gpt-4o-mini-realtime-preview (16,384 tokens)' },
      { value: 'gpt-4o-mini-realtime-preview-2024-12-17', label: 'gpt-4o-mini-realtime-preview-2024-12-17 (16,384 tokens)' },
      { value: 'gpt-4o-search-preview', label: 'gpt-4o-search-preview (4,096 tokens)' },
      { value: 'gpt-4o-search-preview-2025-03-11', label: 'gpt-4o-search-preview-2025-03-11 (4,096 tokens)' },
      { value: 'gpt-4o-mini-search-preview', label: 'gpt-4o-mini-search-preview (16,384 tokens)' },
      { value: 'gpt-4o-mini-search-preview-2025-03-11', label: 'gpt-4o-mini-search-preview-2025-03-11 (16,384 tokens)' }
  ],
  'OpenAI Realtime Models': [
      { value: 'gpt-realtime', label: 'gpt-realtime (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'gpt-realtime-mini', label: 'gpt-realtime-mini (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' }
  ],
  'OpenAI Audio Models': [
      { value: 'gpt-audio', label: 'gpt-audio (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' },
      { value: 'gpt-audio-mini', label: 'gpt-audio-mini (Context window: 200,000 tokens, Max output tokens: 100,000 tokens)' }
  ],
  'OpenAI Image Models': [
      { value: 'gpt-image-1', label: 'gpt-image-1 (Input tokens, Output tokens)' },
      { value: 'gpt-image-1.5', label: 'gpt-image-1.5 (Input tokens, Output tokens)' },
      { value: 'gpt-image-1-mini', label: 'gpt-image-1-mini (Input tokens, Output tokens)' },
      { value: 'chatgpt-image-latest', label: 'chatgpt-image-latest (Input tokens, Output tokens)' }
  ],
  'OpenAI Codex Models': [
      { value: 'codex-mini-latest', label: 'codex-mini-latest (16,384 tokens)' }
  ],
  'OpenAI Computer Use Models': [
      { value: 'computer-use-preview', label: 'computer-use-preview (16,384 tokens)' },
      { value: 'computer-use-preview-2025-03-11', label: 'computer-use-preview-2025-03-11 (16,384 tokens)' }
  ],
  'OpenAI GPT Turbo Models': [
      { value: 'gpt-4-turbo', label: 'Gpt-4-Turbo (4,096 tokens)' },
      { value: 'gpt-4-turbo-preview', label: 'Gpt-4-Turbo-Preview (4,096 tokens)' },
      { value: 'gpt-4', label: 'Gpt-4 (8,192 tokens)' },
      { value: 'gpt-3.5-turbo', label: 'Gpt-3.5-Turbo (4,096 tokens)' },
      { value: 'gpt-3.5-turbo-0125', label: 'Gpt-3.5-Turbo-0125 (4,096 tokens)' },
      { value: 'gpt-3.5-turbo-16k', label: 'Gpt-3.5-Turbo-16k (16,385 tokens)' }
  ]
};

interface PermissionStatus {
  accessibility: boolean;
  microphone: boolean;
  input_monitoring: boolean;
  screen_capture: boolean;
}

interface ModelStatus {
  installed: boolean;
  path: string;
  size: string;
}

interface ModelInfo {
  id: string;
  name: string;
  description: string;
  url: string;
  size_mb: number;
  model_type: string;
  is_active: boolean;
  installed: boolean;
}

interface OllamaModel {
  name: string;
  size: number;
}

interface AudioDevice {
  id: string;
  label: string;
}

// Convert key event to display string - UNIVERSAL VERSION
const keyEventToString = (e: KeyboardEvent): string => {
  const parts: string[] = [];
  
  // Always show modifier keys if pressed
  if (e.metaKey) parts.push("⌘");
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  
  // Handle ANY key - no restrictions
  let key = e.key;
  
  // Special handling for common keys
  if (key === " ") key = "Space";
  else if (key === "Enter") key = "Enter";
  else if (key === "Escape") key = "Escape";
  else if (key === "Tab") key = "Tab";
  else if (key === "Backspace") key = "Backspace";
  else if (key === "Delete") key = "Delete";
  else if (key === "Insert") key = "Insert";
  else if (key === "Home") key = "Home";
  else if (key === "End") key = "End";
  else if (key === "PageUp") key = "PageUp";
  else if (key === "PageDown") key = "PageDown";
  else if (key === "ArrowUp") key = "↑";
  else if (key === "ArrowDown") key = "↓";
  else if (key === "ArrowLeft") key = "←";
  else if (key === "ArrowRight") key = "→";
  else if (key === "F1") key = "F1";
  else if (key === "F2") key = "F2";
  else if (key === "F3") key = "F3";
  else if (key === "F4") key = "F4";
  else if (key === "F5") key = "F5";
  else if (key === "F6") key = "F6";
  else if (key === "F7") key = "F7";
  else if (key === "F8") key = "F8";
  else if (key === "F9") key = "F9";
  else if (key === "F10") key = "F10";
  else if (key === "F11") key = "F11";
  else if (key === "F12") key = "F12";
  else if (key === "CapsLock") key = "CapsLock";
  else if (key === "NumLock") key = "NumLock";
  else if (key === "ScrollLock") key = "ScrollLock";
  else if (key === "Pause") key = "Pause";
  else if (key === "PrintScreen") key = "PrintScreen";
  else if (key === "ContextMenu") key = "ContextMenu";
  else if (key === "Meta") key = "⌘";
  else if (key === "ContextMenu") key = "Menu";
  else if (key === "OS") key = "OS";
  else if (key === "Super") key = "Super";
  else if (key === "Win") key = "Win";
  else if (key === "Help") key = "Help";
  else if (key === "Select") key = "Select";
  else if (key === "Execute") key = "Execute";
  else if (key === "ZoomIn") key = "ZoomIn";
  else if (key === "ZoomOut") key = "ZoomOut";
  else if (key === "BrightnessUp") key = "BrightnessUp";
  else if (key === "BrightnessDown") key = "BrightnessDown";
  else if (key === "AudioVolumeUp") key = "VolumeUp";
  else if (key === "AudioVolumeDown") key = "VolumeDown";
  else if (key === "AudioVolumeMute") key = "Mute";
  else if (key === "MediaPlay") key = "Play";
  else if (key === "MediaPause") key = "Pause";
  else if (key === "MediaPlayPause") key = "Play/Pause";
  else if (key === "MediaStop") key = "Stop";
  else if (key === "MediaTrackNext") key = "Next";
  else if (key === "MediaTrackPrevious") key = "Prev";
  else if (key === "MediaTrackFirst") key = "First";
  else if (key === "MediaTrackLast") key = "Last";
  else if (key === "MediaRecord") key = "Record";
  else if (key === "Eject") key = "Eject";
  else if (key === "LaunchMail") key = "Mail";
  else if (key === "LaunchApp1") key = "App1";
  else if (key === "LaunchApp2") key = "App2";
  else if (key === "LaunchApp3") key = "App3";
  else if (key === "LaunchApp4") key = "App4";
  else if (key === "LaunchApp5") key = "App5";
  else if (key === "LaunchApp6") key = "App6";
  else if (key === "LaunchApp7") key = "App7";
  else if (key === "LaunchApp8") key = "App8";
  else if (key === "LaunchApp9") key = "App9";
  else if (key === "LaunchApp10") key = "App10";
  else if (key === "LaunchApp11") key = "App11";
  else if (key === "LaunchApp12") key = "App12";
  else if (key === "LaunchApp13") key = "App13";
  else if (key === "LaunchApp14") key = "App14";
  else if (key === "LaunchApp15") key = "App15";
  else if (key === "LaunchApp16") key = "App16";
  else if (key === "LaunchApp17") key = "App17";
  else if (key === "LaunchApp18") key = "App18";
  else if (key === "LaunchApp19") key = "App19";
  else if (key === "LaunchApp20") key = "App20";
  
  // For any other key, use it as-is (including numbers, letters, symbols)
  // No restrictions - accept ANY key
  if (key && key !== "" && key !== "Unidentified" && !["Meta", "Control", "Alt", "Shift"].includes(key)) {
    parts.push(key);
  }
  
  return parts.join(" + ");
};

// Convert key event to storage format - UNIVERSAL VERSION
const keyEventToStorageFormat = (e: KeyboardEvent): string => {
  const parts: string[] = [];
  
  // Always show modifier keys if pressed
  if (e.metaKey || e.ctrlKey) parts.push("CommandOrControl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  
  // Handle ANY key - no restrictions
  let key = e.key;
  
  // Special handling for common keys to ensure compatibility
  if (key === " ") key = "Space";
  else if (key === "Enter") key = "Enter";
  else if (key === "Escape") key = "Escape";
  else if (key === "Tab") key = "Tab";
  else if (key === "Backspace") key = "Backspace";
  else if (key === "Delete") key = "Delete";
  else if (key === "Insert") key = "Insert";
  else if (key === "Home") key = "Home";
  else if (key === "End") key = "End";
  else if (key === "PageUp") key = "PageUp";
  else if (key === "PageDown") key = "PageDown";
  else if (key === "ArrowUp") key = "ArrowUp";
  else if (key === "ArrowDown") key = "ArrowDown";
  else if (key === "ArrowLeft") key = "ArrowLeft";
  else if (key === "ArrowRight") key = "ArrowRight";
  else if (key === "F1") key = "F1";
  else if (key === "F2") key = "F2";
  else if (key === "F3") key = "F3";
  else if (key === "F4") key = "F4";
  else if (key === "F5") key = "F5";
  else if (key === "F6") key = "F6";
  else if (key === "F7") key = "F7";
  else if (key === "F8") key = "F8";
  else if (key === "F9") key = "F9";
  else if (key === "F10") key = "F10";
  else if (key === "F11") key = "F11";
  else if (key === "F12") key = "F12";
  else if (key === "CapsLock") key = "CapsLock";
  else if (key === "NumLock") key = "NumLock";
  else if (key === "ScrollLock") key = "ScrollLock";
  else if (key === "Pause") key = "Pause";
  else if (key === "PrintScreen") key = "PrintScreen";
  else if (key === "ContextMenu") key = "ContextMenu";
  else if (key === "Meta") key = "Command";
  else if (key === "OS") key = "OS";
  else if (key === "Super") key = "Super";
  else if (key === "Win") key = "Win";
  else if (key === "Help") key = "Help";
  else if (key === "Select") key = "Select";
  else if (key === "Execute") key = "Execute";
  else if (key === "ZoomIn") key = "ZoomIn";
  else if (key === "ZoomOut") key = "ZoomOut";
  else if (key === "BrightnessUp") key = "BrightnessUp";
  else if (key === "BrightnessDown") key = "BrightnessDown";
  else if (key === "AudioVolumeUp") key = "AudioVolumeUp";
  else if (key === "AudioVolumeDown") key = "AudioVolumeDown";
  else if (key === "AudioVolumeMute") key = "AudioVolumeMute";
  else if (key === "MediaPlay") key = "MediaPlay";
  else if (key === "MediaPause") key = "MediaPause";
  else if (key === "MediaPlayPause") key = "MediaPlayPause";
  else if (key === "MediaStop") key = "MediaStop";
  else if (key === "MediaTrackNext") key = "MediaTrackNext";
  else if (key === "MediaTrackPrevious") key = "MediaTrackPrevious";
  else if (key === "MediaTrackFirst") key = "MediaTrackFirst";
  else if (key === "MediaTrackLast") key = "MediaTrackLast";
  else if (key === "MediaRecord") key = "MediaRecord";
  else if (key === "Eject") key = "Eject";
  else if (key === "LaunchMail") key = "LaunchMail";
  else if (key === "LaunchApp1") key = "LaunchApp1";
  else if (key === "LaunchApp2") key = "LaunchApp2";
  else if (key === "LaunchApp3") key = "LaunchApp3";
  else if (key === "LaunchApp4") key = "LaunchApp4";
  else if (key === "LaunchApp5") key = "LaunchApp5";
  else if (key === "LaunchApp6") key = "LaunchApp6";
  else if (key === "LaunchApp7") key = "LaunchApp7";
  else if (key === "LaunchApp8") key = "LaunchApp8";
  else if (key === "LaunchApp9") key = "LaunchApp9";
  else if (key === "LaunchApp10") key = "LaunchApp10";
  else if (key === "LaunchApp11") key = "LaunchApp11";
  else if (key === "LaunchApp12") key = "LaunchApp12";
  else if (key === "LaunchApp13") key = "LaunchApp13";
  else if (key === "LaunchApp14") key = "LaunchApp14";
  else if (key === "LaunchApp15") key = "LaunchApp15";
  else if (key === "LaunchApp16") key = "LaunchApp16";
  else if (key === "LaunchApp17") key = "LaunchApp17";
  else if (key === "LaunchApp18") key = "LaunchApp18";
  else if (key === "LaunchApp19") key = "LaunchApp19";
  else if (key === "LaunchApp20") key = "LaunchApp20";
  
  // For any other key, use it as-is (including numbers, letters, symbols)
  // No restrictions - accept ANY key
  if (key && key !== "" && key !== "Unidentified" && !["Meta", "Control", "Alt", "Shift"].includes(key)) {
    parts.push(key);
  }
  
  return parts.join("+");
};

function App() {
  const [settings, setSettings] = useState<Settings>({
    shortcut: "CommandOrControl+Shift+Space",
    push_to_talk_key: "Option+Space",
    auto_copy: true,
    correction_mode: "local",
    ollama_model: "llama3",
    ollama_vision_model: "moondream",
    context_mode: "selection",
    auto_start: false,
    system_prompt: DEFAULT_SYSTEM_PROMPT,
    input_device_id: "default",
    overlay_position: "Bottom",
    openai_model: "gpt-4o-mini",
    openrouter_model: "google/gemini-2.0-flash-exp:free",
    transcription_engine: "parakeet",
    active_model_id: "parakeet-v3",
    style_selector_key: "Option+S",
    highlight_shortcut: "Option+H",
    highlight_mode: "text",
  });
  
  // Ref to keep track of latest settings for event listeners
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  const [saved, setSaved] = useState(false);
  const [permissions, setPermissions] = useState<PermissionStatus>({
    accessibility: false,
    microphone: false,
    input_monitoring: false,
    screen_capture: false,
  });
  const [isRecordingShortcut, setIsRecordingShortcut] = useState(false);
  const [displayShortcut, setDisplayShortcut] = useState("Option + Space");
  const [isRecordingStyleShortcut, setIsRecordingStyleShortcut] = useState(false);
  const [displayStyleShortcut, setDisplayStyleShortcut] = useState("Option + S");
  const [isRecordingRawModeShortcut, setIsRecordingRawModeShortcut] = useState(false);
  const [displayRawModeShortcut, setDisplayRawModeShortcut] = useState("None");
  const [isRecordingHighlightShortcut, setIsRecordingHighlightShortcut] = useState(false);
  const [displayHighlightShortcut, setDisplayHighlightShortcut] = useState("Option + H");
  const [styleLang, setStyleLang] = useState<"en" | "de">("en");
  
  // Audio waveform state
  const [isRecording, setIsRecording] = useState(false);
  
  // Audio devices
  const [audioDevices, setAudioDevices] = useState<AudioDevice[]>([]);
  
  // Model state
  const [_modelStatus, setModelStatus] = useState<ModelStatus>({
    installed: false,
    path: "",
    size: "~478 MB",
  });
  const [keyboardError, setKeyboardError] = useState<string | null>(null);
  
  // Available models
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [modelError, setModelError] = useState<string | null>(null);
  const [activeModelId, setActiveModelId] = useState<string>("parakeet-v3");
  const [downloadingModels, setDownloadingModels] = useState<Set<string>>(new Set());
  const [modelDownloadProgress, setModelDownloadProgress] = useState<Map<string, {downloaded: number, total: number, percentage: number}>>(new Map());
  
  const [isModelsExpanded, setIsModelsExpanded] = useState(false);
  const [openRouterModels, setOpenRouterModels] = useState<OpenRouterModel[]>([]);
  const [openRouterSearch, setOpenRouterSearch] = useState("");
  const [showOpenAIKey, setShowOpenAIKey] = useState(false);
  const [showOpenRouterKey, setShowOpenRouterKey] = useState(false);

  // Ollama state
  const [installedOllamaModels, setInstalledOllamaModels] = useState<Set<string>>(new Set());
  const [downloadingOllamaModel, setDownloadingOllamaModel] = useState<string | null>(null);
  const [ollamaDownloadProgress, setOllamaDownloadProgress] = useState<{status: string, percentage: number} | null>(null);
  const [ollamaDownloadError, setOllamaDownloadError] = useState<string | null>(null);
  const [modelPaths, setModelPaths] = useState<{core: string, text: string, vision: string} | null>(null);

  // Helper to check for Tauri environment
  const isTauri = () => !!(window as any).__TAURI__;

  // Enumerate audio devices
  const refreshAudioDevices = async () => {
    if (!isTauri()) return;
    try {
      const devices = await invoke<AudioDevice[]>("get_available_microphones");
      setAudioDevices(devices);
    } catch (e) {
      console.error("Failed to enumerate devices:", e);
    }
  };

  const checkInstalledOllamaModels = async () => {
    if (!isTauri()) return;
    try {
      const models = await invoke<OllamaModel[]>("get_installed_ollama_models");
      setInstalledOllamaModels(new Set(models.map(m => m.name)));
    } catch (e) {
      console.error("Failed to check installed Ollama models:", e);
      // Attempt to auto-start Ollama if it's not running
      if (String(e).includes("connect to Ollama")) {
          console.log("Ollama not reachable, attempting to start...");
          try {
              const result = await invoke<string>("start_ollama");
              console.log("Start Ollama result:", result);
              // Wait 5 seconds and retry
              setTimeout(() => {
                  checkInstalledOllamaModels();
              }, 5000);
          } catch (startErr) {
               console.error("Failed to auto-start Ollama:", startErr);
               // User requested prompt if not installed/start fails
               if (String(startErr).includes("not found")) {
                   alert("Ollama is not installed or not found in /Applications. Please install Ollama to use local models.");
               }
           }
      }
    }
  };

  const fetchModelPaths = async () => {
    if (!isTauri()) return;
    try {
      const paths = await invoke<{core: string, text: string, vision: string}>("get_model_paths");
      setModelPaths(paths);
    } catch (e) {
      console.error("Failed to fetch model paths:", e);
    }
  };

  useEffect(() => {
    if (!isTauri()) {
        console.log("Browser preview detected: Skipping Tauri setup.");
        return;
    }

    invoke<Settings>("get_settings").then((s) => {
      // Check for outdated prompts and migrate if necessary
      let newSettings = { ...s };
      let needsUpdate = false;

      // Migrate Vision Prompt - Aggressively update if it contains the old restrictive instruction
      if (s.context_mode === "vision" && (s.system_prompt.includes("no markdown") || !s.system_prompt.includes("**bold**"))) {
        newSettings.system_prompt = VISION_SYSTEM_PROMPT;
        needsUpdate = true;
        console.log("🚀 Migrating Vision Prompt to new version allowing markdown/formatting");
      } 
      // Migrate Default Prompt - Update if it doesn't mention bullet points or professional formatting
      else if (s.context_mode !== "vision" && (!s.system_prompt.includes("bullet points") || !s.system_prompt.includes("Do NOT use markdown bolding"))) {
        newSettings.system_prompt = DEFAULT_SYSTEM_PROMPT;
        needsUpdate = true;
        console.log("🚀 Migrating Default Prompt to Professional + Bullets + NO-ASTERISKS version");
      }

      // Migrate Model - Switch to 3B model (balanced) if using 1B (bad) or 8B (slow)
      if (s.ollama_model === "llama3.2:1b" || s.ollama_model === "llama3") {
        newSettings.ollama_model = "llama3.2:3b";
        needsUpdate = true;
        console.log("🚀 Migrating Model to llama3.2:3b for speed/quality balance");
      }

      setSettings(newSettings);
      console.log("Loaded settings:", newSettings);
      setDisplayShortcut(formatShortcutForDisplay(newSettings.push_to_talk_key));
      if (newSettings.style_selector_key) {
        setDisplayStyleShortcut(formatShortcutForDisplay(newSettings.style_selector_key));
      } else {
        setDisplayStyleShortcut("None");
      }

      if (newSettings.highlight_shortcut) {
        setDisplayHighlightShortcut(formatShortcutForDisplay(newSettings.highlight_shortcut));
      } else {
        setDisplayHighlightShortcut("None");
      }
      
      if (newSettings.raw_mode_toggle_key) {
        setDisplayRawModeShortcut(formatShortcutForDisplay(newSettings.raw_mode_toggle_key));
      } else {
        setDisplayRawModeShortcut("None");
      }
      
      if (needsUpdate) {
        invoke("save_settings", { newSettings });
        console.log("Migrated system prompt to new version");
      }
    });
    // Initialize permissions once and avoid repeated mic prompts
    initPermissions();
    checkModelStatus();
    refreshAudioDevices();
    loadAvailableModels();
    checkInstalledOllamaModels();
    fetchOpenRouterModels();
    fetchModelPaths();
    // Listen for model ready event
    const unlistenModelReady = isTauri() ? listen("model-ready", () => {
      checkModelStatus();
    }) : Promise.resolve(() => {});

    // Recording is handled by overlay window - settings just updates UI state
    const unlistenStartRecording = isTauri() ? listen("start-recording", () => {
      console.log("🎤 Settings: recording started (handled by overlay)");
      setIsRecording(true);
    }) : Promise.resolve(() => {});

    const unlistenStopRecording = isTauri() ? listen("stop-recording", () => {
      console.log("🛑 Settings: recording stopped");
      setIsRecording(false);
    }) : Promise.resolve(() => {});

    // Listen for model download progress
    const unlistenDownloadProgress = isTauri() ? listen<{model_id: string, downloaded: number, total: number, percentage: number}>(
      "model-download-progress",
      (event) => {
        const { model_id, downloaded, total, percentage } = event.payload;
        setModelDownloadProgress(prev => new Map(prev).set(model_id, { downloaded, total, percentage }));
      }
    ) : Promise.resolve(() => {});

    // Listen for download complete
    const unlistenDownloadComplete = isTauri() ? listen<string>("model-download-complete", (event) => {
      const modelId = event.payload;
      setDownloadingModels(prev => {
        const next = new Set(prev);
        next.delete(modelId);
        return next;
      });
      setModelDownloadProgress(prev => {
        const next = new Map(prev);
        next.delete(modelId);
        return next;
      });
      loadAvailableModels();
    }) : Promise.resolve(() => {});

    // Listen for keyboard listener issues (global shortcuts)
    const unlistenKeyboardError = isTauri() ? listen<string>("keyboard-error", (event) => {
      const msg = String(event.payload ?? "Keyboard listener failed");
      setKeyboardError(msg);
    }) : Promise.resolve(() => {});

    // Listen for shortcut registration errors
    const unlistenShortcutError = isTauri() ? listen<string>("shortcut-error", (event) => {
      const msg = String(event.payload ?? "Shortcut registration failed");
      setKeyboardError(msg);
    }) : Promise.resolve(() => {});

    // Listen for model not loaded
    const unlistenModelNotLoaded = isTauri() ? listen("model-not-loaded", () => {
      setKeyboardError("Model is not loaded yet. Please wait for the model to download and load.");
    }) : Promise.resolve(() => {});

    // Listen for Ollama download progress
    const unlistenOllamaProgress = isTauri() ? listen<{model: string, status: string, percentage: number}>("ollama-download-progress", (event) => {
      const { model, status, percentage } = event.payload;
      if (status === "success") {
        setDownloadingOllamaModel(null);
        setOllamaDownloadProgress(null);
        checkInstalledOllamaModels();
      } else {
        setDownloadingOllamaModel(model);
        setOllamaDownloadProgress({ status, percentage });
      }
    }) : Promise.resolve(() => {});

    // Listen for device changes
    navigator.mediaDevices.addEventListener("devicechange", refreshAudioDevices);

    return () => {
      unlistenModelReady.then((fn) => fn());
      unlistenStartRecording.then((fn) => fn());
      unlistenStopRecording.then((fn) => fn());
      unlistenDownloadProgress.then((fn) => fn());
      unlistenDownloadComplete.then((fn) => fn());
      unlistenKeyboardError.then((fn) => fn());
      unlistenShortcutError.then((fn) => fn());
      unlistenModelNotLoaded.then((fn) => fn());
      unlistenOllamaProgress.then((fn) => fn());
      navigator.mediaDevices.removeEventListener("devicechange", refreshAudioDevices);
    };
  }, []);

  const fetchOpenRouterModels = async () => {
    if (!isTauri()) return;
    try {
      const response = await invoke<OpenRouterResponse>("fetch_openrouter_models");
      // Sort models: Free first, then by name
      const sortedModels = response.data.sort((a, b) => {
        const isAFree = parseFloat(a.pricing.prompt) === 0 && parseFloat(a.pricing.completion) === 0;
        const isBFree = parseFloat(b.pricing.prompt) === 0 && parseFloat(b.pricing.completion) === 0;
        
        if (isAFree && !isBFree) return -1;
        if (!isAFree && isBFree) return 1;
        return a.name.localeCompare(b.name);
      });
      setOpenRouterModels(sortedModels);
    } catch (e) {
      console.error("Failed to fetch OpenRouter models:", e);
    }
  };

  const checkModelStatus = async () => {
    if (!isTauri()) return;
    try {
      const status = await invoke<ModelStatus>("check_model_status");
      setModelStatus(status);
    } catch (e) {
      console.error("Failed to check model status:", e);
    }
  };

  const loadAvailableModels = async () => {
    if (!isTauri()) return;
    try {
      setModelError(null);
      const models = await invoke<ModelInfo[]>("get_available_models");
      console.log("Loaded models:", models);
      
      // Filter models that are actually installed
      const installedModels = models.filter(m => m.installed);
      console.log("Installed models:", installedModels);
      
      setAvailableModels(models);
      
      // Only set active model if we have installed models
      if (installedModels.length > 0) {
        // Skip active model check on initial load to prevent blocking
        // We will only set it if we are sure it's safe
        /* 
        const activeId = await invoke<string>("get_active_model");
        const isActiveInstalled = installedModels.some(m => m.id === activeId);
        
        if (isActiveInstalled) {
            setActiveModelId(activeId);
        } else {
            console.log(`Active model ${activeId} is not installed.`);
            // Do NOT automatically fallback or load for now to prevent freeze
        }
        */
        // Just set the first installed model as active in UI state without triggering backend load yet
         const activeId = await invoke<string>("get_active_model");
         if (installedModels.some(m => m.id === activeId)) {
             setActiveModelId(activeId);
         }
      } else {
        console.log("No installed models found. Waiting for user action.");
      }
    } catch (e) {
      console.error("Failed to load models:", e);
      setModelError("Failed to load models: " + String(e));
    }
  };

  const formatShortcutForDisplay = (shortcut: string): string => {
    return shortcut
      .replace("CommandOrControl", "⌘")
      .replace("Control", "Ctrl")
      .replace("Alt", "Option")
      .replace("+", " + ")
      .replace(/\+/g, " + ");
  };

  const checkPermissions = async () => {
    if (!isTauri()) return;
    try {
      const perms = await invoke<PermissionStatus>("check_permissions");
      setPermissions(perms);
    } catch (e) {
      console.error("Failed to check permissions:", e);
    }
  };

  // Ensure microphone permission is only requested once (guards React dev double-invoke)
  const micRequestedRef = useRef(false);
  const initPermissions = async () => {
    if (!isTauri()) return;
    try {
      const perms = await invoke<PermissionStatus>("check_permissions");
      setPermissions(perms);
      // Request mic only if not granted and not already requested in this session
      if (!perms.microphone && !micRequestedRef.current) {
        micRequestedRef.current = true;
        try {
          await navigator.mediaDevices.getUserMedia({ audio: true });
          // Refresh status and devices after grant
          checkPermissions();
          refreshAudioDevices();
        } catch (e) {
          console.log("Microphone permission not granted yet");
        }
      }
    } catch (e) {
      console.error("Failed to initialize permissions:", e);
    }
  };

  const requestMicrophonePermission = async () => {
    try {
      await navigator.mediaDevices.getUserMedia({ audio: true });
      checkPermissions();
      refreshAudioDevices();
    } catch (e) {
      console.log("Microphone permission not granted yet");
    }
  };

  const openAccessibilitySettings = async () => {
    if (!isTauri()) return;
    await invoke("open_accessibility_settings");
  };

  // Poll for permissions and check on focus
  useEffect(() => {
    if (!isTauri()) return;

    // Check on window focus (user returning from System Settings)
    const onFocus = () => {
      checkPermissions();
      checkInstalledOllamaModels(); // Also re-check Ollama status
    };
    window.addEventListener("focus", onFocus);

    // Poll every 2 seconds if permissions are missing
    let pollInterval: number | undefined;
    if (!permissions.accessibility || !permissions.microphone) {
      pollInterval = setInterval(() => {
        checkPermissions();
      }, 2000) as unknown as number;
    }

    return () => {
      window.removeEventListener("focus", onFocus);
      if (pollInterval) clearInterval(pollInterval);
    };
  }, [permissions.accessibility, permissions.microphone]);

  const handleSave = async () => {
    if (!isTauri()) {
        console.log("Browser preview: Settings save mocked");
        setSaved(true);
        setTimeout(() => setSaved(false), 2000);
        return;
    }
    await invoke("save_settings", { newSettings: settings });
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleResetPrompt = () => {
    const defaultPrompt = DEFAULT_SYSTEM_PROMPT;
    setSettings({...settings, system_prompt: defaultPrompt});
  };

  const handleToneChange = (tone: string) => {
    if (!tone) return;
    
    let newPrompt = settings.system_prompt;
    
    // Regex to find existing style block (matches both old simple format and new detailed block)
    const styleRegex = /(\n\nStyle\/Tone: [\s\S]*|\n\n\*\*System Message: Style Adherence\*\*[\s\S]*)/;

    if (tone === "REMOVE") {
      if (styleRegex.test(newPrompt)) {
        newPrompt = newPrompt.replace(styleRegex, "").trim();
      }
      setSettings({ ...settings, tone: tone, system_prompt: newPrompt });
      return;
    }

    // Find the human-readable label for the selected tone
    let selectedLabel = tone;
    const currentOptions = TONE_OPTIONS[styleLang];
    for (const group of currentOptions) {
      const found = group.options.find(opt => opt.value === tone);
      if (found) {
        selectedLabel = found.label;
        break;
      }
    }

    // Create a detailed instruction block based on the user's high-performance prompt
    const newBlock = `\n\n**System Message: Style Adherence** - This message serves as guidance for the following task. Rewrite the text in the Style: ${selectedLabel}. Always ensure grammatical correctness, aligning with the desired style. Utilize your discretion to decide if a direct rewrite is optimal or if expansion is necessary to preserve the context and essence of the original text & style.

    !!IMPORTANT!!: Style is always a key factor in ensuring the correctness of the response.

    - if you are presented with random text and you somehow think you are to respond in a conversational way...then you are dead wrong!!!! even "hey, how are you today?" Any text given is meant to be rewritten based on the style, even if you think otherwise. example "how are you today?" you are never to answer to that what are you to do no matter the situation rewrite based on the style. if the same language then you are expected to do what the style depicts. exactly.

    - if you are presented with text and a style that seems as if it needs more then a simple rewrite like for instance(e.g. professional, formal, etc) then you are to definately rewrite that, according to the style provided(do not ask)

      Don't merely replicate the original text. Instead, enrich it to elevate its expression according to the chosen style. In cases where words are written incorrectly in the source text, attempt to assess what is meant and rewrite according to the chosen style. In styles like 'Professional', 'Power of Attorney/Legal', 'Cancellation/Legal', 'Letter of Recommendation', 'Legal', 'Legal Paragraphed', 'Invoice', 'Notice of Termination', 'Medical', 'Diplomatic', include a designated line for signatures if applicable. Otherwise, maintain the document's format and structure to accommodate the selected stylistic. **End of System Message**`;

    if (styleRegex.test(newPrompt)) {
      newPrompt = newPrompt.replace(styleRegex, newBlock);
    } else {
      newPrompt += newBlock;
    }

    setSettings({ ...settings, tone: tone, system_prompt: newPrompt });
  };

  const handleSelectModelPath = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select Model Storage Location"
      });

      if (selected && typeof selected === 'string') {
        // Use ref to get latest settings
        const newSettings = { ...settingsRef.current, custom_models_path: selected };
        setSettings(newSettings);
        
        if (isTauri()) {
          // Save settings
          await invoke("save_settings", { newSettings });
          
          // Configure Ollama
          try {
             const result = await invoke("configure_ollama_path", { path: selected });
             alert(result); // Show success message
             fetchModelPaths(); // Refresh paths
          } catch (e) {
             console.error("Failed to configure Ollama path:", e);
             alert("Failed to update Ollama configuration: " + e);
          }
        }
      }
    } catch (e) {
      console.error("Failed to select directory:", e);
    }
  };

  // Listen for menu change model path
  useEffect(() => {
    if (!isTauri()) return;
    
    const unlisten = listen("menu-change-model-path", () => {
        // We use settingsRef inside handleSelectModelPath so it's safe to call
        handleSelectModelPath();
    });

    return () => {
        unlisten.then(f => f());
    };
  }, []);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    // Check which recording state is active
    if (!isRecordingShortcut && !isRecordingStyleShortcut && !isRecordingRawModeShortcut && !isRecordingHighlightShortcut) return;
    
    e.preventDefault();
    e.stopPropagation();
    
    // Allow Escape to cancel any recording without changes
    if (e.key === "Escape") {
        setIsRecordingShortcut(false);
        setIsRecordingStyleShortcut(false);
        setIsRecordingRawModeShortcut(false);
        setIsRecordingHighlightShortcut(false);
        return;
    }

    // Helper to determine if a key is a modifier
    const isModifierKey = (key: string) => {
      return ["Meta", "Control", "Alt", "Shift", "Command", "Option", "AltGraph", "CapsLock", "Fn", "Symbol", "Hyper", "Super"].includes(key);
    };
    
    // Helper to check if the storage format string consists ONLY of modifiers
    const isOnlyModifiers = (format: string) => {
      const parts = format.split("+").map(p => p.trim());
      const modifiers = ["CommandOrControl", "Alt", "Shift", "Command", "Control", "Meta", "Option"];
      return parts.every(p => modifiers.includes(p));
    };
    
    // Handle Push-to-Talk shortcut recording - UNIVERSAL ACCEPTANCE
    if (isRecordingShortcut) {
      // Accept ANY key combination - no restrictions
      const storageFormat = keyEventToStorageFormat(e);
      const displayFormat = keyEventToString(e);
      
      setDisplayShortcut(displayFormat);

      // Don't save if only modifiers are pressed
      if (isModifierKey(e.key)) return;
      if (isOnlyModifiers(storageFormat)) return;

      // Only save if we actually have a valid key (not just modifiers)
      if (storageFormat.length > 0) {
        const newSettings = {...settings, push_to_talk_key: storageFormat};
        setSettings(newSettings);
        setDisplayShortcut(displayFormat);
        setIsRecordingShortcut(false);
        
        if (isTauri()) {
          invoke("save_settings", { newSettings });
        } else {
          console.log("Browser preview: Shortcut saved (mock)");
        }
      }
      return;
    }
    
    // Handle Style Selector shortcut recording - UNIVERSAL ACCEPTANCE
    if (isRecordingStyleShortcut) {
      // Allow Backspace/Delete to clear the shortcut
      if (e.key === "Backspace" || e.key === "Delete") {
          const newSettings = {...settings, style_selector_key: undefined};
          setSettings(newSettings);
          setDisplayStyleShortcut("None");
          setIsRecordingStyleShortcut(false);
          invoke("save_settings", { newSettings });
          return;
      }
      
      // Accept ANY key combination - no restrictions
      const storageFormat = keyEventToStorageFormat(e);
      const displayFormat = keyEventToString(e);
      
      setDisplayStyleShortcut(displayFormat);

      // Don't save if only modifiers are pressed
      if (isModifierKey(e.key)) return;
      if (isOnlyModifiers(storageFormat)) return;
      
      // Only save if we actually have a valid key (not just modifiers)
      if (storageFormat.length > 0) {
        const newSettings = {...settings, style_selector_key: storageFormat};
        setSettings(newSettings);
        setDisplayStyleShortcut(displayFormat);
        setIsRecordingStyleShortcut(false);
        invoke("save_settings", { newSettings });
      }
      return;
    }
    
    // Handle Raw Mode shortcut recording - UNIVERSAL ACCEPTANCE
    if (isRecordingRawModeShortcut) {
      // Allow Backspace/Delete to clear the shortcut
      if (e.key === "Backspace" || e.key === "Delete") {
          const newSettings = {...settings, raw_mode_toggle_key: undefined};
          setSettings(newSettings);
          setDisplayRawModeShortcut("None");
          setIsRecordingRawModeShortcut(false);
          invoke("save_settings", { newSettings });
          return;
      }
      
      // Accept ANY key combination - no restrictions
      const storageFormat = keyEventToStorageFormat(e);
      const displayFormat = keyEventToString(e);

      setDisplayRawModeShortcut(displayFormat);
      
      // Don't save if only modifiers are pressed
      if (isModifierKey(e.key)) return;
      if (isOnlyModifiers(storageFormat)) return;
      
      // Only save if we actually have a valid key (not just modifiers)
      if (storageFormat.length > 0) {
        const newSettings = {...settings, raw_mode_toggle_key: storageFormat};
        setSettings(newSettings);
        setDisplayRawModeShortcut(displayFormat);
        setIsRecordingRawModeShortcut(false);
        invoke("save_settings", { newSettings });
      }
      return;
    }

    // Handle Highlight Shortcut recording
    if (isRecordingHighlightShortcut) {
      // Allow Backspace/Delete to clear the shortcut
      if (e.key === "Backspace" || e.key === "Delete") {
          const newSettings = {...settings, highlight_shortcut: undefined};
          setSettings(newSettings);
          setDisplayHighlightShortcut("None");
          setIsRecordingHighlightShortcut(false);
          invoke("save_settings", { newSettings });
          return;
      }
      
      // Accept ANY key combination - no restrictions
      const storageFormat = keyEventToStorageFormat(e);
      const displayFormat = keyEventToString(e);

      setDisplayHighlightShortcut(displayFormat);
      
      // Don't save if only modifiers are pressed
      if (isModifierKey(e.key)) return;
      if (isOnlyModifiers(storageFormat)) return;
      
      // Only save if we actually have a valid key (not just modifiers)
      if (storageFormat.length > 0) {
        const newSettings = {...settings, highlight_shortcut: storageFormat};
        setSettings(newSettings);
        setDisplayHighlightShortcut(displayFormat);
        setIsRecordingHighlightShortcut(false);
        invoke("save_settings", { newSettings });
      }
      return;
    }
  }, [isRecordingShortcut, isRecordingStyleShortcut, isRecordingRawModeShortcut, isRecordingHighlightShortcut, settings]);

  useEffect(() => {
    // Only set up listener if any recording state is active
    if (isRecordingShortcut || isRecordingStyleShortcut || isRecordingRawModeShortcut || isRecordingHighlightShortcut) {
      window.addEventListener("keydown", handleKeyDown);
      return () => window.removeEventListener("keydown", handleKeyDown);
    }
  }, [isRecordingShortcut, isRecordingStyleShortcut, isRecordingRawModeShortcut, isRecordingHighlightShortcut, handleKeyDown]);

  // Global cleanup function to ensure no listeners remain
  useEffect(() => {
    return () => {
      // This ensures listeners are cleaned up when component unmounts
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleKeyDown]);

  // Ensure proper cleanup when component unmounts to prevent any stuck listeners
  useEffect(() => {
    return () => {
      // Final cleanup to ensure no global shortcuts interfere
      setIsRecordingShortcut(false);
      setIsRecordingStyleShortcut(false);
      setIsRecordingRawModeShortcut(false);
      setIsRecordingHighlightShortcut(false);
    };
  }, []);

    // Listen for open-settings request (e.g. from dock icon or tray)
    useEffect(() => {
        const unlisten = listen('open-settings', (event) => {
            console.log("Opening settings request received:", event);
            // Ensure window is visible/focused (backend handles this too, but redundancy helps)
            // If we had a router or tabs, we would switch here.
            // Since the main app IS the settings, we just log for now.
        });

        return () => {
            unlisten.then(f => f());
        };
    }, []);

    // Listen for settings updates from backend (e.g. from global shortcuts or other windows)
  useEffect(() => {
    const unlisten = listen<Settings>('settings-updated', (event) => {
      console.log("Settings updated from backend:", event.payload);
      setSettings(event.payload);
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  const startRecordingShortcut = () => {
    // Ensure all other recording states are cleared
    setIsRecordingStyleShortcut(false);
    setIsRecordingRawModeShortcut(false);
    
    setIsRecordingShortcut(true);
    setDisplayShortcut("Press your shortcut...");
  };

  const startRecordingStyleShortcut = () => {
    // Ensure all other recording states are cleared
    setIsRecordingShortcut(false);
    setIsRecordingRawModeShortcut(false);
    
    setIsRecordingStyleShortcut(true);
    setDisplayStyleShortcut("Press shortcut (Backspace to clear)...");
  };

  const startRecordingRawModeShortcut = () => {
    // Ensure all other recording states are cleared
    setIsRecordingShortcut(false);
    setIsRecordingStyleShortcut(false);
    
    setIsRecordingRawModeShortcut(true);
    setDisplayRawModeShortcut("Press shortcut (Backspace to clear)...");
  };

  const startRecordingHighlightShortcut = () => {
    // Ensure all other recording states are cleared
    setIsRecordingShortcut(false);
    setIsRecordingStyleShortcut(false);
    setIsRecordingRawModeShortcut(false);
    
    setIsRecordingHighlightShortcut(true);
    setDisplayHighlightShortcut("Press shortcut (Backspace to clear)...");
  };

  // Reset individual shortcuts
  const resetPushToTalkShortcut = async () => {
    const defaultShortcut = "CommandOrControl+Shift+Space";
    const newSettings = {...settings, push_to_talk_key: defaultShortcut};
    setSettings(newSettings);
    setDisplayShortcut(formatShortcutForDisplay(defaultShortcut));
    
    if (isTauri()) {
      try {
        await invoke("save_settings", { newSettings });
      } catch (e) {
        console.error("Failed to reset push-to-talk shortcut:", e);
        // Fallback or alert user
      }
    }
  };

  const resetStyleShortcut = async () => {
    // Use Option as default for Mac users
    const defaultShortcut = "Option+S"; 
    const newSettings = {...settings, style_selector_key: defaultShortcut};
    setSettings(newSettings);
    setDisplayStyleShortcut(formatShortcutForDisplay(defaultShortcut));
    
    if (isTauri()) {
      try {
        await invoke("save_settings", { newSettings });
      } catch (e) {
        console.error("Failed to reset style shortcut:", e);
      }
    }
  };

  const resetRawModeShortcut = async () => {
    const newSettings = {...settings, raw_mode_toggle_key: undefined};
    setSettings(newSettings);
    setDisplayRawModeShortcut("None");
    
    if (isTauri()) {
      try {
        await invoke("save_settings", { newSettings });
      } catch (e) {
        console.error("Failed to reset raw mode shortcut:", e);
      }
    }
  };

  const resetHighlightShortcut = async () => {
    const defaultShortcut = "Option+H";
    const newSettings = {...settings, highlight_shortcut: defaultShortcut};
    setSettings(newSettings);
    setDisplayHighlightShortcut(formatShortcutForDisplay(defaultShortcut));
    
    if (isTauri()) {
      try {
        await invoke("save_settings", { newSettings });
      } catch (e) {
        console.error("Failed to reset highlight shortcut:", e);
      }
    }
  };

  // Reset all shortcuts at once
  const resetAllShortcuts = async () => {
    const newSettings = {
      ...settings,
      push_to_talk_key: "CommandOrControl+Shift+Space",
      style_selector_key: "Option+S",
      raw_mode_toggle_key: undefined,
      highlight_shortcut: "Option+H"
    };
    setSettings(newSettings);
    setDisplayShortcut(formatShortcutForDisplay("CommandOrControl+Shift+Space"));
    setDisplayStyleShortcut(formatShortcutForDisplay("Option+S"));
    setDisplayRawModeShortcut("None");
    setDisplayHighlightShortcut(formatShortcutForDisplay("Option+H"));
    
    if (isTauri()) {
      try {
        await invoke("save_settings", { newSettings });
      } catch (e) {
        console.error("Failed to reset all shortcuts:", e);
      }
    }
  };

  const allPermissionsGranted = permissions.accessibility && permissions.microphone;
  const activeModel = availableModels.find(m => m.id === activeModelId);
  const isReady = allPermissionsGranted && (activeModel?.installed ?? false);

  return (
    <div className="app">
      <header>
        <img src={logo} alt="Vocal Typewriter Logo" className="settings-logo" />
        <h1>Vocal Typewriter</h1>
        <p className="subtitle">Speech-to-text settings</p>
      </header>

      <div className={"status-bar " + (isReady ? "" : "warning") + (isRecording ? " recording" : "")}>
        <div className={"status-dot " + (isReady ? "" : "warning") + (isRecording ? " recording" : "")} />
        <span>
          {isRecording
            ? "Recording..."
            : isReady
              ? "Ready - Hold shortcut to dictate"
              : !activeModel?.installed
                ? "Download a model to get started"
                : !permissions.accessibility
                  ? "Accessibility permission required - Click Settings"
                  : !permissions.microphone
                    ? "Microphone permission required"
                    : "Not Ready"}
        </span>
      </div>

      <StatsDisplay />

      {/* Available Models Section */}
      <section className="section models-section">
        <button 
          className={`models-header-btn ${isModelsExpanded ? 'expanded' : ''}`}
          onClick={() => setIsModelsExpanded(!isModelsExpanded)}
        >
          <div className="header-content">
            <h2>Models</h2>
            <span className="active-model-summary">
              {activeModel?.installed 
                ? `Active: ${activeModel.name}`
                : "No Model Loaded"}
            </span>
          </div>
          <span className="toggle-icon">{isModelsExpanded ? "▲" : "▼"}</span>
        </button>

        {isModelsExpanded && (
          <div className="models-accordion-content">
            {modelError && (
              <div className="error-warning" style={{padding: '12px', margin: '12px'}}>
                <p>{modelError}</p>
              </div>
            )}
            <div className="models-list">
              {availableModels.map((model) => (
                <div 
                  key={model.id}
                  className={`model-item ${model.id === activeModelId ? 'active' : ''}`}
                >
                  <div className="model-info-row">
                    <div className="model-main-info">
                      <div className="model-name-row">
                        <h3>{model.name}</h3>
                        {model.id === activeModelId && <span className="active-badge">Active</span>}
                      </div>
                      <p className="model-desc">{model.description}</p>
                      <span className="model-meta">Size: {model.size_mb} MB • {model.model_type}</span>
                    </div>

                    <div className="model-actions">
                      {downloadingModels.has(model.id) ? (
                        <div className="download-progress-compact">
                          <span className="progress-text-compact">
                            {modelDownloadProgress.get(model.id)?.percentage.toFixed(0) || 0}%
                          </span>
                          <div className="progress-bar-compact">
                            <div 
                              className="progress-fill"
                              style={{width: `${modelDownloadProgress.get(model.id)?.percentage || 0}%`}} 
                            />
                          </div>
                        </div>
                      ) : (
                        <button 
                          className={`action-btn ${model.installed && model.id === activeModelId ? 'ready' : model.installed ? 'select' : 'download'}`}
                          onClick={async (e) => {
                            e.stopPropagation();
                            if (model.installed) {
                              if (model.id !== activeModelId) {
                                 try {
                                   await invoke("load_model_by_id", { modelId: model.id });
                                   setActiveModelId(model.id);
                                   const engine = model.model_type.toLowerCase().includes('parakeet') ? 'parakeet' : 'whisper';
                                   setSettings(prev => ({...prev, active_model_id: model.id, transcription_engine: engine}));
                                   checkModelStatus();
                                 } catch (error) {
                                   console.error(`Failed to select ${model.name}:`, error);
                                 }
                              }
                            } else {
                               setDownloadingModels(prev => new Set(prev).add(model.id));
                               try {
                                 await invoke("download_model_by_id", { modelId: model.id });
                               } catch (error) {
                                 console.error(`Failed to download ${model.name}:`, error);
                                 setDownloadingModels(prev => {
                                   const next = new Set(prev);
                                   next.delete(model.id);
                                   return next;
                                 });
                               }
                            }
                          }}
                        >
                          {model.installed ? (model.id === activeModelId ? "Active" : "Select") : "Download"}
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>

            <div style={{marginTop: '20px', borderTop: '1px solid var(--border-color)', paddingTop: '15px'}}>
                 <button 
                    className="action-btn danger" 
                    onClick={async () => {
                        if (window.confirm("Are you sure you want to delete all downloaded models? This will require re-downloading a model to continue.")) {
                            try {
                                await invoke("reset_models");
                                // Refresh status
                                await checkModelStatus();
                                await loadAvailableModels();
                            } catch (e) {
                                console.error("Failed to reset models:", e);
                                setModelError("Failed to reset models: " + e);
                            }
                        }
                    }}
                    style={{width: '100%', backgroundColor: '#d9534f', marginTop: '5px'}}
                 >
                    Reset / Delete All Models
                 </button>
                 <p className="description" style={{fontSize: '0.85em', marginTop: '8px', color: 'var(--text-secondary)'}}>
                    Use this if you are experiencing issues with model loading or installation.
                 </p>
            </div>
          </div>
        )}
      </section>

      {/* Audio Device Selection */}
      <section className="section">
        <h2>Input Device</h2>
        <p className="description" style={{marginBottom: '12px'}}>
          Select the microphone to use for recording.
        </p>
        <select 
          value={settings.input_device_id}
          onChange={async (e) => {
            const deviceId = e.target.value;
            setSettings({...settings, input_device_id: deviceId});
            try {
              await invoke("set_selected_microphone", { deviceName: deviceId });
              console.log("Microphone switched to:", deviceId);
            } catch (err) {
              console.error("Failed to switch microphone:", err);
            }
          }}
        >
          <option value="default">System Default</option>
          {audioDevices.map(device => (
            <option key={device.id} value={device.id}>
              {device.label}
            </option>
          ))}
        </select>
        <button className="check-btn" onClick={refreshAudioDevices} style={{marginTop: '8px'}}>
          Refresh Devices
        </button>
      </section>

      <section className="section">
        <h2>Reset Shortcuts</h2>
        <p className="description" style={{marginBottom: '12px'}}>
          If you encounter issues or crashes, use this to restore default shortcuts.
        </p>
        <button 
          className="action-btn" 
          onClick={resetAllShortcuts}
          style={{width: '100%', backgroundColor: '#d9534f'}}
        >
          Reset All Shortcuts to Defaults
        </button>
      </section>

      <section className="section">
        <h2>Push-to-Talk Shortcut</h2>
        <p className="description" style={{marginBottom: '12px'}}>
          Click the box below and press your desired key combination.
        </p>
        <div style={{display: 'flex', flexDirection: 'column', gap: '5px', marginBottom: '12px'}}>
          <div style={{display: 'flex', justifyContent: 'flex-end'}}>
             <button className="reset-icon-btn" onClick={resetPushToTalkShortcut} title="Reset to default: Cmd+Shift+Space">
               <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                 <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 12" />
                 <path d="M3 5v7h7" />
               </svg>
             </button>
          </div>
          <div 
            className={"shortcut-recorder " + (isRecordingShortcut ? "recording" : "")}
            onClick={startRecordingShortcut}
            tabIndex={0}
            style={{width: '100%'}}
          >
            <span className="shortcut-keys">{displayShortcut}</span>
            <span className="shortcut-hint">
              {isRecordingShortcut ? "Listening..." : "Click to change"}
            </span>
          </div>
        </div>
      </section>

      <section className="section">
        <h2>Style Selector Shortcut</h2>
        <p className="description" style={{marginBottom: '12px'}}>
          Shortcut to open the style selector sidebar.
        </p>

        <button 
          className="action-btn" 
          onClick={() => invoke('toggle_style_window')}
          style={{marginBottom: '12px', width: '100%', backgroundColor: '#4a90e2'}}
        >
          Open Style Selector Sidebar
        </button>

        <div style={{display: 'flex', flexDirection: 'column', gap: '5px', marginBottom: '12px'}}>
          <div style={{display: 'flex', justifyContent: 'flex-end'}}>
             <button className="reset-icon-btn" onClick={resetStyleShortcut} title="Reset to default: Option+S">
               <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                 <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 12" />
                 <path d="M3 5v7h7" />
               </svg>
             </button>
          </div>
          <div 
            className={"shortcut-recorder " + (isRecordingStyleShortcut ? "recording" : "")}
            onClick={startRecordingStyleShortcut}
            tabIndex={0}
            style={{width: '100%'}}
          >
            <span className="shortcut-keys">{displayStyleShortcut}</span>
            <span className="shortcut-hint">
              {isRecordingStyleShortcut ? "Listening..." : "Click to change"}
            </span>
          </div>
        </div>
      </section>

      <section className="section">
        <h2>Raw Mode Toggle Shortcut</h2>
        <p className="description" style={{marginBottom: '12px'}}>
          Shortcut to toggle between Raw Mode (no AI) and your last AI Mode.
        </p>
        <div style={{display: 'flex', flexDirection: 'column', gap: '5px', marginBottom: '12px'}}>
          <div style={{display: 'flex', justifyContent: 'flex-end'}}>
             <button className="reset-icon-btn" onClick={resetRawModeShortcut} title="Clear shortcut">
               <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                 <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 12" />
                 <path d="M3 5v7h7" />
               </svg>
             </button>
          </div>
          <div 
            className={"shortcut-recorder " + (isRecordingRawModeShortcut ? "recording" : "")}
            onClick={startRecordingRawModeShortcut}
            tabIndex={0}
            style={{width: '100%'}}
          >
            <span className="shortcut-keys">{displayRawModeShortcut}</span>
            <span className="shortcut-hint">
              {isRecordingRawModeShortcut ? "Listening..." : "Click to change"}
            </span>
          </div>
        </div>
      </section>

      <section className="section">
        <h2>Highlight Text Context Shortcut</h2>
        <p className="description" style={{marginBottom: '12px'}}>
          Highlight text before speaking. AI will use it as context for editing.
        </p>
        
        <div style={{marginBottom: '12px'}}>
          <label style={{display: 'block', marginBottom: '4px', fontSize: '0.9em', color: '#888'}}>Action Mode:</label>
          <select 
            value={settings.highlight_mode || "text"}
            onChange={async (e) => {
              const newSettings = {...settings, highlight_mode: e.target.value};
              setSettings(newSettings);
              if (isTauri()) {
                await invoke("save_settings", { newSettings });
              }
            }}
            style={{width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid #444', backgroundColor: '#2a2a2a', color: 'white'}}
          >
            <option value="text">Text Selection (Read highlighted text)</option>
            <option value="screenshot">Screenshot (Analyze screen content)</option>
          </select>
        </div>

        <div style={{display: 'flex', flexDirection: 'column', gap: '5px', marginBottom: '12px'}}>
          <div style={{display: 'flex', justifyContent: 'flex-end'}}>
             <button className="reset-icon-btn" onClick={resetHighlightShortcut} title="Reset to default: Option+H">
               <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                 <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 12" />
                 <path d="M3 5v7h7" />
               </svg>
             </button>
          </div>
          <div 
            className={"shortcut-recorder " + (isRecordingHighlightShortcut ? "recording" : "")}
            onClick={startRecordingHighlightShortcut}
            tabIndex={0}
            style={{width: '100%'}}
          >
            <span className="shortcut-keys">{displayHighlightShortcut}</span>
            <span className="shortcut-hint">
              {isRecordingHighlightShortcut ? "Listening..." : "Click to change"}
            </span>
          </div>
        </div>
      </section>

      <section className="section">
        <h2>Transcription Engine</h2>
        <p className="description" style={{marginBottom: '12px'}}>
          Choose the speech recognition backend.
        </p>
        <select 
          value={settings.transcription_engine || "parakeet"}
          onChange={async (e) => {
            const engine = e.target.value;
            setSettings({...settings, transcription_engine: engine});
            try {
              await invoke("set_transcription_engine", { engine });
              console.log("Engine switched to:", engine);
            } catch (err) {
              console.error("Failed to switch engine:", err);
            }
          }}
        >
          <option value="parakeet">Parakeet (Fast, Native)</option>
        </select>
        <p className="hint" style={{marginTop: '8px'}}>
          Parakeet is optimized for speed and runs fully locally.
        </p>
      </section>

      <section className="section">
        <h2>Storage</h2>
        <p className="description" style={{marginBottom: '12px'}}>
          Choose where to store large AI models (Ollama).
        </p>
        
        <div className="storage-control">
            <div className="path-display" style={{
                background: 'var(--bg-secondary)',
                padding: '8px 12px',
                borderRadius: '6px',
                fontSize: '12px',
                fontFamily: 'monospace',
                marginBottom: '8px',
                wordBreak: 'break-all',
                border: '1px solid var(--border-color)'
            }}>
                {settings.custom_models_path || "Default (~/.ollama/models)"}
            </div>
            
            <div style={{display: 'flex', gap: '8px'}}>
                <button 
                    className="action-btn"
                    onClick={async () => {
                        try {
                            const selected = await open({
                                directory: true,
                                multiple: false,
                                title: "Select Models Folder"
                            });
                            
                            if (selected && typeof selected === 'string') {
                                const newSettings = {...settings, custom_models_path: selected};
                                setSettings(newSettings);
                                if (isTauri()) {
                                    await invoke("save_settings", { newSettings });
                                    await invoke("restart_app");
                                }
                            }
                        } catch (err) {
                            console.error("Failed to select folder:", err);
                        }
                    }}
                >
                    Change Folder
                </button>
                
                {settings.custom_models_path && (
                    <button 
                        className="action-btn secondary"
                        onClick={async () => {
                            const newSettings = {...settings, custom_models_path: undefined};
                            setSettings(newSettings);
                            if (isTauri()) {
                                await invoke("save_settings", { newSettings });
                                await invoke("restart_app");
                            }
                        }}
                    >
                        Reset to Default
                    </button>
                )}
            </div>
        </div>
        
        <p className="hint" style={{marginTop: '8px'}}>
          Useful if you want to store models on an external drive. App will restart automatically.
        </p>
      </section>

      <section className="section">
        <h2>Context Awareness</h2>
        <p className="description" style={{marginBottom: '12px'}}>
          How the AI understands what you're working on.
        </p>
        <select 
          value={settings.context_mode}
          onChange={(e) => {
            const newMode = e.target.value;
            const newPrompt = newMode === "vision" ? VISION_SYSTEM_PROMPT : DEFAULT_SYSTEM_PROMPT;
            setSettings({...settings, context_mode: newMode, system_prompt: newPrompt});
          }}
        >
          <option value="none">None - Just transcribe speech</option>
          <option value="selection">Selection - Read highlighted text (Cmd+C)</option>
          <option value="vision">Vision - See screen via screenshot</option>
        </select>
        <p className="hint" style={{marginTop: '8px'}}>
          {settings.context_mode === "selection" && "Highlight text before speaking. AI will use it as context for editing."}
          {settings.context_mode === "vision" && "AI takes a screenshot to understand your document's style and context."}
          {settings.context_mode === "none" && "Basic mode - speech is transcribed and formatted without context."}
        </p>
      </section>

      <section className="section">
        <h2>Correction Mode</h2>

        <select 
          value={settings.correction_mode}
          onChange={async (e) => {
            const newSettings = {...settings, correction_mode: e.target.value};
            setSettings(newSettings);
            if (isTauri()) {
              await invoke("save_settings", { newSettings });
            }
          }}
        >
          <option value="none">None (Raw Transcription)</option>
          <option value="basic">Basic (Punctuation & Capitalization)</option>
          <option value="local">Local LLM (Ollama - Free & Private)</option>
          <option value="openai">OpenAI (Cloud)</option>
          <option value="openrouter">OpenRouter (Cloud)</option>
        </select>

        {settings.correction_mode === "openai" && (
            <div className="cloud-section">
                <label>OpenAI Model</label>
                <select 
                  value={settings.openai_model}
                  onChange={async (e) => {
                    const newSettings = {...settings, openai_model: e.target.value};
                    setSettings(newSettings);
                    if (isTauri()) {
                      await invoke("save_settings", { newSettings });
                    }
                  }}
                  className="model-select"
                >
                  {Object.entries(OPENAI_MODELS).map(([groupLabel, models]) => (
                    <optgroup key={groupLabel} label={groupLabel}>
                      {models.map(model => (
                        <option key={model.value} value={model.value}>
                          {model.label}
                        </option>
                      ))}
                    </optgroup>
                  ))}
                </select>
                <div className="label-row">
                  <label>OpenAI API Key</label>
                  <button 
                    className="password-toggle-btn-top"
                    onClick={() => setShowOpenAIKey(!showOpenAIKey)}
                    title={showOpenAIKey ? "Hide API Key" : "Show API Key"}
                  >
                    {showOpenAIKey ? (
                      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"></path><line x1="1" y1="1" x2="23" y2="23"></line></svg>
                    ) : (
                      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path><circle cx="12" cy="12" r="3"></circle></svg>
                    )}
                  </button>
                </div>
                <input 
                    type={showOpenAIKey ? "text" : "password"}
                    placeholder="sk-..."
                    value={settings.openai_api_key || ""}
                    onChange={(e) => setSettings({...settings, openai_api_key: e.target.value})}
                />
            </div>
        )}

        {settings.correction_mode === "openrouter" && (
            <div className="cloud-section">
                <label>OpenRouter Model</label>
                <div style={{fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '6px', marginTop: '-4px'}}>
                  Type the word "free" to see the free models
                </div>
                <input
                  type="text"
                  placeholder="Search 400+ models..."
                  value={openRouterSearch}
                  onChange={(e) => setOpenRouterSearch(e.target.value)}
                  style={{marginBottom: '8px'}}
                />
                <select 
                  value={settings.openrouter_model}
                  onChange={async (e) => {
                    const newSettings = {...settings, openrouter_model: e.target.value};
                    setSettings(newSettings);
                    if (isTauri()) {
                      await invoke("save_settings", { newSettings });
                    }
                  }}
                  className="model-select"
                >
                  {(() => {
                    const filtered = openRouterModels.filter(m => 
                      m.name.toLowerCase().includes(openRouterSearch.toLowerCase()) || 
                      m.id.toLowerCase().includes(openRouterSearch.toLowerCase())
                    );
                    
                    const grouped = filtered.reduce((acc, model) => {
                      const company = model.id.split('/')[0] || 'Other';
                      const companyLabel = company.charAt(0).toUpperCase() + company.slice(1);
                      if (!acc[companyLabel]) acc[companyLabel] = [];
                      acc[companyLabel].push(model);
                      return acc;
                    }, {} as Record<string, OpenRouterModel[]>);
                    
                    return Object.entries(grouped)
                      .sort(([a], [b]) => a.localeCompare(b))
                      .map(([company, models]) => (
                        <optgroup key={company} label={company}>
                          {models.map(model => (
                            <option key={model.id} value={model.id}>
                              {model.name}
                            </option>
                          ))}
                        </optgroup>
                      ));
                  })()}
                </select>
                <div className="label-row">
                  <label>OpenRouter API Key</label>
                  <button 
                    className="password-toggle-btn-top"
                    onClick={() => setShowOpenRouterKey(!showOpenRouterKey)}
                    title={showOpenRouterKey ? "Hide API Key" : "Show API Key"}
                  >
                    {showOpenRouterKey ? (
                      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"></path><line x1="1" y1="1" x2="23" y2="23"></line></svg>
                    ) : (
                      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path><circle cx="12" cy="12" r="3"></circle></svg>
                    )}
                  </button>
                </div>
                <input 
                    type={showOpenRouterKey ? "text" : "password"}
                    placeholder="sk-or-..."
                    value={settings.openrouter_api_key || ""}
                    onChange={(e) => setSettings({...settings, openrouter_api_key: e.target.value})}
                />
            </div>
        )}
        
        {settings.correction_mode === "local" && (
           <div className="ollama-section">
             <label>Ollama Model</label>
            <div style={{display: 'flex', gap: '10px', alignItems: 'center', marginBottom: '8px'}}>
              <select 
                value={settings.ollama_model}
                onChange={async (e) => {
                  const newSettings = {...settings, ollama_model: e.target.value};
                  setSettings(newSettings);
                  if (isTauri()) {
                    await invoke("save_settings", { newSettings });
                  }
                }}
                style={{flex: 1, marginBottom: 0}}
              >
                <optgroup label="Recommended (Balanced)">
                  <option value="llama3.2:3b">Llama 3.2 3B (~2GB) - Best Balance</option>
                  <option value="phi3.5">Phi-3.5 Mini (3.8B) - Very Smart & Compact</option>
                  <option value="qwen2.5:3b">Qwen 2.5 3B (~2GB) - Smart & Fast</option>
                  <option value="qwen2.5-coder:1.5b">Qwen 2.5 Coder 1.5B (~1GB) - Code Optimized</option>
                </optgroup>
                <optgroup label="Lightweight (Fastest)">
                  <option value="qwen2.5:0.5b">Qwen 2.5 0.5B (~400MB) - Ultra Fast</option>
                  <option value="qwen2.5:1.5b">Qwen 2.5 1.5B (~1GB) - Lightning Fast</option>
                  <option value="llama3.2:1b">Llama 3.2 1B (~1GB) - Basic Speed</option>
                  <option value="gemma2:2b">Gemma 2 2B (~1.5GB) - Google Fast</option>
                </optgroup>
                <optgroup label="Heavyweight (Smartest - Requires 8GB+ RAM)">
                  <option value="mistral:7b">Mistral 7B (~4GB) - Reliable</option>
                  <option value="gemma2:9b">Gemma 2 9B (~5.5GB) - Very Smart</option>
                  <option value="qwen2.5:7b">Qwen 2.5 7B (~4.5GB) - High Performance</option>
                  <option value="qwen2.5:14b">Qwen 2.5 14B (~9GB) - Very Powerful</option>
                  <option value="qwen2.5:32b">Qwen 2.5 32B (~19GB) - State of the Art</option>
                  <option value="qwen2.5-coder:7b">Qwen 2.5 Coder 7B (~4.5GB) - Code Pro</option>
                  <option value="qwen2.5-coder:14b">Qwen 2.5 Coder 14B (~9GB) - Expert Coder</option>
                  <option value="qwen2.5-coder:32b">Qwen 2.5 Coder 32B (~19GB) - Master Coder</option>
                  <option value="llama3">Llama 3 8B (~4.7GB) - Classic</option>
                </optgroup>
                <optgroup label="Vision Models (Multimodal)">
                  <option value="moondream">Moondream (~1GB) - Fast Vision</option>
                  <option value="qwen2.5vl:3b">Qwen 2.5-VL 3B (~2GB) - Balanced Vision</option>
                  <option value="llava:7b">LLaVA 7B (~4.5GB) - General Vision</option>
                  <option value="llama3.2-vision">Llama 3.2 Vision (~7GB) - Latest</option>
                  <option value="minicpm-v">MiniCPM-V (~2.5GB) - Efficient</option>
                </optgroup>
              </select>
              
              {!installedOllamaModels.has(settings.ollama_model) && !downloadingOllamaModel && (
                <button 
                  className="action-btn download"
                  style={{padding: '8px 16px', height: '40px', whiteSpace: 'nowrap'}}
                  onClick={async () => {
                    setDownloadingOllamaModel(settings.ollama_model);
                    setOllamaDownloadError(null);
                    setOllamaDownloadProgress({ status: "Starting download...", percentage: 0 });
                    try {
                      await invoke("download_ollama_model", { model: settings.ollama_model });
                    } catch (e) {
                      console.error("Failed to download model:", e);
                      setOllamaDownloadError(String(e));
                      setDownloadingOllamaModel(null);
                      setOllamaDownloadProgress(null);
                    }
                  }}
                >
                  Download
                </button>
              )}
              
              {installedOllamaModels.has(settings.ollama_model) && (
                <span className="badge success" style={{
                  padding: '4px 8px', 
                  borderRadius: '4px', 
                  background: '#e6f4ea', 
                  color: '#137333', 
                  fontSize: '12px',
                  fontWeight: 500,
                  whiteSpace: 'nowrap'
                }}>
                  ✓ Installed
                </span>
              )}
            </div>

            {downloadingOllamaModel === settings.ollama_model && ollamaDownloadProgress && (
              <div className="download-progress-compact" style={{marginBottom: '10px'}}>
                <div style={{display: 'flex', justifyContent: 'space-between', marginBottom: '4px', fontSize: '12px'}}>
                  <span>{ollamaDownloadProgress.status}</span>
                  <span>{ollamaDownloadProgress.percentage.toFixed(0)}%</span>
                </div>
                <div className="progress-bar-compact">
                  <div 
                    className="progress-fill"
                    style={{width: `${ollamaDownloadProgress.percentage}%`}} 
                  />
                </div>
              </div>
            )}

            {ollamaDownloadError && (
               <div className="error-message" style={{marginBottom: '10px', color: '#d93025', fontSize: '12px', padding: '8px', background: '#fce8e6', borderRadius: '4px', border: '1px solid #fad2cf', whiteSpace: 'pre-wrap'}}>
                 ❌ <strong>Download Failed:</strong><br/>
                 {ollamaDownloadError}
               </div>
            )}

            <p className="hint">
              {settings.ollama_model.includes("llava") || settings.ollama_model.includes("vision") || settings.ollama_model.includes("moondream")
                ? "Vision model selected - can see screenshots for context"
                : "Text model selected - use 'Selection' context mode for best results"}
            </p>
            {!installedOllamaModels.has(settings.ollama_model) && !downloadingOllamaModel && (
               <p className="hint" style={{marginTop: '4px', color: '#d93025'}}>
                 ⚠️ Model not installed. Click Download to get it.
               </p>
            )}
            
            <div className="ollama-section" style={{marginTop: '20px'}}>
              <label>Vision Model (for Screenshots)</label>
              <div style={{display: 'flex', gap: '10px', alignItems: 'center', marginBottom: '8px'}}>
                <select
                  value={settings.ollama_vision_model}
                  onChange={async (e) => {
                    const newSettings = {...settings, ollama_vision_model: e.target.value};
                    setSettings(newSettings);
                    if (isTauri()) {
                      await invoke("save_settings", { newSettings });
                    }
                  }}
                  style={{flex: 1, marginBottom: 0}}
                >
                  <optgroup label="Gemma 3 (New & Recommended)">
                    <option value="gemma3:4b">Gemma 3 4B (~3GB) - Fast & Efficient</option>
                    <option value="gemma3:12b">Gemma 3 12B (~8GB) - Balanced</option>
                    <option value="gemma3:27b">Gemma 3 27B (~16GB) - High Performance</option>
                  </optgroup>
                  <optgroup label="CPU-Friendly">
                    <option value="moondream">Moondream (~1GB) - Ultra Fast</option>
                    <option value="minicpm-v">MiniCPM-V (~2.5GB) - Efficient</option>
                  </optgroup>
                  <optgroup label="Heavier (More Capable)">
                    <option value="llava:13b">LLaVA 13B (~8GB) - Better vision</option>
                    <option value="llama3.2-vision">Llama 3.2 Vision (~7GB) - Latest</option>
                    <option value="gemma3:4b">Gemma 3 4B (~3–4GB) - Multimodal</option>
                    <option value="llava-llama3">LLaVA-Llama3 (~6–8GB) - Fine-tuned</option>
                    <option value="llava-phi3">LLaVA-Phi3 (~3.8GB) - Small</option>
                    <option value="granite3.2-vision">Granite 3.2 Vision (~2–4GB) - Docs</option>
                  </optgroup>
                  <optgroup label="Specialized">
                    <option value="deepseek-ocr">DeepSeek-OCR (~2–3GB) - OCR</option>
                  </optgroup>
                </select>
                
                {!installedOllamaModels.has(settings.ollama_vision_model) && !downloadingOllamaModel && (
                  <button 
                    className="action-btn download"
                    style={{padding: '8px 16px', height: '40px', whiteSpace: 'nowrap'}}
                    onClick={async () => {
                      setDownloadingOllamaModel(settings.ollama_vision_model);
                      setOllamaDownloadError(null);
                      setOllamaDownloadProgress({ status: "Starting download...", percentage: 0 });
                      try {
                        await invoke("download_ollama_model", { model: settings.ollama_vision_model });
                      } catch (e) {
                        console.error("Failed to download model:", e);
                        setOllamaDownloadError(String(e));
                        setDownloadingOllamaModel(null);
                      }
                    }}
                  >
                    Download
                  </button>
                )}
                
                {installedOllamaModels.has(settings.ollama_vision_model) && (
                  <span className="badge success" style={{
                    padding: '4px 8px', 
                    borderRadius: '4px', 
                    background: '#e6f4ea', 
                    color: '#137333', 
                    fontSize: '12px',
                    fontWeight: 500,
                    whiteSpace: 'nowrap'
                  }}>
                    ✓ Installed
                  </span>
                )}
              </div>
              
              {downloadingOllamaModel === settings.ollama_vision_model && ollamaDownloadProgress && (
                <div className="download-progress-compact" style={{marginBottom: '10px'}}>
                  <div style={{display: 'flex', justifyContent: 'space-between', marginBottom: '4px', fontSize: '12px'}}>
                    <span>{ollamaDownloadProgress.status}</span>
                    <span>{ollamaDownloadProgress.percentage.toFixed(0)}%</span>
                  </div>
                  <div className="progress-bar-compact">
                    <div 
                      className="progress-fill"
                      style={{width: `${ollamaDownloadProgress.percentage}%`}} 
                    />
                  </div>
                </div>
              )}

              {ollamaDownloadError && (
                 <div className="error-message" style={{marginBottom: '10px', color: '#d93025', fontSize: '12px', padding: '8px', background: '#fce8e6', borderRadius: '4px', border: '1px solid #fad2cf', whiteSpace: 'pre-wrap'}}>
                   ❌ <strong>Download Failed:</strong><br/>
                   {ollamaDownloadError}
                 </div>
              )}
              
              <p className="hint">
                Screenshot processing uses the Vision Model automatically.
              </p>
              {!installedOllamaModels.has(settings.ollama_vision_model) && !downloadingOllamaModel && (
                 <p className="hint" style={{marginTop: '4px', color: '#d93025'}}>
                   ⚠️ Vision model not installed. Click Download to get it.
                 </p>
              )}
            </div>
          </div>
        )}
      </section>

      <section className="section">
        <h2>Storage</h2>
        <p className="description" style={{marginBottom: '12px'}}>
          Choose where to store AI models (Ollama and Local).
        </p>
        <div style={{display: 'flex', flexDirection: 'column', gap: '12px'}}>
           {/* Core Models Path */}
           <div style={{display: 'flex', flexDirection: 'column', gap: '4px'}}>
             <label style={{fontSize: '12px', color: '#888', fontWeight: 600}}>Core Models (Whisper/Parakeet)</label>
             <div style={{padding: '8px', background: '#2a2a2a', borderRadius: '4px', border: '1px solid #444', fontSize: '13px', color: '#ccc', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap'}}>
               {modelPaths?.core || "Loading..."}
             </div>
           </div>

           {/* Text Models Path */}
           <div style={{display: 'flex', flexDirection: 'column', gap: '4px'}}>
             <label style={{fontSize: '12px', color: '#888', fontWeight: 600}}>Text Models (Ollama)</label>
             <div style={{padding: '8px', background: '#2a2a2a', borderRadius: '4px', border: '1px solid #444', fontSize: '13px', color: '#ccc', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap'}}>
               {modelPaths?.text || "Loading..."}
             </div>
           </div>

           {/* Vision Models Path */}
           <div style={{display: 'flex', flexDirection: 'column', gap: '4px'}}>
             <label style={{fontSize: '12px', color: '#888', fontWeight: 600}}>Vision Models (Ollama)</label>
             <div style={{padding: '8px', background: '#2a2a2a', borderRadius: '4px', border: '1px solid #444', fontSize: '13px', color: '#ccc', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap'}}>
               {modelPaths?.vision || "Loading..."}
             </div>
           </div>

           {/* Change Folder Button */}
           <div style={{marginTop: '4px'}}>
             <button onClick={handleSelectModelPath} style={{width: '100%', padding: '10px', cursor: 'pointer', background: '#444', border: 'none', borderRadius: '4px', color: 'white', fontWeight: 600}}>
               Change System Default Storage Folder...
             </button>
           </div>
           
           {/* Hint */}
           {settings.custom_models_path && (
             <p className="hint" style={{color: '#faad14'}}>
               ⚠️ Note: Changing the folder for Ollama requires restarting the Ollama application. Existing models in the old location will not be automatically moved.
             </p>
           )}
        </div>
      </section>

      <section className="section">
        <h2>Target Language</h2>
        <p className="description" style={{marginBottom: '12px'}}>
            If selected, the AI will default to translating input into this language.
            <br/>
            <span style={{fontSize: '0.9em', color: '#666'}}>(Applies to Local, OpenAI, and OpenRouter modes)</span>
        </p>
        <select 
            value={settings.target_language || ""}
            onChange={async (e) => {
                const newSettings = {...settings, target_language: e.target.value};
                setSettings(newSettings);
                if (isTauri()) {
                    await invoke("save_settings", { newSettings });
                }
            }}
        >
            <option value="">None (No Target Language)</option>
            <optgroup label="Favorites 🌟💖👍"> 
                <option value="en">English</option> 
                <option value="de">German</option> 
                <option value="fa">Farsi</option> 
                <option value="no">Norwegian</option> 
                <option value="tr">Turkish</option> 
                <option value="uk">Ukrainian</option> 
                <option value="el">Greek</option> 
            </optgroup> 
            <optgroup label="Main List 🌐🔤✨"> 
                <option value="af">Afrikaans</option> 
                <option value="sq">Albanian</option> 
                <option value="ar">Arabic</option> 
                <option value="hy">Armenian</option> 
                <option value="az">Azerbaijani</option> 
                <option value="eu">Basque</option> 
                <option value="be">Belarusian</option> 
                <option value="bn">Bengali</option> 
                <option value="bg">Bulgarian</option> 
                <option value="ca">Catalan</option> 
                <option value="zh-CN">Cantonese (CN)</option> 
                <option value="zh-HK">Cantonese (HK)</option> 
                <option value="cs">Czech</option> 
                <option value="da">Danish</option> 
                <option value="nl">Dutch</option> 
                <option value="en">English</option> 
                <option value="et">Estonian</option> 
                <option value="fi">Finnish</option> 
                <option value="fr">French</option> 
                <option value="gl">Galician</option> 
                <option value="ka">Georgian</option> 
                <option value="de">German</option> 
                <option value="el">Greek</option> 
                <option value="gu">Gujarati</option> 
                <option value="ht">Haitian Creole</option> 
                <option value="he">Hebrew</option> 
                <option value="hi">Hindi</option> 
                <option value="hu">Hungarian</option> 
                <option value="is">Icelandic</option> 
                <option value="id">Indonesian</option> 
                <option value="ga">Irish</option> 
                <option value="it">Italian</option> 
                <option value="ja">Japanese</option> 
                <option value="jv">Javanese</option> 
                <option value="kn">Kannada</option> 
                <option value="kk">Kazakh</option> 
                <option value="km">Khmer</option> 
                <option value="rw">Kinyarwanda</option> 
                <option value="ko">Korean</option> 
                <option value="ku">Kurdish (Kurmanji)</option> 
                <option value="ky">Kyrgyz</option> 
                <option value="lo">Lao</option> 
                <option value="la">Latin</option> 
                <option value="lv">Latvian</option> 
                <option value="lt">Lithuanian</option> 
                <option value="lb">Luxembourgish</option> 
                <option value="mk">Macedonian</option> 
                <option value="mg">Malagasy</option> 
                <option value="ms">Malay</option> 
                <option value="ml">Malayalam</option> 
                <option value="mt">Maltese</option> 
                <option value="mi">Maori</option> 
                <option value="mr">Marathi</option> 
                <option value="mn">Mongolian</option> 
                <option value="my">Myanmar (Burmese)</option> 
                <option value="ne">Nepali</option> 
                <option value="no">Norwegian</option> 
                <option value="or">Odia (Oriya)</option> 
                <option value="ps">Pashto</option> 
                <option value="fa">Persian</option> 
                <option value="pl">Polish</option> 
                <option value="pt">Portuguese</option> 
                <option value="pa">Punjabi</option> 
                <option value="ro">Romanian</option> 
                <option value="ru">Russian</option> 
                <option value="sm">Samoan</option> 
                <option value="gd">Scots Gaelic</option> 
                <option value="sr">Serbian</option> 
                <option value="st">Sesotho</option> 
                <option value="sn">Shona</option> 
                <option value="sd">Sindhi</option> 
                <option value="si">Sinhala</option> 
                <option value="sk">Slovak</option> 
                <option value="sl">Slovenian</option> 
                <option value="so">Somali</option> 
                <option value="es">Spanish</option> 
                <option value="su">Sundanese</option> 
                <option value="sw">Swahili</option> 
                <option value="sv">Swedish</option> 
                <option value="tg">Tajik</option> 
                <option value="ta">Tamil</option> 
                <option value="tt">Tatar</option> 
                <option value="te">Telugu</option> 
                <option value="th">Thai</option> 
                <option value="bo">Tibetan</option> 
                <option value="ti">Tigrinya</option> 
                <option value="to">Tongan</option> 
                <option value="tr">Turkish</option> 
                <option value="tk">Turkmen</option> 
                <option value="uk">Ukrainian</option> 
                <option value="ur">Urdu</option> 
                <option value="ug">Uyghur</option> 
                <option value="uz">Uzbek</option> 
                <option value="vi">Vietnamese</option> 
                <option value="cy">Welsh</option> 
                <option value="xh">Xhosa</option> 
                <option value="yi">Yiddish</option> 
                <option value="yo">Yoruba</option> 
                <option value="zu">Zulu</option> 
            </optgroup> 
        </select>
      </section>

      {(settings.correction_mode === "local" || settings.correction_mode === "openai" || settings.correction_mode === "openrouter") && (
        <section className="section">
          <h2>AI System Prompt</h2>
          <p className="description" style={{marginBottom: '12px'}}>
            Customize how the AI formats and corrects your speech.
          </p>
          
          <div style={{marginBottom: '15px'}}>
            <label style={{display: 'block', marginBottom: '5px', fontWeight: 500}}>User Name (Optional)</label>
            <input 
              type="text" 
              placeholder="Enter your name for personalized AI responses..."
              value={settings.user_name || ""}
              onChange={(e) => setSettings({...settings, user_name: e.target.value})}
              style={{width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid #ccc'}}
            />
            <p className="description" style={{fontSize: '0.85em', marginTop: '4px', color: '#666'}}>
              If provided, this name will be shared with the AI model to personalize responses.
            </p>
          </div>
          
          <div style={{marginBottom: '10px'}}>
             <div style={{display: 'flex', gap: '10px', marginBottom: '8px'}}>
               <label style={{fontSize: '0.9em', cursor: 'pointer'}}>
                 <input 
                   type="radio" 
                   name="styleLang" 
                   checked={styleLang === 'en'} 
                   onChange={() => setStyleLang('en')}
                   style={{marginRight: '4px'}}
                 />
                 English
               </label>
               <label style={{fontSize: '0.9em', cursor: 'pointer'}}>
                 <input 
                   type="radio" 
                   name="styleLang" 
                   checked={styleLang === 'de'} 
                   onChange={() => setStyleLang('de')}
                   style={{marginRight: '4px'}}
                 />
                 German
               </label>
             </div>
             <select 
                onChange={(e) => handleToneChange(e.target.value)}
                value=""
                style={{width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid #ccc', marginBottom: '8px'}}
              >
                <option value="" disabled>Select a style to append...</option>
                <option value="REMOVE">❌ Remove Style</option>
                {TONE_OPTIONS[styleLang].map(group => (
                  <optgroup key={group.label} label={group.label}>
                    {group.options.map(opt => (
                      <option key={opt.value} value={opt.value}>{opt.label}</option>
                    ))}
                  </optgroup>
                ))}
              </select>
          </div>

          <textarea 
            value={settings.system_prompt}
            onChange={(e) => setSettings({...settings, system_prompt: e.target.value})}
            rows={8}
            placeholder="Enter system prompt..."
          />
          <button className="reset-btn" onClick={handleResetPrompt}>
            Reset to Default
          </button>
        </section>
      )}

      {!allPermissionsGranted && (
        <section className="section permission-warning">
          <h2>⚠️ Permissions Required</h2>
          <p className="description" style={{marginBottom: '16px'}}>
            Grant these permissions to enable dictation. You may need to restart the app after granting.
          </p>
          <div className="permission-list">
            <div className={"permission-item " + (permissions.microphone ? "granted" : "missing")}>
              <span>{permissions.microphone ? "✓" : "✗"}</span>
              <span>Microphone Access</span>
              {!permissions.microphone && (
                <button className="grant-btn" onClick={requestMicrophonePermission}>Grant</button>
              )}
            </div>
            <div className={"permission-item " + (permissions.accessibility ? "granted" : "missing")}>
              <span>{permissions.accessibility ? "✓" : "✗"}</span>
              <span>Accessibility (for typing text)</span>
              {!permissions.accessibility && (
                <button className="grant-btn" onClick={openAccessibilitySettings}>Open Settings</button>
              )}
            </div>
            <div className={"permission-item " + (permissions.input_monitoring ? "granted" : "missing")}>
              <span>{permissions.input_monitoring ? "✓" : "✗"}</span>
              <span>Input Monitoring (for keyboard shortcuts)</span>
              {!permissions.input_monitoring && (
                <button className="grant-btn" onClick={async () => await invoke("open_input_monitoring_settings")}>Open Settings</button>
              )}
            </div>
          </div>
        </section>
      )}

      {keyboardError && (
        <section className="section error-warning">
          <h2>⚠️ Error</h2>
          <p className="description">{keyboardError}</p>
        </section>
      )}

      <section className="section">
        <h2>Options</h2>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={settings.auto_copy}
            onChange={(e) => setSettings({...settings, auto_copy: e.target.checked})}
          />
          <span>Also copy transcript to clipboard</span>
        </label>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={settings.auto_start}
            onChange={(e) => setSettings({...settings, auto_start: e.target.checked})}
          />
          <span>Start at login</span>
        </label>
      </section>

      <button className="save-btn" onClick={handleSave}>
        {saved ? "✓ Saved!" : "Save Settings"}
      </button>
      
      <div style={{marginTop: '20px', padding: '16px', border: '1px solid var(--border-color)', borderRadius: '8px', backgroundColor: 'var(--bg-secondary)'}}>
        <h3 style={{margin: '0 0 10px 0', fontSize: '14px', color: 'var(--text-secondary)'}}>🔧 TROUBLESHOOTING</h3>
        <div style={{display: 'flex', gap: '10px', flexWrap: 'wrap'}}>
          <button className="reset-btn" onClick={resetAllShortcuts} style={{backgroundColor: '#ff4d4f', color: 'white', border: 'none', padding: '8px 16px', borderRadius: '4px', cursor: 'pointer'}}>
            Reset All Shortcuts
          </button>
          <span style={{fontSize: '12px', color: 'var(--text-secondary)', alignSelf: 'center', marginLeft: '10px'}}>
            Use this if shortcuts get stuck or unresponsive
          </span>
        </div>
      </div>
      

    </div>
  );
}

export default App;
