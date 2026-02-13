# CrackingInterview — AI Project Guide (for Coding Agents)

This document is written for an AI coding agent (and future maintainers) to quickly understand **what this repo is**, **how data flows**, and **where to make changes**.

## What this app does (current behavior)

CrackingInterview is a **Tauri (Rust) + React** desktop app that helps a software engineer solve algorithm questions in sites like LeetCode/HackerRank by:

- **Listing Chrome tabs** (via **Chrome DevTools Protocol** / CDP) and letting the user choose an input source
- **Extracting content** from the selected source either as:
  - **Text** (execute JS in the tab to read `document.body.innerText`), or
  - **Screenshot** (CDP tab screenshot or OS display capture)
  - **Audio** (system audio recording sent directly to AI models that support audio input)
- Sending that content into an **LLM** (via OpenRouter proxy or user's own API key)
- Showing the **AI response** inside the app UI

There is also an explicit goal to support a "LeetCode Wizard"-style workflow (see `https://leetcodewizard.io/`) where the user can trigger "Solve" using a **global hotkey** without leaving Chrome.

## Repo structure (ignore generated dirs)

Do **not** try to "read everything" including generated artifacts. In this repo, these dirs are generated/large:

- `node_modules/`
- `dist/` (frontend build output)
- `src-tauri/target/` (Rust build output)

The primary source code is:

- **Frontend**: `src/`
  - `src/main.tsx`: React entry point (mounts `App`)
  - `src/App.tsx`: main UI + orchestration (CDP status, source list, solve flow, settings modal, hotkey listener, auth state)
  - `src/App.css`: global app styles
  - `src/services/prompts.ts`: prompt templates + `buildPrompt(...)`
  - `src/services/supabase.ts`: Supabase client, auth helpers, usage stats
  - `src/components/TabDropdown.tsx`: custom dropdown for **InputSource** with thumbnails
  - `src/components/TabDropdown.css`: styles for `TabDropdown.tsx`
  - `src/components/AIResponseDisplay.tsx`: renders AI response, parses common markers/blocks, syntax-highlights code
  - `src/components/PromptEditor.tsx` + `.css`: UI for editing prompt templates
  - `src/components/PromptListView.tsx` + `.css`: UI for selecting templates and entering edit mode
  - `src/components/AuthScreen.tsx` + `.css`: Sign in/sign up container
  - `src/components/SignInForm.tsx`: Email/password sign in form
  - `src/components/SignUpForm.tsx`: Email/password sign up form
- **Backend (Tauri/Rust)**: `src-tauri/src/`
  - `main.rs`: Tauri commands (IPC) + global hotkey registration
  - `chrome/*`: Chrome CDP integration (tabs, activate, execute JS, screenshots)
  - `ai/*`: Gemini + Claude HTTP clients + provider routing
  - `audio.rs`: System audio recording (macOS: ScreenCaptureKit via Swift helper, Windows: WASAPI loopback), MP3 encoding
  - `screenshot.rs`: OS display capture (screenshots crate), thumbnails
- **Resources**: `src-tauri/resources/`
  - `audio_recorder.swift`: Swift helper for macOS audio recording (compiled at runtime)
- **Supabase Edge Functions**: `supabase/functions/`
  - `ai-proxy/index.ts`: OpenRouter proxy with quota enforcement
  - `create-checkout/index.ts`: Stripe checkout session creation (production)
  - `create-checkout-test/index.ts`: Stripe checkout (test mode)
  - `create-billing-portal/index.ts`: Stripe Customer Portal (production)
  - `create-billing-portal-test/index.ts`: Stripe Customer Portal (test mode)
  - `stripe-webhook/index.ts`: Stripe webhook handler (production)
  - `stripe-webhook-test/index.ts`: Stripe webhook handler (test mode)
  - `ping/index.ts`: Network latency diagnostic

### "Is this file used?" — how to verify quickly

In this repo, most runtime wiring is via **imports**:

- A TS/TSX file is "used" if it is imported (directly or transitively) from `src/main.tsx`.
- A CSS file is "used" if it is imported by some TS/TSX module that is imported from `src/main.tsx`.

Example: `TabDropdown.tsx` is imported by `src/App.tsx`, and `TabDropdown.css` is imported by `TabDropdown.tsx`, so both are in the runtime bundle.

## Frontend file map (what calls what)

Entry:

- `src/main.tsx` → renders `<App />` from `src/App.tsx`

App composition (current):

- `src/App.tsx`
  - imports `TabDropdown` for Input Source selection
  - imports `AIResponseDisplay` to render the final LLM output
  - imports `PromptEditor` / `PromptListView` for prompt-template UX
  - imports `AuthScreen` for sign in/sign up flow
  - calls Tauri commands using `invoke(...)` for CDP, screenshots, AI, auth
  - listens for backend-emitted hotkey events (`hotkey-solve-text`, `hotkey-solve-screenshot`, `hotkey-audio-toggle`)

Component responsibilities:

- `src/components/TabDropdown.tsx`
  - renders a dropdown list of `InputSource` entries
  - supports thumbnail previews (`thumbnail` is expected to be a base64 data URL)
  - imports its own styles from `src/components/TabDropdown.css`
- `src/components/AIResponseDisplay.tsx`
  - tries to parse response using markers (EXPLANATION_START/END, SOLUTION_START/END)
  - falls back to markdown code fences or heuristics (`class Solution`)
  - uses `react-syntax-highlighter` for code formatting
- `src/components/AuthScreen.tsx`
  - Container for authentication views (sign in, sign up, forgot password)
  - Light theme with app branding
- `src/components/SignInForm.tsx`
  - Email/password sign in
  - Auto-fills email from previous session
- `src/components/SignUpForm.tsx`
  - Email/password sign up

## Backend file map (Rust/Tauri)

- `src-tauri/src/main.rs`
  - defines all Tauri commands exposed to the frontend (`invoke_handler`)
  - registers global hotkeys and emits events (frontend listens)
  - domain validation for free tier restrictions
- `src-tauri/src/chrome/mod.rs`
  - CDP tab listing (`GET http://localhost:9222/json/list`)
  - activate tab (`/json/activate/<id>`)
  - execute JS via per-tab WebSocket (`Runtime.evaluate`)
  - screenshots / thumbnails via CDP (`Page.captureScreenshot`)
- `src-tauri/src/chrome/launcher.rs`
  - launches a dedicated Chrome instance with `--remote-debugging-port=9222`
  - tries to avoid killing user's primary Chrome session
- `src-tauri/src/ai/mod.rs`
  - routes to Gemini vs Claude based on `config.selected_model`
  - MIME sniffing helper for image bytes
- `src-tauri/src/ai/gemini.rs`
  - calls Google Generative Language API
  - supports API key authentication
- `src-tauri/src/ai/claude.rs`
  - calls Anthropic Messages API
  - uses detected image MIME type for screenshots
- `src-tauri/src/audio.rs`
  - System audio recording for both macOS and Windows
  - macOS: Spawns Swift helper process (`audio_recorder.swift`) using ScreenCaptureKit
  - Windows: WASAPI loopback capture in separate thread
  - WAV to MP3 conversion using `mp3lame-encoder` crate (statically linked)
  - Warm mode support for instant recording start (macOS only)
  - 3-minute automatic timeout on both platforms
- `src-tauri/src/screenshot.rs`
  - OS display enumeration + capture (screenshots crate)
  - encodes capture as JPEG bytes and (optionally) downsizes for limits

## Key runtime concepts

### 1) Chrome integration via CDP (not a Chrome extension)

Chrome tabs are fetched from:

- `http://localhost:9222/json/list`

This requires Chrome to be running with:

- `--remote-debugging-port=9222`

This project includes a "CDP Chrome" launcher (separate profile) so you don't need to close your normal Chrome:

- `src-tauri/src/chrome/launcher.rs`
- Tauri command: `open_chrome_cdp` → `chrome::launch_chrome_cdp_window()`

### 2) Input sources in the UI

The frontend's `InputSource` is a union of:

- a **Chrome tab** (from CDP), or
- an OS **display** (from `screenshots::Screen::all()`)

This is why the UI shows "Input Source" rather than only tabs.

### 3) Extraction modes

Frontend setting: `useScreenshot` (stored in `localStorage`)

- **Text mode** (Chrome tab only): `extract_tab_text` command which runs:
  - `chrome::execute_javascript(tab_id, "document.body.innerText")`
- **Screenshot mode**
  - Chrome tab: `capture_tab_screenshot` (CDP `Page.captureScreenshot` as JPEG bytes, written to a temp file)
  - Display: `capture_display_screenshot` (OS capture → encoded as JPEG bytes, written to a temp file)
- **Audio mode** (system audio):
  - `start_audio_recording` / `stop_audio_recording`
  - Records system audio (interviewer voice from Zoom/Teams/etc.)
  - Returns MP3 file path (sent directly to Gemini which supports audio input)
  - 3-minute automatic timeout
  - macOS: Uses ScreenCaptureKit via compiled Swift helper with "warm mode" for instant start
  - Windows: Uses WASAPI loopback capture

### 4) AI providers and payload formats

Routing happens in:

- `src-tauri/src/ai/mod.rs`

Providers:

- `src-tauri/src/ai/gemini.rs`
- `src-tauri/src/ai/claude.rs`

Important: screenshots are typically JPEG bytes. The code detects the real MIME type using "magic bytes" via:

- `ai::detect_image_mime_type(image_data)`

This prevents provider errors like "image data does not match media type image/png".

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
- `query_ai_via_proxy(prompt, model, accessToken, sourceUrl)` (for proxy users)
- or `query_ai(prompt, config, sourceUrl)` (for BYO API key users)

### C) Solve (screenshot mode)

Frontend:

- `activate_tab(tabId)` (only for tab screenshot)
- `capture_tab_screenshot(tabId)` or `capture_display_screenshot(displayId)`
- `buildPrompt(template, language)` (prompt without injected text)
- `query_ai_via_proxy_with_image(prompt, imagePath, model, accessToken, sourceUrl)`
- or `query_ai_with_image(prompt, imagePath, config, sourceUrl)`

Backend:

- reads bytes from `imagePath` and sends to AI provider with correct MIME type

### D) Solve (audio mode)

Frontend:

- Select "Audio (System)" from Input Source dropdown → triggers `warm_audio_capture()` (macOS only, pre-initializes ScreenCaptureKit)
- Click record or press hotkey → `start_audio_recording()` - begins system audio capture
- (user waits, timer shows duration, max 3 minutes)
- Click stop or press hotkey → `stop_audio_recording()` - returns MP3 file path
- `buildPrompt(template, language, audioInstructions)` - uses audio-specific prompt
- `query_ai_via_proxy_with_audio(prompt, audioPath, 'gemini-3-flash', accessToken)`

Backend:

- Audio is recorded as WAV then converted to MP3 using mp3lame-encoder (bundled, no FFmpeg needed)
- MP3 is base64-encoded and sent to OpenRouter with `input_audio` content type
- Model is forced to `gemini-3-flash` (Google's Gemini model that supports audio input)
- OpenRouter routes to `google/gemini-3-flash-preview`

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

AI (Direct calls - user's own API key):

- `query_ai(prompt: String, config: AIConfig, source_url: Option<String>) -> String`
- `query_ai_with_image(prompt: String, image_path: String, config: AIConfig, source_url: Option<String>) -> String`

AI (Proxy calls - via Supabase Edge Function):

- `query_ai_via_proxy(prompt: String, model: String, access_token: String, source_url: Option<String>) -> AIProxyResponse`
- `query_ai_via_proxy_with_image(prompt: String, image_path: String, model: String, access_token: String, source_url: Option<String>) -> AIProxyResponse`

Audio:

- `start_audio_recording() -> ()` - begins system audio recording
- `stop_audio_recording() -> String` - stops recording and returns MP3 file path
- `warm_audio_capture() -> ()` - (macOS only) pre-initializes ScreenCaptureKit for instant recording start
- `cooldown_audio_capture() -> ()` - (macOS only) releases warm audio capture resources
- `is_audio_recording() -> bool` - checks if currently recording
- `query_ai_via_proxy_with_audio(prompt, audioPath, model, accessToken) -> AIProxyResponse` - sends audio to AI

Auth (Supabase - proxied through Rust to bypass corporate VPN SSL issues):

- `supabase_sign_up(email: String, password: String) -> SignUpResponse`
- `supabase_sign_in(email: String, password: String) -> SignInResponse`

Subscription:

- `create_checkout_session(user_id: String, user_email: String) -> CheckoutResponse`
- `create_billing_portal_session(customer_id: String) -> PortalResponse`

Utility:

- `open_url(url: String) -> ()` (opens URL in system browser)
- `resize_window(window: Window, width: f64, height: f64) -> ()`

Hotkeys:

- `get_hotkeys() -> HotkeyConfig`
- `set_hotkeys(config: HotkeyConfig) -> ()`

## Global hotkeys

Backend registers global hotkeys on startup (customizable in Settings → HotKeys):

| Action | macOS Default | Windows Default | Linux Default |
|--------|---------------|-----------------|---------------|
| Extract text → Solve | `Cmd+1` | `Alt+1` | `Ctrl+1` |
| Screenshot → Solve | `Cmd+2` | `Alt+2` | `Ctrl+2` |
| Audio Start/Stop → Solve | `Cmd+3` | `Alt+3` | `Ctrl+3` |

When pressed, Rust emits events that the frontend listens for:

- `hotkey-solve-text`
- `hotkey-solve-screenshot`
- `hotkey-audio-toggle`

Notes / limitations:

- This does **not** auto-detect "currently active Chrome tab"; it runs on the **tab selected in the app**.
- To fully match LeetCode Wizard, you likely want:
  - active-tab detection (platform-specific or deeper CDP integration)
  - an overlay / small always-on-top result panel that doesn't steal focus

## Authentication & Subscription System

The app includes a full authentication and subscription system using Supabase and Stripe.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Tauri Desktop App                                │
├─────────────────────────────────────────────────────────────────────────┤
│  AuthScreen (sign up/in)  →  MainApp (CDP, prompts, AI)                 │
│                                                                          │
│  Settings → Account Tab → Subscription status, quota display, upgrade    │
│  Settings → AI Models Tab → Model selection, BYO API key (free users)    │
│  Settings → HotKeys Tab → Customize keyboard shortcuts                   │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         Supabase Backend                                 │
├─────────────────────────────────────────────────────────────────────────┤
│  auth.users (Supabase Auth)                                              │
│  users table (subscription_status, lifetime_ai_calls, stripe IDs)        │
│  api_usage table (tracks each AI request)                                │
│                                                                          │
│  Edge Functions:                                                         │
│    - create-checkout / create-checkout-test (Stripe checkout)            │
│    - create-billing-portal / create-billing-portal-test (manage sub)     │
│    - stripe-webhook / stripe-webhook-test (subscription lifecycle)       │
│    - ai-proxy (OpenRouter proxy with quota enforcement)                  │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
            ┌─────────────┐                 ┌─────────────┐
            │   Stripe    │                 │ OpenRouter  │
            │ (payments)  │                 │ (AI models) │
            └─────────────┘                 └─────────────┘
```

### Subscription Tiers

| Tier | Cost | AI Quota | Input Source | Domain Restriction |
|------|------|----------|--------------|-------------------|
| Free | $0 | 3 lifetime calls | Chrome tabs only | leetcode.com, codewars.com, codeforces.com, neetcode.io |
| Free + BYO Key | $0 | Unlimited (own key) | Chrome tabs only | Same domain restriction |
| Pro | $10/month | 150 requests/billing period | All sources | No restriction |

### AI Models

**Free Tier:**
- Gemini 2.5 Flash (via OpenRouter proxy or BYO Gemini API key)

**Pro Tier:**
- GPT-5.2 Codex (OpenAI)
- Claude Sonnet 4.5 (Anthropic)
- Gemini 3 Flash (Google)
- Grok 4.1 Fast (xAI)

**Audio Input:**
- Always uses Gemini 3 Flash (only model supporting audio input)
- Model selection is overridden when audio source is used

### AI Routing Logic

The app supports two AI request paths:

1. **Proxy via Edge Function** (default for all users)
   - Quota enforced (150/billing period for paid, 3 lifetime for free)
   - Uses `query_ai_via_proxy`, `query_ai_via_proxy_with_image` commands
   - Calls Supabase Edge Function → OpenRouter
   - Provider sorted by throughput for fastest response

2. **Direct API calls** (BYO API key for free users who exhausted quota)
   - No quota limits from the app
   - Uses `query_ai`, `query_ai_with_image` commands
   - Calls Gemini API directly
   - Domain restrictions still enforced client-side

### Audio Recording Implementation

The app supports recording system audio (sound from Zoom/Teams/browser) and sending it directly to AI models that support audio input.

#### Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Audio Recording Flow                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  User selects Audio source  →  warm_audio_capture() [macOS only]        │
│                                 (pre-initializes ScreenCaptureKit)      │
│                                                                         │
│  User clicks Record  →  start_audio_recording()                         │
│                         ├── macOS: Swift helper via ScreenCaptureKit    │
│                         └── Windows: WASAPI loopback capture            │
│                                                                         │
│  User clicks Stop  →  stop_audio_recording()                            │
│                       ├── Stop capture                                  │
│                       ├── WAV → MP3 conversion (mp3lame-encoder)        │
│                       ├── Delete WAV file                               │
│                       └── Return MP3 path                               │
│                                                                         │
│  Frontend  →  query_ai_via_proxy_with_audio()                           │
│               ├── Read MP3, base64 encode                               │
│               ├── Send with input_audio content type                    │
│               └── Model: gemini-3-flash (forced)                        │
│                                                                         │
│  Edge Function  →  OpenRouter API                                       │
│                    Model: google/gemini-3-flash-preview                 │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

#### macOS Implementation

**Swift Helper** (`src-tauri/resources/audio_recorder.swift`):
- Compiled on first use (or app startup via pre-warming)
- Uses `ScreenCaptureKit` (macOS 13+) for system audio capture
- Supports two modes:
  - **Warm mode** (`--warm`): Pre-initializes ScreenCaptureKit, waits for "start"/"stop" commands via stdin
  - **Legacy mode**: Starts recording immediately
- Output: 44.1kHz mono WAV with volume boost and soft clipping
- 3-minute automatic timeout

**Warm Mode (Instant Start)**:
1. When user selects Audio source → `warm_audio_capture()` called
2. Swift helper spawned with `--warm` flag
3. ScreenCaptureKit initialized (~0.5s)
4. Helper waits for "start" command
5. When user clicks Record → "start" sent via stdin → Recording begins instantly
6. When user switches away from Audio → `cooldown_audio_capture()` terminates helper

**Pre-warming on App Startup**:
- `prewarm_audio_recorder()` called in Tauri setup
- Compiles Swift helper in background
- Eliminates ~1.5s compilation delay on first recording

#### Windows Implementation

**WASAPI Loopback** (`src-tauri/src/audio.rs` → `mod windows`):
- Uses Windows Audio Session API (WASAPI) in loopback mode
- Captures system audio output (what you hear from speakers/headphones)
- Resamples to 16kHz mono
- Volume boost (5x) with soft clipping
- 3-minute automatic timeout

#### MP3 Encoding

**Library**: `mp3lame-encoder` crate (statically linked, no external dependencies)

**Process**:
1. Record to WAV (temporary file)
2. Read WAV using `hound` crate
3. Encode to MP3 at 128kbps
4. Delete WAV file
5. Return MP3 path

**Why MP3 over WAV**:
- ~10x smaller file size
- Faster upload to OpenRouter
- Gemini supports both formats

#### OpenRouter Audio Format

Per [OpenRouter documentation](https://openrouter.ai/docs/guides/overview/multimodal/audio):

```json
{
  "model": "google/gemini-3-flash-preview",
  "messages": [
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "Listen to this audio..." },
        { 
          "type": "input_audio", 
          "input_audio": { 
            "data": "<base64_mp3>",
            "format": "mp3"
          }
        }
      ]
    }
  ]
}
```

**Important**: Audio must use `input_audio` content type (not `audio_url`).

#### Model Selection

Audio input **always** uses `gemini-3-flash` regardless of user's model selection:
- This is because only certain models support audio input
- `gemini-3-flash` → OpenRouter model ID: `google/gemini-3-flash-preview`
- Other Pro models (GPT-5.2, Claude 4.5, Grok 4.1) don't support audio input

### Billing Period vs Calendar Month

- **Pro users**: Quota is based on subscription billing period (subscription_start_date to subscription_end_date), NOT calendar month
- **Free users**: 3 lifetime calls total (never resets)
- When a Pro user renews, their `subscription_start_date` and `subscription_end_date` are updated, effectively resetting their quota

### Domain Restrictions (Security)

Free tier users are restricted to specific coding practice sites:
- `leetcode.com`
- `codewars.com`
- `codeforces.com`
- `neetcode.io`

This is enforced at:
1. **Frontend** (`src/App.tsx`): URL validation before AI call
2. **Rust backend** (`src-tauri/src/main.rs`): `validate_source_url()` for BYO API key users
3. **Edge Function** (`supabase/functions/ai-proxy/index.ts`): Server-side validation

### Stripe Webhook Events

The webhook handler (`stripe-webhook` / `stripe-webhook-test`) processes:

| Event | Action |
|-------|--------|
| `checkout.session.completed` | Create/update user, set `subscription_status: active`, `subscription_tier: pro` |
| `customer.subscription.created` | Set `subscription_status: active`, save subscription dates |
| `customer.subscription.updated` | Detect cancellation (`cancel_at` timestamp), set `subscription_status: cancelling` |
| `customer.subscription.deleted` | Set `subscription_status: cancelled`, `subscription_tier: free` |
| `invoice.payment_succeeded` | Renew subscription, reset billing period dates |
| `invoice.payment_failed` | Set `subscription_status: past_due` (grace period) |

### Database Schema

**users table** (extends auth.users):
- `id` (UUID, FK to auth.users.id)
- `email` (text)
- `subscription_status` (text: 'active', 'inactive', 'cancelled', 'cancelling', 'past_due')
- `subscription_tier` (text: 'free', 'pro')
- `stripe_customer_id` (text)
- `stripe_subscription_id` (text)
- `lifetime_ai_calls` (int, for free tier tracking)
- `subscription_start_date` (timestamp, billing period start)
- `subscription_end_date` (timestamp, billing period end)

**api_usage table**:
- `id` (UUID)
- `user_id` (UUID, FK to users.id)
- `ai_model` (text)
- `tokens_used` (int)
- `request_count` (int, default 1)
- `created_at` (timestamp)

**Automatic cleanup job** (pg_cron):
- Runs daily at 3:00 AM UTC
- Deletes api_usage records older than 3 months

### Auth Flow

1. User opens app → Auth session cleared (must sign in each session)
2. Email from previous session auto-filled for convenience
3. Sign in → Session stored in localStorage + Supabase
4. On success → Settings modal closes, main app shown
5. Sign out → Session cleared, returns to auth screen

### Environment Setup

**Supabase Edge Function Secrets:**
- `OPENROUTER_API_KEY`: Your OpenRouter API key
- `STRIPE_SECRET_KEY`: Stripe secret key (production)
- `STRIPE_SECRET_KEY_TEST`: Stripe secret key (test mode)
- `STRIPE_WEBHOOK_SECRET`: Stripe webhook signing secret (production)
- `STRIPE_WEBHOOK_SECRET_TEST`: Stripe webhook signing secret (test mode)

**Frontend constants** (in `src/services/supabase.ts`):
- `SUPABASE_URL`
- `SUPABASE_ANON_KEY`

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

- **App.tsx is doing a lot**: UI + state + settings + prompt editing + Auth state + subscription logic. Consider splitting into:
  - `useCdpSources()` hook
  - `useAiSettings()` hook
  - `useSolveFlow()` hook
  - `useAuth()` hook (for Supabase auth state)
  - `useSubscription()` hook
- **CDP commands use a constant `"id": 1`** for WebSocket requests. If you ever add concurrency, switch to incrementing IDs and matching responses.
- **Audio sends directly to AI**: Local speech-to-text (Vosk) was removed. Audio is now sent directly to Gemini which handles transcription and understanding natively. This is simpler and more accurate.
- **Test vs Production Edge Functions**: There are duplicate functions (`-test` suffix) for Stripe integration. Consider using environment variables to switch between test/prod instead of separate functions.
- **Corporate VPN workarounds**: Auth and some API calls go through Rust backend to bypass SSL inspection issues. This adds complexity but is necessary for some enterprise environments.
