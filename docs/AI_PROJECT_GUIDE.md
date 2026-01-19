# CrackingInterview — AI Project Guide (for Coding Agents)

This document is written for an AI coding agent (and future maintainers) to quickly understand **what this repo is**, **how data flows**, and **where to make changes**.

## What this app does (current behavior)

CrackingInterview is a **Tauri (Rust) + React** desktop app that helps a software engineer solve algorithm questions in sites like LeetCode/HackerRank by:

- **Listing Chrome tabs** (via **Chrome DevTools Protocol** / CDP) and letting the user choose an input source
- **Extracting content** from the selected source either as:
  - **Text** (execute JS in the tab to read `document.body.innerText`), or
  - **Screenshot** (CDP tab screenshot or OS display capture)
- Sending that content into an **LLM** (Gemini or Claude)
- Showing the **AI response** inside the app UI

There is also an explicit goal to support a “LeetCode Wizard”-style workflow (see `https://leetcodewizard.io/`) where the user can trigger “Solve” using a **global hotkey** without leaving Chrome.

## Repo structure (ignore generated dirs)

Do **not** try to “read everything” including generated artifacts. In this repo, these dirs are generated/large:

- `node_modules/`
- `dist/` (frontend build output)
- `src-tauri/target/` (Rust build output)

The primary source code is:

- **Frontend**: `src/`
  - `src/main.tsx`: React entry point (mounts `App`)
  - `src/App.tsx`: main UI + orchestration (CDP status, source list, solve flow, settings modal, hotkey listener)
  - `src/App.css`: global app styles
  - `src/services/prompts.ts`: prompt templates + `buildPrompt(...)`
  - `src/hooks/useWindowSizeManager.ts`: optional hook for resizing window when settings open/close
  - `src/components/TabDropdown.tsx`: custom dropdown for **InputSource** with thumbnails
  - `src/components/TabDropdown.css`: styles for `TabDropdown.tsx`
  - `src/components/AIResponseDisplay.tsx`: renders AI response, parses common markers/blocks, syntax-highlights code
  - `src/components/PromptEditor.tsx` + `.css`: UI for editing prompt templates
  - `src/components/PromptListView.tsx` + `.css`: UI for selecting templates and entering edit mode
- **Backend (Tauri/Rust)**: `src-tauri/src/`
  - `main.rs`: Tauri commands (IPC) + global hotkey registration
  - `chrome/*`: Chrome CDP integration (tabs, activate, execute JS, screenshots)
  - `ai/*`: Gemini + Claude HTTP clients + provider routing
  - `screenshot.rs`: OS display capture (screenshots crate), thumbnails
  - `oauth_server.rs` + `google_oauth.rs`: OAuth flow for Gemini access tokens

### “Is this file used?” — how to verify quickly

In this repo, most runtime wiring is via **imports**:

- A TS/TSX file is “used” if it is imported (directly or transitively) from `src/main.tsx`.
- A CSS file is “used” if it is imported by some TS/TSX module that is imported from `src/main.tsx`.

Example: `TabDropdown.tsx` is imported by `src/App.tsx`, and `TabDropdown.css` is imported by `TabDropdown.tsx`, so both are in the runtime bundle.

## Frontend file map (what calls what)

Entry:

- `src/main.tsx` → renders `<App />` from `src/App.tsx`

App composition (current):

- `src/App.tsx`
  - imports `TabDropdown` for Input Source selection
  - imports `AIResponseDisplay` to render the final LLM output
  - imports `PromptEditor` / `PromptListView` for prompt-template UX
  - calls Tauri commands using `invoke(...)` for CDP, screenshots, AI, OAuth
  - listens for backend-emitted hotkey event `hotkey-solve` (Tauri event) and triggers solve

Component responsibilities:

- `src/components/TabDropdown.tsx`
  - renders a dropdown list of `InputSource` entries
  - supports thumbnail previews (`thumbnail` is expected to be a base64 data URL)
  - imports its own styles from `src/components/TabDropdown.css`
- `src/components/AIResponseDisplay.tsx`
  - tries to parse response using markers (EXPLANATION_START/END, SOLUTION_START/END)
  - falls back to markdown code fences or heuristics (`class Solution`)
  - uses `react-syntax-highlighter` for code formatting

## Backend file map (Rust/Tauri)

- `src-tauri/src/main.rs`
  - defines all Tauri commands exposed to the frontend (`invoke_handler`)
  - registers global hotkey and emits `hotkey-solve` (frontend listens)
