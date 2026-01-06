# CrackingInterview - Swift to Tauri Conversion Plan

## 🎯 Project Overview

**Source:** macOS Swift app (16 files, ~1,500 lines)
**Target:** Cross-platform Tauri app (macOS + Windows + Linux)
**Timeline:** 4-6 weeks (part-time) or 2-3 weeks (full-time)
**Your Role:** Development, testing, decisions
**Claude's Role:** Code generation, debugging, guidance

---

## 📋 Master Checklist (High-Level)

### Phase 1: Setup & Foundation (Week 1)
- [ ] ✅ Project structure created
- [ ] ✅ Tauri initialized
- [ ] ✅ Development environment ready
- [ ] ✅ Basic "Hello World" working

### Phase 2: Core Backend (Week 2)
- [ ] AI service layer (Gemini + Claude APIs)
- [ ] Chrome CDP integration
- [ ] Screenshot capture (macOS + Windows)
- [ ] Prompt management

### Phase 3: Frontend UI (Week 3)
- [ ] Main app layout
- [ ] Tab selector component
- [ ] AI response display with syntax highlighting
- [ ] Settings modal

### Phase 4: Integration (Week 4)
- [ ] Connect frontend ↔ backend
- [ ] Global hotkeys
- [ ] Configuration persistence
- [ ] Error handling

### Phase 5: Polish & Testing (Week 5)
- [ ] UI refinement (match Swift design)
- [ ] Test on macOS
- [ ] Test on Windows (VM or real machine)
- [ ] Bug fixes

### Phase 6: Distribution (Week 6)
- [ ] Build installers (.dmg + .msi)
- [ ] Code signing (optional)
- [ ] Documentation
- [ ] Release!

---

## 🗺️ Detailed Step-by-Step Plan

---

## PHASE 1: Setup & Foundation (Days 1-3)

### Day 1: Project Initialization

#### Step 1.1: Initialize Tauri Project (Me)
```bash
cd /Users/nsalehvaziri/cracking-interview
npm create tauri-app@latest . -- --template vanilla
```

**What this creates:**
- `src/` - Frontend (HTML/CSS/JS)
- `src-tauri/` - Backend (Rust)
- `package.json` - Dependencies
- `tauri.conf.json` - Configuration

#### Step 1.2: Install Dependencies (You)
```bash
cd /Users/nsalehvaziri/cracking-interview
npm install
```

#### Step 1.3: Install React (Me)
```bash
npm install react react-dom
npm install --save-dev @vitejs/plugin-react
npm install react-syntax-highlighter
npm install @types/react @types/react-dom --save-dev
```

#### Step 1.4: Test Basic App (Me + You)
```bash
npm run tauri dev
```

**Expected:** Window opens with Tauri logo

**Status check:** ✅ Basic Tauri working

---

### Day 2: File Structure Setup

#### Step 2.1: Create Project Structure (Me)
```
cracking-interview/
├── src/                          # React Frontend
│   ├── App.tsx                   # Main component
│   ├── App.css                   # Global styles
│   ├── components/
│   │   ├── TabSelector.tsx       # Chrome tab picker
│   │   ├── AIResponse.tsx        # Response display
│   │   ├── SettingsModal.tsx     # Settings UI
│   │   └── ActionButtons.tsx     # Screenshot/Extract buttons
│   ├── services/
│   │   └── prompts.ts            # Prompt templates
│   └── types/
│       └── index.ts              # TypeScript types
│
└── src-tauri/                    # Rust Backend
    ├── src/
    │   ├── main.rs               # Entry point
    │   ├── chrome/
    │   │   ├── mod.rs            # Chrome CDP manager
    │   │   ├── tab_manager.rs    # Tab operations
    │   │   └── cdp.rs            # CDP protocol
    │   ├── ai/
    │   │   ├── mod.rs            # AI service coordinator
    │   │   ├── gemini.rs         # Gemini API
    │   │   └── claude.rs         # Claude API
    │   ├── screenshot/
    │   │   ├── mod.rs            # Screenshot manager
    │   │   ├── macos.rs          # macOS capture
    │   │   └── windows.rs        # Windows capture
    │   └── config.rs             # Configuration manager
    │
    ├── Cargo.toml                # Rust dependencies
    └── tauri.conf.json           # App configuration
```

