import path from 'path';
import fs from 'fs';
const os = require('os');

// 1x1 Red Pixel PNG Base64 (without prefix, for Ollama)
const RAW_BASE64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
// With prefix (for OpenAI/OpenRouter)
const DATA_URL = `data:image/png;base64,${RAW_BASE64}`;

// Try to find the settings file
const settingsPath = path.join(os.homedir(), 'Library/Application Support/com.vocaltypewriter.app/settings.json');
let settings = {};

if (fs.existsSync(settingsPath)) {
    try {
        settings = JSON.parse(fs.readFileSync(settingsPath, 'utf8'));
        console.log(`📂 Loaded settings from: ${settingsPath}`);
    } catch (e) {
        console.error(`⚠️ Failed to parse settings file: ${e.message}`);
        process.exit(1);
    }
} else {
    console.error(`❌ Settings file not found at: ${settingsPath}`);
    process.exit(1);
}

// Configuration
const CONFIG = {
    openai: {
        enabled: !!settings.openai_api_key,
        key: settings.openai_api_key,
        model: settings.openai_model || "gpt-4o",
        url: "https://api.openai.com/v1/chat/completions"
    },
    openrouter: {
        enabled: !!settings.openrouter_api_key,
        key: settings.openrouter_api_key,
        model: settings.openrouter_model || "meta-llama/llama-3.2-90b-vision-instruct",
        url: "https://openrouter.ai/api/v1/chat/completions"
    },
    ollama: {
        enabled: true,
        model: settings.ollama_vision_model || "llava", // Use vision model for screenshot test
        url: "http://localhost:11434/api/generate"
    }
};

async function testOpenAI() {
    if (!CONFIG.openai.enabled) {
        console.log("⏩ Skipping OpenAI (no key)");
        return;
    }
    console.log(`\n🚀 Testing OpenAI Vision (${CONFIG.openai.model})...`);
    
    const payload = {
        model: CONFIG.openai.model,
        messages: [
            { role: "system", content: "You are a helpful assistant." },
            {
                role: "user",
                content: [
                    { type: "text", text: "What color is this pixel? Answer in 1 word." },
                    { type: "image_url", image_url: { url: DATA_URL } }
                ]
            }
        ]
    };

    try {
        const res = await fetch(CONFIG.openai.url, {
            method: "POST",
            headers: {
                "Authorization": `Bearer ${CONFIG.openai.key}`,
                "Content-Type": "application/json"
            },
            body: JSON.stringify(payload)
        });
        
        const data = await res.json();
        if (res.ok) {
            console.log("✅ OpenAI Success:", data.choices[0].message.content);
        } else {
            console.error("❌ OpenAI Failed:", JSON.stringify(data, null, 2));
        }
    } catch (e) {
        console.error("❌ OpenAI Error:", e.message);
    }
}

async function testOpenRouter() {
    if (!CONFIG.openrouter.enabled) {
        console.log("⏩ Skipping OpenRouter (no key)");
        return;
    }
    console.log(`\n🚀 Testing OpenRouter Vision (${CONFIG.openrouter.model})...`);
    
    const payload = {
        model: CONFIG.openrouter.model,
        messages: [
            { role: "system", content: "You are a helpful assistant." },
            {
                role: "user",
                content: [
                    { type: "text", text: "What color is this pixel? Answer in 1 word." },
                    { type: "image_url", image_url: { url: DATA_URL } }
                ]
            }
        ],
        stream: false
    };

    try {
        const res = await fetch(CONFIG.openrouter.url, {
            method: "POST",
            headers: {
                "Authorization": `Bearer ${CONFIG.openrouter.key}`,
                "Content-Type": "application/json",
                "HTTP-Referer": "https://localhost:1420",
                "X-Title": "Vocal Typewriter Test"
            },
            body: JSON.stringify(payload)
        });
        
        const data = await res.json();
        if (res.ok) {
            console.log("✅ OpenRouter Success:", data.choices?.[0]?.message?.content || data);
        } else {
            console.error("❌ OpenRouter Failed:", JSON.stringify(data, null, 2));
        }
    } catch (e) {
        console.error("❌ OpenRouter Error:", e.message);
    }
}

async function testOllama() {
    console.log(`\n🚀 Testing Ollama Vision (${CONFIG.ollama.model})...`);
    
    // Ollama generate API expects raw base64 in "images" array
    const payload = {
        model: CONFIG.ollama.model,
        prompt: "What color is this pixel? Answer in 1 word.",
        stream: false,
        images: [RAW_BASE64]
    };

    try {
        const res = await fetch(CONFIG.ollama.url, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(payload)
        });
        
        // Handle connection refused (Ollama not running)
        if (!res.ok) {
            console.error(`❌ Ollama Failed: ${res.status} ${res.statusText}`);
            return;
        }

        const data = await res.json();
        console.log("✅ Ollama Success:", data.response);

    } catch (e) {
        if (e.cause && e.cause.code === 'ECONNREFUSED') {
             console.log("⚠️ Ollama is not running (ECONNREFUSED). Please start it with 'ollama serve'.");
        } else {
             console.error("❌ Ollama Error:", e.message);
        }
    }
}

async function runAll() {
    console.log("--- STARTING PROVIDER TESTS ---");
    await testOpenAI();
    await testOpenRouter();
    await testOllama();
    console.log("\n--- TESTS COMPLETED ---");
}

runAll();
