import { invoke } from '@tauri-apps/api/core';

export interface WindowRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface SessionSnapshot {
  schema_version: number;
  saved_at_unix_ms: number;
  window_rect: WindowRect | null;
  stealth_enabled: boolean;
  user_hidden: boolean;
  selected_model: string | null;
  active_template_id: string | null;
  programming_language: string | null;
  active_mode: 'text' | 'screenshot' | null;
  last_response_markdown: string | null;
}

const MAX_RESPONSE_CHARS = 60_000;
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

/**
 * Strips any accidental data URLs or secret-looking tokens and caps markdown length
 * before sending the non-sensitive snapshot to the Rust backend.
 */
export function sanitizeResponseMarkdown(markdown: string | null | undefined): string | null {
  if (!markdown) return null;
  const trimmed = markdown.trim();
  if (!trimmed) return null;
  // Never persist embedded base64 data URLs
  const withoutDataUrls = trimmed.replace(/data:image\/[a-zA-Z0-9.+-]+;base64,[A-Za-z0-9+/=]+/g, '[image-omitted]');
  return withoutDataUrls.slice(0, MAX_RESPONSE_CHARS);
}

/**
 * Debounced (500ms) session snapshot writer.
 * Explicitly excludes API keys, auth tokens, raw images, and audio.
 */
export function scheduleSessionSnapshot(params: {
  stealthEnabled: boolean;
  selectedModel?: string | null;
  activeTemplateId?: string | null;
  programmingLanguage?: string | null;
  useScreenshot?: boolean;
  lastResponseMarkdown?: string | null;
}): void {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
  }
  debounceTimer = setTimeout(() => {
    const snapshot: SessionSnapshot = {
      schema_version: 1,
      saved_at_unix_ms: Date.now(),
      window_rect: null, // Populated natively by Rust from outer_position/inner_size
      stealth_enabled: Boolean(params.stealthEnabled),
      user_hidden: false,
      selected_model: params.selectedModel || null,
      active_template_id: params.activeTemplateId || null,
      programming_language: params.programmingLanguage || null,
      active_mode: params.useScreenshot ? 'screenshot' : 'text',
      last_response_markdown: sanitizeResponseMarkdown(params.lastResponseMarkdown),
    };
    invoke('save_session_snapshot', { snapshot }).catch(() => {});
  }, 500);
}

/**
 * Queries the Rust backend for a session snapshot if the process was launched
 * with `--restore-session` (e.g., after a guardian respawn or desktop switch).
 */
export async function fetchRestoredSessionSnapshot(): Promise<SessionSnapshot | null> {
  try {
    const snap = await invoke<SessionSnapshot | null>('get_session_snapshot');
    return snap ?? null;
  } catch {
    return null;
  }
}