**Your task:** Review structure, suggest changes

---

### Day 3: Core Dependencies & Chrome CDP Test

#### Step 3.1: Add Rust Dependencies (Me)
Edit `src-tauri/Cargo.toml`:
```toml
[dependencies]
tauri = { version = "2", features = ["macos-private-api"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
base64 = "0.22"
tokio-tungstenite = "0.21"
futures-util = "0.3"

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.52", features = ["Win32_Graphics_Gdi"] }
screenshots = "0.7"

[target.'cfg(target_os = "macos")'.dependencies]
screenshots = "0.7"
```

#### Step 3.2: Test Chrome CDP Connection (Me)
Create simple test to verify Chrome CDP works:
```rust
// src-tauri/src/main.rs
#[tauri::command]
async fn test_chrome_cdp() -> Result<String, String> {
    let response = reqwest::get("http://localhost:9222/json/version")
        .await
        .map_err(|e| e.to_string())?;
    
    let text = response.text().await.map_err(|e| e.to_string())?;
    Ok(text)
}
```

**Your task:** Launch Chrome with `--remote-debugging-port=9222` and test

---

## PHASE 2: Core Backend (Days 4-10)

### Day 4-5: AI Service Layer

#### Your Swift Files to Convert:
- `UnifiedAIService.swift` (80 lines)
- `GeminiServiceNew.swift` (70 lines)  
- `ClaudeService.swift` (100 lines)

#### Conversion Strategy:

**Step 4.1: I'll Convert GeminiServiceNew.swift**

I'll create `src-tauri/src/ai/gemini.rs`:
```rust
use base64::{Engine as _, engine::general_purpose};
use serde_json::json;

pub async fn query_with_text(
    prompt: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    // Direct port of your Swift code
    let endpoint = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );
    
    let payload = json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }]
    });
    
    let response = reqwest::Client::new()
        .post(&endpoint)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))?;
    
    // Extract response (same logic as Swift)
    if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        Ok(text.to_string())
    } else if let Some(error) = json["error"]["message"].as_str() {
        Err(format!("API Error: {}", error))
    } else {
        Err("No response from Gemini".to_string())
    }
}

pub async fn query_with_image(
    prompt: &str,
    image_data: &[u8],
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    let base64_image = general_purpose::STANDARD.encode(image_data);
    
    let endpoint = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );
    
    let payload = json!({
        "contents": [{
            "parts": [
                { "text": prompt },
                {
                    "inline_data": {
                        "mime_type": "image/png",
                        "data": base64_image
                    }
                }
            ]
        }]
    });
    
    let response = reqwest::Client::new()
        .post(&endpoint)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))?;
    
    if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        Ok(text.to_string())
    } else {
        Err("No response from Gemini".to_string())
    }
}
```

**Your task:** Review and test with your API key

**Step 4.2: I'll Convert ClaudeService.swift**

Similar process for Claude API.

**Step 4.3: I'll Create Unified Service**

Port `UnifiedAIService.swift` logic to coordinate between services.

**Deliverable:** Working AI service layer (text + image queries)

---

### Day 6-7: Chrome CDP Integration

#### Your Swift File to Replace:
- `ChromeTabManager.swift` (180 lines of AppleScript)

#### New Approach (MUCH BETTER!):

**Step 6.1: I'll Create Chrome CDP Module**

