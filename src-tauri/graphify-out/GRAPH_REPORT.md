# Graph Report - src-tauri  (2026-10-13)

## Corpus Check
- 22 files · ~69,879 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 5 file(s) not represented in the graph (top: (none) 3, .icns 1, .ico 1)

## Summary
- 421 nodes · 725 edges · 28 communities (24 shown, 4 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 6 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `dfd3d15a`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- main.rs
- launcher.rs
- chrome/mod.rs
- AudioRecorder
- bundle
- transcription.rs
- set_hotkeys
- gemini.rs
- stealth_hotkey.rs
- ai/mod.rs
- HotkeyAction
- audio.rs
- documents.rs
- setup.rs
- AppHandle
- init_macos_event_tap
- WebviewWindow
- dispatch_action
- screenshot.rs
- CFRunLoopAddSource
- teardown_stealth_hotkeys
- macos_event_tap_callback
- get_window_inner_size
- refresh_stealth_status
- STEALTH_ENABLED
- cracking-interview

## God Nodes (most connected - your core abstractions)
1. `AudioRecorder` - 24 edges
2. `set_hotkeys()` - 17 edges
3. `HotkeyAction` - 17 edges
4. `WsBrowserHandle` - 14 edges
5. `HotkeysConfig` - 12 edges
6. `main()` - 11 edges
7. `get_ws_browser_handle()` - 10 edges
8. `launch_chrome_cdp_window()` - 10 edges
9. `TabSession` - 10 edges
10. `reset_hotkeys_to_default()` - 10 edges

## Surprising Connections (you probably didn't know these)
- `activate_tab()` --calls--> `get_ws_browser_handle()`  [INFERRED]
  src/chrome/mod.rs → src/chrome/launcher.rs
- `capture_screenshot()` --calls--> `get_ws_browser_handle()`  [INFERRED]
  src/chrome/mod.rs → src/chrome/launcher.rs
- `capture_thumbnail_inner()` --calls--> `get_ws_browser_handle()`  [INFERRED]
  src/chrome/mod.rs → src/chrome/launcher.rs
- `execute_javascript()` --calls--> `get_ws_browser_handle()`  [INFERRED]
  src/chrome/mod.rs → src/chrome/launcher.rs
- `get_all_tabs()` --calls--> `get_ws_browser_handle()`  [INFERRED]
  src/chrome/mod.rs → src/chrome/launcher.rs

## Import Cycles
- None detected.

## Communities (28 total, 4 thin omitted)

### Community 0 - "main.rs"
Cohesion: 0.05
Nodes (21): AI_IMAGE_TOTAL_BYTES, AIProxyResponse, AIProxyUsage, apply_macos_dock_hiding(), DEFAULT_AUDIO_TOGGLE_HOTKEY, DEFAULT_MOVE_DOWN_HOTKEY, DEFAULT_MOVE_LEFT_HOTKEY, DEFAULT_MOVE_RIGHT_HOTKEY (+13 more)

### Community 1 - "launcher.rs"
Cohesion: 0.10
Nodes (22): BrowserCmd, CDP_CHROME_PID, CDP_PORT, ChromeSource, AppChrome, UserChrome, detect_user_chrome_port(), get_cdp_port() (+14 more)

### Community 2 - "chrome/mod.rs"
Cohesion: 0.10
Nodes (29): activate_tab(), capture_screenshot(), capture_screenshot_http(), capture_screenshot_ws(), capture_thumbnail(), capture_thumbnail_http(), capture_thumbnail_inner(), capture_thumbnail_ws() (+21 more)

### Community 3 - "AudioRecorder"
Cohesion: 0.08
Nodes (7): AVFoundation, CoreGraphics, CoreMedia, Foundation, AudioRecorder, AudioRecorderApp, ScreenCaptureKit

### Community 4 - "bundle"
Cohesion: 0.06
Nodes (31): app, security, windows, withGlobalTauri, build, beforeBuildCommand, beforeDevCommand, devUrl (+23 more)

### Community 5 - "transcription.rs"
Cohesion: 0.10
Nodes (10): TabSession, Http, Ws, capture_audio_to_channel(), DeepgramAlternative, DeepgramChannel, DeepgramResponse, run_transcription_session() (+2 more)

### Community 6 - "set_hotkeys"
Cohesion: 0.22
Nodes (18): default_hotkeys(), get_hotkeys(), HotkeysConfig, HotkeysState, load_hotkeys_from_disk(), main(), normalize_hotkey(), register_hotkeys() (+10 more)

### Community 7 - "gemini.rs"
Cohesion: 0.21
Nodes (12): error_message(), extract_text(), generate_content_url(), generation_config(), is_overloaded_message(), is_quota_message(), no_response_reason(), parse_retry_after_seconds() (+4 more)

### Community 8 - "stealth_hotkey.rs"
Cohesion: 0.16
Nodes (10): HOOK_INITIALIZED, MAC_EVENT_TAP, MAC_RUN_LOOP, SWALLOWING_ACTIVE, WIN_ALT_COMBO_FIRED, WIN_CTRL_COMBO_FIRED, WIN_HOOK_HANDLE, WIN_HOOK_THREAD_ID (+2 more)

### Community 9 - "ai/mod.rs"
Cohesion: 0.15
Nodes (14): AIConfig, query_with_images(), query_with_text(), validate_config(), extract_tab_text(), fetch_models(), load_images_for_ai(), query_ai() (+6 more)

### Community 10 - "HotkeyAction"
Cohesion: 0.12
Nodes (15): HotkeyAction, AudioToggle, MoveDown, MoveLeft, MoveRight, MoveUp, QuitApp, ScrollDown (+7 more)

### Community 11 - "audio.rs"
Cohesion: 0.18
Nodes (8): compile_helper_from_source(), ensure_executable(), find_helper_binary(), MAX_RECORDING_SECONDS, prewarm_audio_recorder(), prewarm_helper(), warm_audio_capture(), WarmAudioState

### Community 12 - "documents.rs"
Cohesion: 0.27
Nodes (11): DocumentPlaceholder, documents_config_path(), extract_doc(), extract_docx(), extract_pdf(), extract_text_from_bytes(), extract_text_from_ooxml(), extract_txt() (+3 more)

### Community 13 - "setup.rs"
Cohesion: 0.26
Nodes (9): get_setup_status(), CGPreflightScreenCaptureAccess(), CGRequestScreenCaptureAccess(), chrome_installed(), platform(), request_screen_recording(), screen_recording_granted(), SetupStatus (+1 more)

### Community 14 - "AppHandle"
Cohesion: 0.26
Nodes (11): app_config_dir(), delete_document_placeholder(), get_document_placeholders(), hotkeys_config_path(), load_stealth_preference(), restart_app(), save_document_placeholder(), save_stealth_preference() (+3 more)

### Community 15 - "init_macos_event_tap"
Cohesion: 0.24
Nodes (7): CFMachPortCreateRunLoopSource(), CFRelease(), CFRunLoopRun(), CGEventTapCreate(), CGEventTapEnable(), init_macos_event_tap(), request_accessibility_permission()

### Community 16 - "WebviewWindow"
Cohesion: 0.22
Nodes (9): apply_macos_fullscreen_overlay(), apply_macos_screen_capture_protection(), apply_windows_stealth(), reapply_stealth_after_show(), register_hotkey(), remove_windows_stealth(), restore_macos_screen_capture_visibility(), set_stealth_mode() (+1 more)

### Community 17 - "dispatch_action"
Cohesion: 0.36
Nodes (6): bring_to_front_without_focus(), dispatch_action(), init_windows_hook(), refresh_stealth_hotkeys(), setup_stealth_hotkeys(), toggle_visibility_without_focus()

### Community 18 - "screenshot.rs"
Cohesion: 0.33
Nodes (3): get_displays(), DisplayInfo, get_all_displays()

### Community 19 - "CFRunLoopAddSource"
Cohesion: 0.33
Nodes (4): CFRunLoopAddSource(), CFRunLoopGetCurrent(), CFRunLoopStop(), kCFRunLoopCommonModes

### Community 20 - "teardown_stealth_hotkeys"
Cohesion: 0.40
Nodes (3): send_mask_key(), teardown_stealth_hotkeys(), windows_keyboard_proc()

### Community 21 - "macos_event_tap_callback"
Cohesion: 0.60
Nodes (3): CGEventGetFlags(), CGEventGetIntegerValueField(), macos_event_tap_callback()

### Community 22 - "get_window_inner_size"
Cohesion: 0.40
Nodes (4): get_window_inner_size(), move_window_by(), resize_window(), WindowInnerSize

### Community 23 - "refresh_stealth_status"
Cohesion: 1.00
Nodes (3): get_stealth_status(), refresh_stealth_status(), StealthStatus

## Knowledge Gaps
- **63 isolated node(s):** `cracking-interview`, `Foundation`, `AVFoundation`, `CoreMedia`, `ScreenCaptureKit` (+58 more)
  These have ≤1 connection - possible missing edges. (Counts symbols only; 150 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `HotkeyAction` connect `HotkeyAction` to `stealth_hotkey.rs`, `dispatch_action`?**
  _High betweenness centrality (0.051) - this node is a cross-community bridge._
- **What connects `cracking-interview`, `Foundation`, `AVFoundation` to the rest of the system?**
  _63 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.04625346901017576 - nodes in this community are weakly interconnected._
- **Why does `get_setup_status()` connect `setup.rs` to `main.rs`?**
  _High betweenness centrality (0.042) - this node is a cross-community bridge._
- **Should `launcher.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10317460317460317 - nodes in this community are weakly interconnected._
- **Should `chrome/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10317460317460317 - nodes in this community are weakly interconnected._
- **Should `AudioRecorder` be split into smaller, more focused modules?**
  _Cohesion score 0.08199643493761141 - nodes in this community are weakly interconnected._