- `src-tauri/src/chrome/mod.rs`
  - CDP tab listing (`GET http://localhost:9222/json/list`)
  - activate tab (`/json/activate/<id>`)
  - execute JS via per-tab WebSocket (`Runtime.evaluate`)
  - screenshots / thumbnails via CDP (`Page.captureScreenshot`)
- `src-tauri/src/chrome/launcher.rs`
  - launches a dedicated Chrome instance with `--remote-debugging-port=9222`
  - tries to avoid killing user’s primary Chrome session
- `src-tauri/src/ai/mod.rs`
  - routes to Gemini vs Claude based on `config.selected_model`
  - supports Gemini auth by API key **or** OAuth token file in temp dir
  - MIME sniffing helper for image bytes
- `src-tauri/src/ai/gemini.rs`
  - calls Google Generative Language API
  - supports API key query param or OAuth bearer token
- `src-tauri/src/ai/claude.rs`
  - calls Anthropic Messages API
  - uses detected image MIME type for screenshots
- `src-tauri/src/oauth_server.rs`
  - local callback server for Google OAuth (`http://localhost:8080/oauth/callback`)
  - serves `/icon.png` from embedded bytes
- `src-tauri/src/google_oauth.rs`
  - constructs auth URL + exchanges code for tokens
  - also contains refresh logic (not all paths currently use refresh)
- `src-tauri/src/screenshot.rs`
  - OS display enumeration + capture (screenshots crate)
  - encodes capture as JPEG bytes and (optionally) downsizes for limits

## “Is all code used?” (short, honest answer)

Not necessarily.

- **Frontend**: runtime reachability is by imports from `src/main.tsx`. If a file is not imported transitively, it is not used in the bundle.
  - In the current tree, **`src/hooks/useWindowSizeManager.ts` is not imported anywhere**, so it is unused unless you wire it back in.
- **Backend**: everything in `src-tauri/src/` is compiled into the binary, but some methods can remain “dead code” unless invoked by the UI (e.g., OAuth refresh helpers).

## Key runtime concepts

### 1) Chrome integration via CDP (not a Chrome extension)

Chrome tabs are fetched from:

- `http://localhost:9222/json/list`

This requires Chrome to be running with:

- `--remote-debugging-port=9222`

This project includes a “CDP Chrome” launcher (separate profile) so you don’t need to close your normal Chrome:

- `src-tauri/src/chrome/launcher.rs`
- Tauri command: `open_chrome_cdp` → `chrome::launch_chrome_cdp_window()`

### 2) Input sources in the UI

The frontend’s `InputSource` is a union of:

- a **Chrome tab** (from CDP), or
- an OS **display** (from `screenshots::Screen::all()`)

This is why the UI shows “Input Source” rather than only tabs.

### 3) Extraction modes

Frontend setting: `useScreenshot` (stored in `localStorage`)

- **Text mode** (Chrome tab only): `extract_tab_text` command which runs:
  - `chrome::execute_javascript(tab_id, "document.body.innerText")`
- **Screenshot mode**
  - Chrome tab: `capture_tab_screenshot` (CDP `Page.captureScreenshot` as JPEG bytes, written to a temp file)
  - Display: `capture_display_screenshot` (OS capture → encoded as JPEG bytes, written to a temp file)

### 4) AI providers and payload formats

Routing happens in:

- `src-tauri/src/ai/mod.rs`

Providers:

- `src-tauri/src/ai/gemini.rs`
- `src-tauri/src/ai/claude.rs`

Important: screenshots are typically JPEG bytes. The code detects the real MIME type using “magic bytes” via:

- `ai::detect_image_mime_type(image_data)`

This prevents provider errors like “image data does not match media type image/png”.

## End-to-end flows

### A) Fetch tabs (and thumbnails)

Frontend (`src/App.tsx`):

- Poll CDP status: `get_cdp_status` every 3s
- On refresh: `get_displays` + `get_display_thumbnail`
- If CDP ready: `get_chrome_tabs` + `get_tab_thumbnail`

Backend:

- `get_chrome_tabs` → `chrome::get_all_tabs()` → `GET /json/list` filtered to `type == "page"`
- thumbnails/screenshots are produced through CDP WebSocket per tab

### B) Solve (text mode)

Frontend:

- `activate_tab(tabId)`
- `extract_tab_text(tabId)` (JS: `document.body.innerText`)
- `buildPrompt(template, language, text)`
- `query_ai(prompt, config)`

### C) Solve (screenshot mode)

Frontend:

- `activate_tab(tabId)` (only for tab screenshot)
- `capture_tab_screenshot(tabId)` or `capture_display_screenshot(displayId)`
- `buildPrompt(template, language)` (prompt without injected text)
- `query_ai_with_image(prompt, imagePath, config)`

Backend:

- reads bytes from `imagePath` and sends to Gemini/Claude with correct MIME type

## Tauri commands (API surface)

Defined in `src-tauri/src/main.rs` and used by the frontend via `invoke(...)`.

Chrome/CDP:

- `get_chrome_tabs() -> Vec<ChromeTab>`
- `get_cdp_status() -> String`
- `open_chrome_cdp() -> String`
- `activate_tab(tab_id: String) -> ()`
- `extract_tab_text(tab_id: String) -> String`
- `capture_tab_screenshot(tab_id: String) -> String` (returns temp file path)
- `get_tab_thumbnail(tab_id: String) -> String` (data URL)

Displays:

- `get_displays() -> Vec<DisplayInfo>`
- `capture_display_screenshot(display_id: String) -> String` (returns temp file path)
- `get_display_thumbnail(display_id: String) -> String` (data URL)

AI:

- `query_ai(prompt: String, config: AIConfig) -> String`
- `query_ai_with_image(prompt: String, image_path: String, config: AIConfig) -> String`

OAuth:

- `start_google_oauth() -> String`
- `get_google_token_status() -> bool`
- `clear_google_tokens() -> String`

Window:

- `resize_window(window: Window, width: f64, height: f64) -> ()`

## Global hotkey (“LeetCode Wizard”-style workflow)

Backend registers a global hotkey on startup:

- `CmdOrCtrl+Shift+L` (constant `SOLVE_HOTKEY`)

When pressed, Rust emits the event:

- `hotkey-solve`

Frontend listens for `hotkey-solve` and triggers the same “Solve” pipeline using the currently selected Input Source.

Notes / limitations:

- This does **not** auto-detect “currently active Chrome tab”; it runs on the **tab selected in the app**.
- To fully match LeetCode Wizard, you likely want:
  - active-tab detection (platform-specific or deeper CDP integration)
  - an overlay / small always-on-top result panel that doesn’t steal focus

## OAuth / credentials

The UI supports:

- Gemini API key
- Claude API key
- Gemini OAuth token (optional)

Gemini OAuth tokens are stored at:

- `${tempDir}/cracking_interview_google_tokens.json`

The OAuth flow uses:

- `src-tauri/src/oauth_server.rs` (localhost callback server)
- `src-tauri/src/google_oauth.rs` (token exchange + refresh helpers)

Environment variables are loaded from `../.env` (repo root) on startup:

- `GOOGLE_CLIENT_ID`
- `GOOGLE_CLIENT_SECRET`

## Build + run

Frontend:

- `npm run dev`
- `npm run build`

Tauri:

- `npm run tauri dev`
- `npm run tauri build`

## Quick manual / MCP test via browser (dev URL)

When running in dev mode, Tauri points to the Vite dev server:

- **Dev URL**: `http://127.0.0.1:1420/` (see `src-tauri/tauri.conf.json` → `build.devUrl`)

This is useful for quick UI iteration and automated checks:

- **Manual check**: open the dev URL in a normal browser and validate UI behavior.
- **MCP-driven check**: if you have an MCP browser driver (e.g. Playwright/Chrome automation), you can load the dev URL and interact with the UI to verify flows end-to-end.

Notes:

- The URL only works **if the dev server is running** (typically via `npm run tauri dev` or `npm run dev`).
- Some functionality (global hotkey, OS display capture permissions) requires the native Tauri context; for those, prefer `npm run tauri dev`.

## Known technical debt / refactor targets

- **App.tsx is doing a lot**: UI + state + settings + prompt editing + OAuth UX. Consider splitting into:
  - `useCdpSources()` hook
  - `useAiSettings()` hook
  - `useSolveFlow()` hook
- **CDP commands use a constant `"id": 1`** for WebSocket requests. If you ever add concurrency, switch to incrementing IDs and matching responses.
- **GoogleOAuthService has refresh helpers** that aren’t wired into the “load token from file then refresh” path; if you want robust OAuth, load tokens on startup and refresh automatically.


