use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tauri::{AppHandle, Emitter};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
}

fn replace_placeholders(text: &str, user_name: &Option<String>) -> String {
    println!("🧹 replace_placeholders called. user_name: {:?}", user_name);
    let mut result = text.to_string();
    if let Some(name) = user_name {
        if name.trim().is_empty() {
            return result;
        }

        let placeholders = [
            // Square brackets
            "[Your Name]", "[your name]", "[Your name]", "[your Name]",
            "[Name]", "[name]", 
            "[Insert Name]", "[insert name]", "[Insert name]",
            "[Your Name Here]", "[your name here]", "[Your name here]",
            // Angle brackets
            "<Your Name>", "<your name>", "<Your name>",
            "<Name>", "<name>",
            // Curly braces
            "{Your Name}", "{your name}", "{Your name}",
            "{Name}", "{name}"
        ];
        
        for placeholder in placeholders {
            result = result.replace(placeholder, name);
        }
    }
    result
}

/// unified cleaning function to remove system prompts and conversational filler
fn clean_model_output(text: &str) -> String {
    let mut clean_text = text.to_string();

    // 1. Remove System Prompt Leakage
    // Look for the specific header used in Style Selector
    if let Some(start_idx) = clean_text.find("System Message: Style Adherence") {
        if let Some(end_marker) = clean_text[start_idx..].find("End of System Message") {
            let end_idx = start_idx + end_marker + "End of System Message".len();
            // Remove the block, including the following newline if present
            let suffix = &clean_text[end_idx..];
            let suffix_trimmed = suffix.trim_start(); 
            // If the leak was at the start, we just keep the rest
            if start_idx < 10 { // Allow for some whitespace/markdown chars before
                 clean_text = suffix_trimmed.to_string();
            } else {
                 // Leak in middle (unlikely but possible), remove the chunk
                 clean_text.replace_range(start_idx..end_idx, "");
                 clean_text = clean_text.trim().to_string();
            }
            println!("🧹 Removed leaked system prompt from output");
        }
    }

    // 2. Clean up "Here is the text" prefixes
    clean_text = clean_text
        .lines()
        .skip_while(|line| {
            let l = line.trim().to_lowercase();
            l.starts_with("here is") || 
            l.starts_with("sure") || 
            l.starts_with("okay") ||
            l.starts_with("output:") ||
            l.starts_with("context:") ||
            l.is_empty()
        })
        .filter(|line| {
            let l = line.trim().to_lowercase();
            !l.starts_with("let me know") &&
            !l.starts_with("hope this") &&
            !l.starts_with("feel free") &&
            !l.starts_with("is there anything") &&
            !l.starts_with("do you need")
        })
        .collect::<Vec<&str>>()
        .join("\n");

    // 2.5 Clean up "Translation to X:" prefix if present (e.g. "Translation to English: ...")
    // This handles cases where the label and text are on the same line
    let lower_clean = clean_text.to_lowercase();
    if lower_clean.starts_with("translation to") || lower_clean.starts_with("translation:") {
        if let Some(colon_idx) = clean_text.find(':') {
             // ensure the colon is within reasonable distance to avoid false positives
             if colon_idx < 50 {
                 clean_text = clean_text[colon_idx + 1..].trim().to_string();
             }
        }
    }

    // 3. Force remove markdown bolding (asterisks) as strictly requested
    clean_text = clean_text
        .replace("**", "")
        .replace("##", "") // Remove headers too just in case
        .trim()
        .to_string();

    clean_text
}

pub fn perform_paste(enigo: &mut Enigo) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // macOS: Cmd+V
        enigo
            .key(Key::Meta, Direction::Press)
            .map_err(|e| format!("Failed to press Cmd: {}", e))?;
        // Delay between modifiers
        std::thread::sleep(std::time::Duration::from_millis(50));
        
        // Press V
        enigo
            .key(Key::Unicode('v'), Direction::Press)
            .map_err(|e| format!("Failed to press V: {}", e))?;
            
        // Hold briefly
        std::thread::sleep(std::time::Duration::from_millis(50));
        
        // Release V
        enigo
            .key(Key::Unicode('v'), Direction::Release)
            .map_err(|e| format!("Failed to release V: {}", e))?;
            
        // Delay before releasing modifier
        std::thread::sleep(std::time::Duration::from_millis(50));
        
        enigo
            .key(Key::Meta, Direction::Release)
            .map_err(|e| format!("Failed to release Cmd: {}", e))?;
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        // Windows/Linux: Ctrl+V
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| format!("Failed to press Ctrl: {}", e))?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| format!("Failed to press V: {}", e))?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| format!("Failed to release Ctrl: {}", e))?;
    }
    Ok(())
}