`src-tauri/src/chrome/cdp.rs`:
```rust
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromeTab {
    pub id: String,
    pub url: String,
    pub title: String,
    pub tab_type: String,
}

/// Get all Chrome tabs using CDP
pub async fn get_all_tabs() -> Result<Vec<ChromeTab>, String> {
    let response = reqwest::get("http://localhost:9222/json/list")
        .await
        .map_err(|e| format!("CDP connection failed: {}. Make sure Chrome is running with --remote-debugging-port=9222", e))?;
    
    let tabs: Vec<ChromeTab> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse tabs: {}", e))?;
    
    // Filter to only page tabs (not extensions, etc.)
    Ok(tabs.into_iter()
        .filter(|t| t.tab_type == "page")
        .collect())
}

/// Activate a specific tab
pub async fn activate_tab(tab_id: &str) -> Result<(), String> {
    let url = format!("http://localhost:9222/json/activate/{}", tab_id);
    reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to activate tab: {}", e))?;
    
    // Bring Chrome to front
    #[cfg(target_os = "macos")]
    bring_chrome_to_front_macos()?;
    
    #[cfg(target_os = "windows")]
    bring_chrome_to_front_windows()?;
    
    Ok(())
}

/// Execute JavaScript in a tab via CDP WebSocket
pub async fn execute_javascript(tab_id: &str, script: &str) -> Result<String, String> {
    // Get tab's WebSocket URL
    let tabs: Vec<serde_json::Value> = reqwest::get("http://localhost:9222/json/list")
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    
    let tab = tabs.iter()
        .find(|t| t["id"].as_str() == Some(tab_id))
        .ok_or("Tab not found")?;
    
    let ws_url = tab["webSocketDebuggerUrl"]
        .as_str()
        .ok_or("No WebSocket URL")?;
    
    // Connect via WebSocket
    let (ws_stream, _) = connect_async(ws_url)
        .await
        .map_err(|e| format!("WebSocket failed: {}", e))?;
    
    let (mut write, mut read) = ws_stream.split();
    
    // Send Runtime.evaluate command
    let command = serde_json::json!({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": {
            "expression": script,
            "returnByValue": true
        }
    });
    
    write.send(Message::Text(command.to_string()))
        .await
        .map_err(|e| format!("Send failed: {}", e))?;
    
    // Read response
    if let Some(msg) = read.next().await {
        let text = msg.map_err(|e| e.to_string())?.to_text().map_err(|e| e.to_string())?;
        let response: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| e.to_string())?;
        
        if let Some(result) = response["result"]["result"]["value"].as_str() {
            return Ok(result.to_string());
        }
    }
    
    Err("No result from JavaScript execution".to_string())
}

#[cfg(target_os = "macos")]
fn bring_chrome_to_front_macos() -> Result<(), String> {
    std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"Google Chrome\" to activate")
        .output()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn bring_chrome_to_front_windows() -> Result<(), String> {
    // Windows-specific code to bring Chrome to front
    Ok(())
}
```

