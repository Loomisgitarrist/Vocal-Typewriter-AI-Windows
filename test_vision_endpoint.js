import fs from 'fs';
import path from 'path';
import os from 'os';

// 1x1 Red Pixel PNG Base64
const BASE64_IMAGE = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

// Try to find the settings file
const settingsPath = path.join(os.homedir(), 'Library/Application Support/com.vocaltypewriter.app/settings.json');
let settings = {};

if (fs.existsSync(settingsPath)) {
    try {
        settings = JSON.parse(fs.readFileSync(settingsPath, 'utf8'));
        console.log(`📂 Loaded settings from: ${settingsPath}`);
    } catch (e) {
        console.error(`⚠️ Failed to parse settings file: ${e.message}`);
    }
} else {
    console.warn(`⚠️ Settings file not found at: ${settingsPath}`);
}

const API_KEY = process.env.OPENROUTER_API_KEY || settings.openrouter_api_key;
const MODEL = process.env.MODEL || settings.openrouter_model || "meta-llama/llama-3.2-90b-vision-instruct";

if (!API_KEY) {
    console.error("❌ Error: API Key not found in settings or environment.");
    console.error("Please set your API key in the App Settings or use OPENROUTER_API_KEY env var.");
    process.exit(1);
}

async function runTest() {
    console.log(`🚀 Testing OpenRouter Vision Endpoint`);
    console.log(`🔑 API Key: ${API_KEY.substring(0, 10)}...`);
    console.log(`🤖 Model: ${MODEL}`);

    const payload = {
        model: MODEL,
        messages: [
            {
                role: "user",
                content: [
                    { type: "text", text: "What is in this image? Reply with 'It is a red pixel'." },
                    { type: "image_url", image_url: { url: `data:image/png;base64,${BASE64_IMAGE}` } }
                ]
            }
        ],
        stream: false
    };

    console.log("\n📦 Sending Payload:");
    console.log(JSON.stringify(payload, null, 2).substring(0, 500) + " ... (truncated)");

    try {
        const response = await fetch("https://openrouter.ai/api/v1/chat/completions", {
            method: "POST",
            headers: {
                "Authorization": `Bearer ${API_KEY}`,
                "Content-Type": "application/json",
                "HTTP-Referer": "https://localhost:1420",
                "X-Title": "Vocal Typewriter Test"
            },
            body: JSON.stringify(payload)
        });

        console.log(`\nSTATUS: ${response.status} ${response.statusText}`);
        
        const rawText = await response.text();
        console.log(`\n📥 Raw Response Body:`);
        console.log(rawText);

        try {
            const data = JSON.parse(rawText);
            if (data.choices && data.choices[0] && data.choices[0].message) {
                console.log(`\n✅ Parsed Content:`, data.choices[0].message.content);
            } else {
                console.log(`\n⚠️ Unexpected JSON structure.`);
            }
        } catch (e) {
            console.log(`\n❌ Response is not valid JSON.`);
        }

    } catch (error) {
        console.error(`\n❌ Request Failed:`, error);
    }
}

runTest();