/// Type text directly using keyboard simulation (slower but works everywhere)
pub fn type_string(text: &str) -> Result<(), String> {
    let settings = Settings::default();
    let mut enigo = Enigo::new(&settings).map_err(|e| e.to_string())?;
    enigo.text(text).map_err(|e| e.to_string())?;
    Ok(())
}

/// Paste text using clipboard + keyboard shortcut (faster and more reliable)
/// This is the method Handy uses
pub fn paste_text(text: &str, app: &AppHandle) -> Result<(), String> {
    // Save current clipboard content
    let clipboard = app.clipboard();
    let saved_clipboard = match clipboard.read_text() {
        Ok(text) => text,
        Err(_) => String::new(),
    };
    
    // Write our text to clipboard
    clipboard
        .write_text(text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;
    
    // Increased delay to ensure clipboard is ready and previous app has focus
    std::thread::sleep(std::time::Duration::from_millis(300));
    
    // Send Cmd+V (macOS) or Ctrl+V (Windows/Linux)
    let settings = Settings::default();
    let mut enigo = Enigo::new(&settings).map_err(|e| e.to_string())?;
    perform_paste(&mut enigo)?;
    
    // Delay for paste to complete (increased significantly for slower systems/recording)
    std::thread::sleep(std::time::Duration::from_millis(2000));
    
    // Restore original clipboard content
    if !saved_clipboard.is_empty() {
        let _ = clipboard.write_text(&saved_clipboard);
    }
    
    Ok(())
}



/// Apply basic formatting: capitalize sentences, add period at end if missing
pub fn format_basic(text: &str) -> String {
    let mut text = text.trim().to_string();
    if text.is_empty() {
        return String::new();
    }

    // Basic filler word removal (case-insensitive)
    let fillers = [" um ", " uh ", " Um ", " Uh "];
    for filler in fillers {
        text = text.replace(filler, " ");
    }
    
    // Fix double spaces
    while text.contains("  ") {
        text = text.replace("  ", " ");
    }
    
    let mut result = String::new();
    let mut capitalize_next = true;
    
    for c in text.chars() {
        if capitalize_next && c.is_alphabetic() {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
        
        // Capitalize after sentence-ending punctuation
        if c == '.' || c == '!' || c == '?' {
            capitalize_next = true;
        }
    }
    
    // Add period at end if no ending punctuation
    let last_char = result.chars().last().unwrap_or(' ');
    if !matches!(last_char, '.' | '!' | '?' | ':' | ';' | ',') {
        result.push('.');
    }
    
    result
}

/// Process text with AI using Ollama (for editing, optimization, translation)
pub async fn process_with_ai(
    text: &str,
    action: &str,
    system_prompt: &str,
    model: &str,
    context: Option<String>,
    user_name: Option<String>,
    target_language: Option<String>,
    image_context: Option<String>,
) -> Result<String, String> {
    let mut effective_system_prompt = if let Some(name) = &user_name {
        if name.trim().is_empty() {
            system_prompt.to_string()
        } else {
            format!("The user's name is {0}. \nCRITICAL INSTRUCTION: You are acting on behalf of {0}. In any situation where a name, signature, self-reference, or personal identity is needed, you MUST use '{0}'. NEVER use placeholders like '[Your Name]', '[Name]', or generic terms. If the text implies a speaker or sender, assume it is {0}. If you are unsure, default to using '{0}'.\n{1}", name, system_prompt)
        }
    } else {
        system_prompt.to_string()
    };

    if let Some(lang) = &target_language {
        if !lang.is_empty() && lang != "none" {
             effective_system_prompt = format!("{}\n\nTARGET LANGUAGE INSTRUCTION:\nThe user has explicitly selected a target language: {}.\nYour task is to ensure the FINAL OUTPUT is in {}.\n- If the input is a request to translate, translate it to {}.\n- If the input is general text to be fixed/edited, fix/edit it and then output the result in {}.\n- Do NOT chat or explain. Just output the content in {}.", effective_system_prompt, lang, lang, lang, lang, lang);
        }
    }

    let prompt = if let Some(ctx) = context {
        format!("Context:\n{}\n\nInstruction:\n{}", ctx, text)
    } else {
        match action {
            "translate" => format!("Translate the following text to English. Output ONLY the translation, nothing else:\n\n{}", text),
            "optimize" => format!("Optimize and improve the following text. Fix grammar, punctuation, and make it more professional. Output ONLY the improved text:\n\n{}", text),
            "edit" => text.to_string(),
            _ => text.to_string(),
        }
    };
    
    // Call Ollama API
    println!("🤖 Processing with model: {}", model);
    let client = reqwest::Client::new();
    
    // Construct payload using 'system' parameter for better adherence
    let request_body = if let Some(img) = image_context {
        serde_json::json!({
            "model": model,
            "prompt": prompt,
            "system": effective_system_prompt,
            "stream": false,
            "images": [img]
        })
    } else {
        serde_json::json!({
            "model": model,
            "prompt": prompt,
            "system": effective_system_prompt,
            "stream": false
        })
    };

    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Ollama returned error: {}", response.status()));
    }
    
    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Ollama response: {}", e))?;
    
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;
    
    let result = json["response"]
        .as_str()
        .unwrap_or(text)
        .trim()
        .to_string();
    
    // Safety Check: Llama models sometimes refuse to process text if they think it's harmful.
    // If we detect a refusal, fallback to basic formatting.
    let lower_result = result.to_lowercase();
    if lower_result.starts_with("i can't") || 
       lower_result.starts_with("i cannot") || 
       lower_result.contains("harmful or illegal") ||
       lower_result.contains("child exploitation") ||
       lower_result.contains("hate speech") ||
       lower_result.contains("unable to comply") {
        eprintln!("⚠️ AI Refusal detected. Falling back to basic formatting. Output was: {}", result);
        return Err("AI refused to process text".to_string());
    }

    // Clean up using unified function (handles system prompt leakage and prefixes)
    let clean_result = clean_model_output(&result);

    let final_result = replace_placeholders(&clean_result, &user_name);

    Ok(final_result)
}

pub async fn process_with_openai(
    text: &str,
    system_prompt: &str,
    model: &str,
    api_key: &str,
    context: Option<String>,
    user_name: Option<String>,
    target_language: Option<String>,
    image_context: Option<String>,
) -> Result<String, String> {
    // 1. Configure client with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    
    let mut effective_system_prompt = if let Some(name) = &user_name {
        if name.trim().is_empty() {
            system_prompt.to_string()
        } else {
            format!("The user's name is {0}. \nCRITICAL INSTRUCTION: You are acting on behalf of {0}. In any situation where a name, signature, self-reference, or personal identity is needed, you MUST use '{0}'. NEVER use placeholders like '[Your Name]', '[Name]', or generic terms. If the text implies a speaker or sender, assume it is {0}. If you are unsure, default to using '{0}'.\n{1}", name, system_prompt)
        }
    } else {
        system_prompt.to_string()
    };

    if let Some(lang) = &target_language {
        if !lang.is_empty() && lang != "none" {
             effective_system_prompt = format!("{}\n\nTARGET LANGUAGE INSTRUCTION:\nThe user has explicitly selected a target language: {}.\nYour task is to ensure the FINAL OUTPUT is in {}.\n- If the input is a request to translate, translate it to {}.\n- If the input is general text to be fixed/edited, fix/edit it and then output the result in {}.\n- Do NOT chat or explain. Just output the content in {}.", effective_system_prompt, lang, lang, lang, lang, lang);
        }
    }

    // Helper to build request body
    let build_body = |include_image: bool| {
        if include_image && image_context.is_some() {
             let img = image_context.as_ref().unwrap();
             // Multimodal request
            let text_content = if let Some(ctx) = &context {
                format!("Context:\n{}\n\nInstruction:\n{}", ctx, text)
            } else {
                text.to_string()
            };

            // OpenAI Vision structure:
            // Use separate system message for better model adherence
            let mut messages = Vec::new();
            
            if !effective_system_prompt.trim().is_empty() {
                messages.push(serde_json::json!({
                    "role": "system",
                    "content": effective_system_prompt
                }));
            }

            messages.push(serde_json::json!({
                "role": "user",
                "content": [
                    { "type": "text", "text": text_content },
                    { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{}", img) } }
                ]
            }));

            let body = serde_json::Value::Array(messages);
            
            println!("🚀 [DEBUG] OpenAI Vision Request Body (truncated): {{ ... \"content\": [ {{ \"type\": \"text\" ... }}, {{ \"type\": \"image_url\", \"image_url\": {{ \"url\": \"data:image/png;base64,{}...\" }} }} ] }}", &img[0..20]);
            body
        } else if let Some(ctx) = &context {
            serde_json::json!([
                { "role": "system", "content": effective_system_prompt },
                { "role": "user", "content": format!("Context:\n{}\n\nInstruction:\n{}", ctx, text) }
            ])
        } else {
            serde_json::json!([
                { "role": "system", "content": effective_system_prompt },
                { "role": "user", "content": text }
            ])
        }
    };

    // 2. Try with image first (if present)
    let messages = build_body(true);

    let result = match client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": model,
            "messages": messages
        }))
        .send()
        .await {
            Ok(response) => {
                if !response.status().is_success() {
                    let error_text = response.text().await.unwrap_or_default();
                    
                    // 3. Fallback: If failed and had image, retry without image
                    if image_context.is_some() {
                        eprintln!("⚠️ OpenAI Vision request failed ({}). Retrying without image...", error_text);
                        let messages_no_img = build_body(false);
                         match client
                            .post("https://api.openai.com/v1/chat/completions")
                            .header("Authorization", format!("Bearer {}", api_key))
                            .json(&serde_json::json!({
                                "model": model,
                                "messages": messages_no_img
                            }))
                            .send()
                            .await {
                                Ok(retry_resp) => {
                                    if !retry_resp.status().is_success() {
                                        let retry_err = retry_resp.text().await.unwrap_or_default();
                                        return Err(format!("OpenAI retry failed: {}", retry_err));
                                    }
                                    retry_resp
                                },
                                Err(e) => return Err(format!("OpenAI retry connection failed: {}", e))
                            }
                    } else {
                        return Err(format!("OpenAI returned error: {}", error_text));
                    }
                } else {
                    response
                }
            },
            Err(e) => return Err(format!("OpenAI request failed: {}", e))
        };

    let body: serde_json::Value = result
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

    println!("📥 [DEBUG] OpenAI Response Body: {}", body);

    let content_val = &body["choices"][0]["message"]["content"];
    let result_text = if let Some(s) = content_val.as_str() {
        s.to_string()
    } else if let Some(arr) = content_val.as_array() {
        arr.iter()
           .filter_map(|item| item["text"].as_str())
           .collect::<Vec<&str>>()
           .join("")
    } else {
        return Err("Invalid OpenAI response structure: 'content' is missing or invalid".to_string());
    };
    
    let result_text = result_text.trim().to_string();

    // Clean up using unified function
    let clean_result = clean_model_output(&result_text);

    let final_text = replace_placeholders(&clean_result, &user_name);

    Ok(final_text)
}