**Features this enables (that AppleScript CAN'T do):**
- ✅ Execute ANY JavaScript
- ✅ Extract specific DOM elements
- ✅ Monitor network requests
- ✅ Detect which coding platform (LeetCode vs HackerRank)
- ✅ Auto-scroll to problem area
- ✅ Get page performance metrics

**Your task:** Test with your Chrome tabs

---

### Day 8-9: Screenshot Capture

#### Your Swift File to Replace:
- `ChromeTabTextExtractor.swift` (ScreenCaptureKit)

#### New Approach:

**Step 8.1: I'll Create Platform-Specific Screenshot**

`src-tauri/src/screenshot/macos.rs`:
```rust
use std::process::Command;
use std::path::PathBuf;

pub fn capture_chrome_window() -> Result<PathBuf, String> {
    let output_path = std::env::temp_dir().join("cracking-screenshot.png");
    
    // Get Chrome window ID
    let output = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"Google Chrome\" to get id of front window")
        .output()
        .map_err(|e| format!("Failed to get window ID: {}", e))?;
    
    let window_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    
    // Capture using screencapture
    let result = Command::new("screencapture")
        .arg("-l")
        .arg(&window_id)
        .arg("-o")  // No shadow
        .arg(&output_path)
        .output()
        .map_err(|e| format!("screencapture failed: {}", e))?;
    
    if !result.status.success() {
        return Err("Screenshot failed".to_string());
    }
    
    Ok(output_path)
}
```

`src-tauri/src/screenshot/windows.rs`:
```rust
// Windows implementation using Win32 APIs
// I'll create this for you
```

---

### Day 10: Prompt Management

#### Your Swift File to Port:
- `PromptManager.swift` (150 lines)

#### New Approach:

**Step 10.1: I'll Create TypeScript Version**

`src/services/prompts.ts`:
```typescript
export enum PromptTemplate {
    AlgorithmOptimal = 'algorithm_optimal',
    AlgorithmBeginner = 'algorithm_beginner',
    SystemDesign = 'system_design',
    CodeReview = 'code_review',
    ExplainConcept = 'explain_concept',
}

export enum ProgrammingLanguage {
    Java = 'Java',
    Python = 'Python',
    JavaScript = 'JavaScript',
    Cpp = 'C++',
    Swift = 'Swift',
}

export function buildPrompt(
    template: PromptTemplate,
    language: ProgrammingLanguage,
    content?: string
): string {
    const systemPrompt = `You are an expert technical assistant...
    
RESPONSE FORMAT:
Always structure your response with these markers:

EXPLANATION_START
[Provide your explanation here]
EXPLANATION_END

SOLUTION_START
[Provide the solution code without markdown blocks]
SOLUTION_END`;

    let userPrompt = '';
    
    switch (template) {
        case PromptTemplate.AlgorithmOptimal:
            userPrompt = `Solve this ${language} algorithm problem.

Requirements:
- Provide optimal time/space complexity solution
- Include complexity analysis (O notation)
- Write production-ready, clean code
- Explain your approach briefly`;
            break;
            
        case PromptTemplate.AlgorithmBeginner:
            userPrompt = `Explain and solve this ${language} algorithm problem for a beginner...`;
            break;
            
        // ... other templates
    }
    
    let finalPrompt = `${systemPrompt}\n\n${userPrompt}`;
    
    if (content) {
        finalPrompt += `\n\nQuestion:\n${content}`;
    }
    
    return finalPrompt;
}
```

**This is EASIER than Swift** - just data structures!

---

## PHASE 3: Frontend UI (Days 11-17)

### Day 11-12: Main App Layout

#### Your Swift File to Port:
- `ContentView.swift` (280 lines)

#### Step 11.1: I'll Create React Main Component

`src/App.tsx`:
```typescript
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import TabSelector from './components/TabSelector';
import AIResponse from './components/AIResponse';
import SettingsModal from './components/SettingsModal';
import ActionButtons from './components/ActionButtons';
import './App.css';

interface ChromeTab {
    id: string;
    url: string;
    title: string;
    tab_type: string;
}

interface AIConfig {
    selected_model: string;
    gemini_api_key: string;
    claude_api_key: string;
}

function App() {
    const [selectedTab, setSelectedTab] = useState<ChromeTab | null>(null);
    const [aiResponse, setAiResponse] = useState('');
    const [isProcessing, setIsProcessing] = useState(false);
    const [showTabList, setShowTabList] = useState(false);
    const [showSettings, setShowSettings] = useState(false);
    
    const [aiConfig, setAiConfig] = useState<AIConfig>({
        selected_model: localStorage.getItem('ai_model') || 'gemini-2.0-flash',
        gemini_api_key: localStorage.getItem('gemini_key') || '',
        claude_api_key: localStorage.getItem('claude_key') || '',
    });

    // Register hotkeys
    useEffect(() => {
        invoke('register_hotkeys');
        
        const unlistenScreenshot = listen('hotkey-screenshot', () => {
            handleCaptureScreenshot();
        });
        
        const unlistenExtract = listen('hotkey-extract', () => {
            handleExtractText();
        });
        
        return () => {
            unlistenScreenshot.then(fn => fn());
            unlistenExtract.then(fn => fn());
        };
    }, [selectedTab, aiConfig]);

    const handleCaptureScreenshot = async () => {
        if (!selectedTab) {
            setAiResponse('❌ Please select a Chrome tab first');
            return;
        }

        setIsProcessing(true);
        
        try {
            // Activate and capture
            await invoke('activate_chrome_tab', { tabId: selectedTab.id });
            await new Promise(resolve => setTimeout(resolve, 500)); // Wait for activation
            
            const imagePath = await invoke<string>('capture_screenshot');
            
            // Build prompt
            const prompt = buildPrompt(
                localStorage.getItem('template') || 'algorithm_optimal',
                localStorage.getItem('language') || 'Java'
            );
            
            // Query AI with image
            const response = await invoke<string>('query_ai_with_image', {
                prompt,
                imagePath,
                config: aiConfig,
            });
            
            setAiResponse(response);
        } catch (error) {
            setAiResponse(`❌ Error: ${error}`);
        } finally {
            setIsProcessing(false);
        }
    };

    const handleExtractText = async () => {
        if (!selectedTab) {
            setAiResponse('❌ Please select a Chrome tab first');
            return;
        }

        setIsProcessing(true);
        
        try {
            await invoke('activate_chrome_tab', { tabId: selectedTab.id });
            await new Promise(resolve => setTimeout(resolve, 300));
            
            const text = await invoke<string>('extract_tab_text', {
                tabId: selectedTab.id
            });
            
            const prompt = buildPrompt(
                localStorage.getItem('template') || 'algorithm_optimal',
                localStorage.getItem('language') || 'Java',
                text
            );
            
            const response = await invoke<string>('query_ai', {
                prompt,
                config: aiConfig,
            });
            
            setAiResponse(response);
        } catch (error) {
            setAiResponse(`❌ Error: ${error}`);
        } finally {
            setIsProcessing(false);
        }
    };

    return (
        <div className="app-container">
            <header className="app-header">
                <div className="header-left">
                    <span className="app-icon">💻</span>
                    <h1>CrackingInterview</h1>
                </div>
                <div className="header-right">
                    <button onClick={() => setShowSettings(true)}>
                        ⚙️ Settings
                    </button>
                </div>
            </header>

            <TabSelector
                selectedTab={selectedTab}
                onShowList={() => setShowTabList(true)}
            />

            <ActionButtons
                isProcessing={isProcessing}
                onScreenshot={handleCaptureScreenshot}
                onExtract={handleExtractText}
            />

            <AIResponse response={aiResponse} isProcessing={isProcessing} />

            {showTabList && (
                <TabSelectorModal
                    onSelectTab={(tab) => {
                        setSelectedTab(tab);
                        setShowTabList(false);
                    }}
                    onClose={() => setShowTabList(false)}
                />
            )}

            {showSettings && (
                <SettingsModal
                    config={aiConfig}
                    onSave={(newConfig) => {
                        setAiConfig(newConfig);
                        localStorage.setItem('ai_model', newConfig.selected_model);
                        localStorage.setItem('gemini_key', newConfig.gemini_api_key);
                        localStorage.setItem('claude_key', newConfig.claude_api_key);
                        setShowSettings(false);
                    }}
                    onClose={() => setShowSettings(false)}
                />
            )}
        </div>
    );
}

export default App;
```

**Your task:** Review flow, suggest UI improvements

---

### Day 13-14: Tab Selector Component

#### Your Swift File to Port:
- `ChromeTabListViewSimple.swift` (150 lines)

#### Step 13.1: I'll Create React Component

`src/components/TabSelector.tsx`:
```typescript
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface ChromeTab {
    id: string;
    url: string;
    title: string;
}

interface Props {
    onSelectTab: (tab: ChromeTab) => void;
    onClose: () => void;
}

export default function TabSelectorModal({ onSelectTab, onClose }: Props) {
    const [tabs, setTabs] = useState<ChromeTab[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [error, setError] = useState('');

    useEffect(() => {
        fetchTabs();
    }, []);

    const fetchTabs = async () => {
        setIsLoading(true);
        try {
            const chromeTabs = await invoke<ChromeTab[]>('get_chrome_tabs');
            setTabs(chromeTabs);
        } catch (err) {
            setError(String(err));
        } finally {
            setIsLoading(false);
        }
    };

    return (
        <div className="modal-overlay" onClick={onClose}>
            <div className="modal-content" onClick={(e) => e.stopPropagation()}>
                <div className="modal-header">
                    <h2>Chrome Tab Selector</h2>
                    <div className="header-actions">
                        <button onClick={fetchTabs}>🔄 Refresh</button>
                        <button onClick={onClose}>✕</button>
                    </div>
                </div>

                <div className="modal-body">
                    {isLoading ? (
                        <div className="loading">
                            <div className="spinner"></div>
                            <p>Loading tabs...</p>
                        </div>
                    ) : error ? (
                        <div className="error">
                            <p>⚠️ {error}</p>
                            <p>Make sure Chrome is running with:</p>
                            <code>--remote-debugging-port=9222</code>
                        </div>
                    ) : tabs.length === 0 ? (
                        <div className="empty">
                            <p>No Chrome tabs found</p>
                        </div>
                    ) : (
                        <div className="tab-list">
                            {tabs.map((tab) => (
                                <button
                                    key={tab.id}
                                    className="tab-item"
                                    onClick={() => onSelectTab(tab)}
                                >
                                    <span className="tab-icon">🌐</span>
                                    <div className="tab-details">
                                        <div className="tab-title">{tab.title}</div>
                                        <div className="tab-url">{tab.url}</div>
                                    </div>
                                    <span className="chevron">›</span>
                                </button>
                            ))}
                        </div>
                    )}
                </div>

                {tabs.length > 0 && (
                    <div className="modal-footer">
                        <span>{tabs.length} tab(s) found</span>
                    </div>
                )}
            </div>
        </div>
    );
}
```

**Your task:** Test, provide feedback on UX

---

### Day 15-16: AI Response Display

#### Your Swift File to Port:
- `UIComponents.swift` (400 lines - mostly SyntaxHighlightedCodeView)

#### Step 15.1: I'll Create Response Component

**The AMAZING part** - 400 lines becomes ~50 lines!

`src/components/AIResponse.tsx`:
```typescript
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

interface Props {
    response: string;
    isProcessing: boolean;
}

function parseResponse(response: string) {
    // Same logic as your Swift parseGeminiResponse()
    let explanation = '';
    let code = '';

    const explanationMatch = response.match(/EXPLANATION_START\s*([\s\S]*?)\s*EXPLANATION_END/);
    const solutionMatch = response.match(/SOLUTION_START\s*([\s\S]*?)\s*SOLUTION_END/);

    if (explanationMatch) explanation = explanationMatch[1].trim();
    if (solutionMatch) code = solutionMatch[1].trim();

    // Fallback to markdown code blocks
    if (!code) {
        const codeBlockMatch = response.match(/```(?:java)?\s*([\s\S]*?)```/);
        if (codeBlockMatch) {
            code = codeBlockMatch[1].trim();
            explanation = response.substring(0, codeBlockMatch.index || 0).trim();
        }
    }

    return { explanation, code };
}

export default function AIResponse({ response, isProcessing }: Props) {
    if (isProcessing) {
        return (
            <div className="loading-container">
                <div className="spinner"></div>
                <p>Processing with AI...</p>
            </div>
        );
    }

    if (!response) {
        return (
            <div className="empty-state">
                <p>Select a Chrome tab and click a button above to get started!</p>
            </div>
        );
    }

    const { explanation, code } = parseResponse(response);

    return (
        <div className="ai-response">
            {explanation && (
                <div className="explanation-section">
                    <div className="section-header">
                        <span className="icon">📄</span>
                        <h3>Explanation</h3>
                    </div>
                    <div className="explanation-content">
                        {explanation}
                    </div>
                </div>
            )}

            {code && (
                <div className="code-section">
                    <div className="section-header">
                        <span className="icon">⚡</span>
                        <h3>Solution</h3>
                        <button 
                            className="copy-btn"
                            onClick={() => navigator.clipboard.writeText(code)}
                        >
                            📋 Copy
                        </button>
                    </div>
                    
                    <SyntaxHighlighter
                        language={localStorage.getItem('language')?.toLowerCase() || 'java'}
                        style={vscDarkPlus}
                        showLineNumbers={true}
                        customStyle={{
                            margin: 0,
                            borderRadius: '8px',
                            fontSize: '13px',
                        }}
                    >
                        {code}
                    </SyntaxHighlighter>
                </div>
            )}
        </div>
    );
}
```

**Benefits:**
- Your 400 lines → 50 lines!
- Syntax highlighting library does all the work
- Supports 200+ languages (you have 5)
- Better highlighting than custom Swift code

---

### Day 17: Settings Modal

#### Your Swift File to Port:
- `AISettingsView.swift` (200 lines)

#### Step 17.1: I'll Create Settings Component

Similar to what I showed you earlier - dropdown for AI models, API key inputs, template selection.

---

## PHASE 4: Integration (Days 18-24)

### Day 18-19: Wire Everything Together

#### Step 18.1: I'll Create Tauri Commands in main.rs

`src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chrome;
mod ai;
mod screenshot;
mod config;

use tauri::Manager;

#[tauri::command]
async fn get_chrome_tabs() -> Result<Vec<chrome::ChromeTab>, String> {
    chrome::get_all_tabs().await
}

#[tauri::command]
async fn activate_chrome_tab(tab_id: String) -> Result<(), String> {
    chrome::activate_tab(&tab_id).await
}

#[tauri::command]
async fn extract_tab_text(tab_id: String) -> Result<String, String> {
    chrome::execute_javascript(&tab_id, "document.body.innerText").await
}

#[tauri::command]
async fn capture_screenshot() -> Result<String, String> {
    let path = screenshot::capture_chrome_window()?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
async fn query_ai(
    prompt: String,
    config: serde_json::Value,
) -> Result<String, String> {
    let model = config["selected_model"].as_str().unwrap();
    let gemini_key = config["gemini_api_key"].as_str().unwrap();
    let claude_key = config["claude_api_key"].as_str().unwrap();
    
    if model.contains("gemini") {
        ai::gemini::query_with_text(&prompt, gemini_key, model).await
    } else {
        ai::claude::query_with_text(&prompt, claude_key, model).await
    }
}

#[tauri::command]
async fn query_ai_with_image(
    prompt: String,
    image_path: String,
    config: serde_json::Value,
) -> Result<String, String> {
    let image_data = std::fs::read(&image_path)
        .map_err(|e| format!("Failed to read image: {}", e))?;
    
    let model = config["selected_model"].as_str().unwrap();
    let gemini_key = config["gemini_api_key"].as_str().unwrap();
    let claude_key = config["claude_api_key"].as_str().unwrap();
    
    if model.contains("gemini") {
        ai::gemini::query_with_image(&prompt, &image_data, gemini_key, model).await
    } else {
        ai::claude::query_with_image(&prompt, &image_data, claude_key, model).await
    }
}

#[tauri::command]
fn register_hotkeys(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
    
    let screenshot_key = if cfg!(target_os = "macos") { "Cmd+Shift+G" } else { "Ctrl+Shift+G" };
    let extract_key = if cfg!(target_os = "macos") { "Cmd+Shift+E" } else { "Ctrl+Shift+E" };
    
    app.global_shortcut().on_shortcut(screenshot_key, |app, _shortcut, _event| {
        app.emit("hotkey-screenshot", ()).ok();
    }).map_err(|e| e.to_string())?;
    
    app.global_shortcut().on_shortcut(extract_key, |app, _shortcut, _event| {
        app.emit("hotkey-extract", ()).ok();
    }).map_err(|e| e.to_string())?;
    
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_chrome_tabs,
            activate_chrome_tab,
            extract_tab_text,
            capture_screenshot,
            query_ai,
            query_ai_with_image,
            register_hotkeys,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Your task:** Test each command individually

---

### Day 20-22: Styling (Match Your Swift App)

#### Your Swift File to Port:
- `AppTheme.swift` (100 lines)

#### Step 20.1: I'll Create CSS Theme

`src/App.css`:
```css
:root {
    /* Your exact colors from AppTheme.swift */
    --primary-blue: rgb(51, 128, 230);
    --primary-dark: rgb(38, 38, 51);
    --accent-green: rgb(77, 204, 128);
    --background-light: rgb(242, 245, 247);
    --card-background: rgb(250, 250, 252);
    --text-primary: rgb(26, 26, 26);
    --text-secondary: rgb(102, 102, 115);
    
    /* Code editor colors */
    --code-bg: rgb(41, 43, 54);
    --code-text: rgb(242, 242, 247);
}

/* ... rest of styling to match your Swift app exactly */
```

**Your task:** Visual QA - does it match your Swift app?

---

### Day 23-24: Error Handling & Edge Cases

- Handle network errors gracefully
- Chrome not running detection
- API key validation
- Empty response handling

---

## PHASE 5: Testing (Days 25-31)

### Day 25-27: macOS Testing
- [ ] All features work on Mac
- [ ] Hotkeys work
- [ ] Screenshot capture works
- [ ] Text extraction works
- [ ] AI responses display correctly
- [ ] Settings persist

### Day 28-29: Windows Testing
- [ ] Set up Windows VM or machine
- [ ] Test all features on Windows
- [ ] Fix Windows-specific issues
- [ ] Verify Chrome CDP works

### Day 30-31: Cross-Platform QA
- [ ] Test switching between tabs
- [ ] Test different coding platforms (LeetCode, HackerRank)
- [ ] Test all AI models
- [ ] Test all prompt templates
- [ ] Performance testing

---

## PHASE 6: Distribution (Days 32-38)

### Day 32-33: Build Process
- [ ] Configure app icons properly
- [ ] Set up code signing (optional)
- [ ] Build macOS .dmg
- [ ] Build Windows .msi

### Day 34-35: Documentation
- [ ] User guide
- [ ] Installation instructions
- [ ] Troubleshooting guide
- [ ] Update crackinginterview.org website

### Day 36-38: Release
- [ ] Upload to website
- [ ] Create download links
- [ ] Announce to users
- [ ] Monitor feedback

---

## 📅 Timeline Summary

**Full-Time (40 hours/week):**
- Week 1: Setup + Backend
- Week 2: Frontend + Integration
- Week 3: Testing + Polish
- Week 4: Distribution
- **Total: 4 weeks**

**Part-Time (10-15 hours/week):**
- Weeks 1-2: Setup + Backend
- Weeks 3-4: Frontend + Integration
- Week 5: Testing
- Week 6: Distribution
- **Total: 6 weeks**

---

## 🎯 Your Immediate Next Steps

### This Week:
1. ✅ Review this plan - any concerns?
2. ✅ Commit to timeline (full-time or part-time?)
3. ✅ I'll initialize the Tauri project
4. ✅ We start with Chrome CDP (most critical part)

### Next Week:
1. Port AI services
2. Test basic flow (tab → extract → AI → display)
3. Validate approach works

**Decision point after Week 2:** Continue or abort?

---

## 🤝 Division of Labor

### What I'll Do:
- ✅ Initialize project structure
- ✅ Convert Swift code to Rust (using AI assistance)
- ✅ Create React components
- ✅ Set up Chrome CDP integration
- ✅ Debug and fix errors
- ✅ Create all boilerplate code

### What You'll Do:
- ✅ Review and test each phase
- ✅ Provide UI/UX feedback
- ✅ Test on your Mac
- ✅ Make design decisions
- ✅ Provide API keys for testing
- ✅ Final QA and approval

---

## 📊 Risk Mitigation

### What Could Go Wrong:

**Risk 1: Chrome CDP doesn't work well**
- Mitigation: Test in Week 1, pivot if needed
- Fallback: Use Electron with AppleScript

**Risk 2: Learning curve too steep**
- Mitigation: I handle all Rust code
- Fallback: Hire Tauri developer

**Risk 3: UI doesn't match Swift quality**
- Mitigation: Iterative design reviews
- Fallback: Spend extra week on polish

**Risk 4: Windows has unexpected issues**
- Mitigation: Test early and often
- Fallback: macOS-first release, Windows later

---

## ✅ Success Criteria

After 4-6 weeks, you'll have:

- ✅ Cross-platform app (macOS + Windows)
- ✅ Professional installers (.dmg + .msi)
- ✅ Chrome CDP automation (better than AppleScript!)
- ✅ File size: 5-8 MB (comparable to Swift)
- ✅ All current features working
- ✅ Ready to distribute on crackinginterview.org

---

## 🚀 Ready to Start?

**Next step:** I'll initialize the Tauri project structure in your folder!

Say "Let's start" and I'll begin Phase 1, Step 1!