pub async fn process_with_openrouter(
    text: &str,
    system_prompt: &str,
    model: &str,
    api_key: &str,
    context: Option<String>,
    user_name: Option<String>,
    target_language: Option<String>,
    image_context: Option<String>,
) -> Result<String, String> {
    // 1. Configure client with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    
    let mut effective_system_prompt = if let Some(name) = &user_name {
        if name.trim().is_empty() {
            system_prompt.to_string()
        } else {
            format!("The user's name is {0}. \nCRITICAL INSTRUCTION: You are acting on behalf of {0}. In any situation where a name, signature, self-reference, or personal identity is needed, you MUST use '{0}'. NEVER use placeholders like '[Your Name]', '[Name]', or generic terms. If the text implies a speaker or sender, assume it is {0}. If you are unsure, default to using '{0}'.\n{1}", name, system_prompt)
        }
    } else {
        system_prompt.to_string()
    };

    if let Some(lang) = &target_language {
        if !lang.is_empty() && lang != "none" {
             effective_system_prompt = format!("{}\n\nTARGET LANGUAGE INSTRUCTION:\nThe user has explicitly selected a target language: {}.\nYour task is to ensure the FINAL OUTPUT is in {}.\n- If the input is a request to translate, translate it to {}.\n- If the input is general text to be fixed/edited, fix/edit it and then output the result in {}.\n- Do NOT chat or explain. Just output the content in {}.", effective_system_prompt, lang, lang, lang, lang, lang);
        }
    }

    // Helper to build request body
    let build_body = |include_image: bool| {
        if include_image && image_context.is_some() {
             let img = image_context.as_ref().unwrap();
             // Multimodal request
            let text_content = if let Some(ctx) = &context {
                format!("Context:\n{}\n\nInstruction:\n{}", ctx, text)
            } else {
                text.to_string()
            };

            // OpenRouter Vision structure:
            // Use separate system message for better model adherence
            let mut messages = Vec::new();
            
            if !effective_system_prompt.trim().is_empty() {
                messages.push(serde_json::json!({
                    "role": "system",
                    "content": effective_system_prompt
                }));
            }

            messages.push(serde_json::json!({
                "role": "user",
                "content": [
                    { "type": "text", "text": text_content },
                    { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{}", img) } }
                ]
            }));

            let body = serde_json::Value::Array(messages);
            
            println!("🚀 [DEBUG] OpenRouter Vision Request Body (truncated): {{ ... \"content\": [ {{ \"type\": \"text\" ... }}, {{ \"type\": \"image_url\", \"image_url\": {{ \"url\": \"data:image/png;base64,{}...\" }} }} ] }}", &img[0..20]);
            body
        } else if let Some(ctx) = &context {
            serde_json::json!([
                { "role": "system", "content": effective_system_prompt },
                { "role": "user", "content": format!("Context:\n{}\n\nInstruction:\n{}", ctx, text) }
            ])
        } else {
            serde_json::json!([
                { "role": "system", "content": effective_system_prompt },
                { "role": "user", "content": text }
            ])
        }
    };

    // 2. Try with image first (if present)
    let messages = build_body(true);

    let result = match client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        // Optional: Add site URL and name for OpenRouter rankings
        // .header("HTTP-Referer", "https://your-site.com")
        // .header("X-Title", "Vocal Typewriter")
        .json(&serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false
        }))
        .send()
        .await {
            Ok(response) => {
                if !response.status().is_success() {
                    let error_text = response.text().await.unwrap_or_default();
                    
                    // 3. Fallback: If failed and had image, retry without image
                    if image_context.is_some() {
                        eprintln!("⚠️ OpenRouter Vision request failed ({}). Retrying without image...", error_text);
                        let messages_no_img = build_body(false);
                         match client
                            .post("https://openrouter.ai/api/v1/chat/completions")
                            .header("Authorization", format!("Bearer {}", api_key))
                            .json(&serde_json::json!({
                                "model": model,
                                "messages": messages_no_img,
                                "stream": false
                            }))
                            .send()
                            .await {
                                Ok(retry_resp) => {
                                    if !retry_resp.status().is_success() {
                                        let retry_err = retry_resp.text().await.unwrap_or_default();
                                        return Err(format!("OpenRouter retry failed: {}", retry_err));
                                    }
                                    retry_resp
                                },
                                Err(e) => return Err(format!("OpenRouter retry connection failed: {}", e))
                            }
                    } else {
                        return Err(format!("OpenRouter returned error: {}", error_text));
                    }
                } else {
                    response
                }
            },
            Err(e) => return Err(format!("OpenRouter request failed: {}", e))
        };

    let body: serde_json::Value = result
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenRouter response: {}", e))?;

    println!("📥 [DEBUG] OpenRouter Response Body: {}", body);

    let content_val = &body["choices"][0]["message"]["content"];
    let result_text = if let Some(s) = content_val.as_str() {
        s.to_string()
    } else if let Some(arr) = content_val.as_array() {
        arr.iter()
           .filter_map(|item| item["text"].as_str())
           .collect::<Vec<&str>>()
           .join("")
    } else {
        // Log the actual structure for debugging
        println!("❌ [DEBUG] Invalid content structure: {:?}", content_val);
        return Err("Invalid OpenRouter response structure: 'content' is missing or invalid".to_string());
    };
    
    let result_text = result_text.trim().to_string();

    if result_text.is_empty() {
        return Err("OpenRouter returned empty content".to_string());
    }

    // Clean up using unified function
    let clean_result = clean_model_output(&result_text);

    let final_text = replace_placeholders(&clean_result, &user_name);

    Ok(final_text)
}

pub async fn download_ollama_model(app: &AppHandle, model: &str) -> Result<(), String> {
    println!("⬇️ Requesting download for model: {}", model);
    
    let client = reqwest::Client::new();
    let mut response = match client
        .post("http://localhost:11434/api/pull")
        .json(&serde_json::json!({
            "name": model,
            "stream": true
        }))
        .send()
        .await {
            Ok(res) => res,
            Err(e) => {
                println!("❌ Connection failed: {}", e);
                if e.is_connect() {
                    return Err("Ollama is not running. Please open the Ollama app or run 'ollama serve' in your terminal.".to_string());
                }
                return Err(format!("Failed to start download: {}", e));
            }
        };
        
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        println!("❌ Download failed: {} - {}", status, error_text);
        return Err(format!("Download failed: {} - {}", status, error_text));
    }

    println!("✅ Connection established. Starting stream processing...");

    // Process the stream with buffering to handle split chunks
    let mut buffer = String::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        // Extract complete lines
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].to_string();
            // Remove the processed line and the newline
            buffer.replace_range(..pos + 1, "");

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(status) = json["status"].as_str() {
                    let total = json["total"].as_u64().unwrap_or(0);
                    let completed = json["completed"].as_u64().unwrap_or(0);
                    let percentage = if total > 0 {
                        (completed as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    };
                    
                    let _ = app.emit("ollama-download-progress", serde_json::json!({
                        "model": model,
                        "status": status,
                        "percentage": percentage
                    }));
                }
            }
        }
    }
    
    // Final success emission
    let _ = app.emit("ollama-download-progress", serde_json::json!({
        "model": model,
        "status": "success",
        "percentage": 100.0
    }));
    
    Ok(())
}

pub async fn get_installed_ollama_models() -> Result<Vec<OllamaModel>, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;
        
    if !response.status().is_success() {
        return Err(format!("Ollama API error: {}", response.status()));
    }
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    
    let mut models = Vec::new();
    if let Some(models_array) = json["models"].as_array() {
        for m in models_array {
            if let (Some(name), Some(size)) = (m["name"].as_str(), m["size"].as_u64()) {
                models.push(OllamaModel {
                    name: name.to_string(),
                    size,
                });
            }
        }
    }
    
    Ok(models)
}

#[allow(dead_code)]
pub async fn is_ollama_running() -> bool {
    let client = reqwest::Client::new();
    match client.get("http://localhost:11434/api/tags").send().await {
        Ok(res) => res.status().is_success(),
        Err(_) => false,
    }
}

#[tauri::command]
pub fn start_ollama(app: tauri::AppHandle) -> Result<String, String> {
    // macOS specific: Try to open Ollama.app
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_shell::ShellExt;
        
        // First check if it's in /Applications
        let path = std::path::Path::new("/Applications/Ollama.app");
        if !path.exists() {
            return Err("Ollama.app not found in /Applications. Please install it manually.".to_string());
        }

        app.shell().open("/Applications/Ollama.app", None)
            .map_err(|e| format!("Failed to open Ollama: {}", e))?;

        Ok("Ollama starting...".to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Automatic start only supported on macOS currently.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_placeholders() {
        let user_name = Some("Loomis Green".to_string());
        
        // Test basic replacement
        assert_eq!(
            replace_placeholders("Sincerely, [Your Name]", &user_name),
            "Sincerely, Loomis Green"
        );
        
        // Test case sensitivity
        assert_eq!(
            replace_placeholders("Sincerely, [your name]", &user_name),
            "Sincerely, Loomis Green"
        );
        
        // Test other bracket types
        assert_eq!(
            replace_placeholders("Sincerely, <Name>", &user_name),
            "Sincerely, Loomis Green"
        );
        
        // Test multiple occurrences
        assert_eq!(
            replace_placeholders("[Name] said hello to [Name]", &user_name),
            "Loomis Green said hello to Loomis Green"
        );
        
        // Test empty name (should not replace)
        assert_eq!(
            replace_placeholders("[Your Name]", &Some("".to_string())),
            "[Your Name]"
        );
        
        // Test None (should not replace)
        assert_eq!(
            replace_placeholders("[Your Name]", &None),
            "[Your Name]"
        );
    }
}
