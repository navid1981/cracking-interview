export interface ModelInfo {
  id: string;
  name: string;
  provider: string;
}

export const PRO_MODELS: ModelInfo[] = [
  { id: 'gpt-5.2-codex', name: 'GPT-5.2 Codex', provider: 'OpenAI' },
  { id: 'claude-sonnet-5', name: 'Claude Sonnet 5', provider: 'Anthropic' },
  { id: 'gemini-3-flash', name: 'Gemini 3 Flash', provider: 'Google' },
  { id: 'grok-4.3', name: 'Grok 4.3', provider: 'xAI' },
];

export const FREE_MODEL: ModelInfo = {
  id: 'gemini-3.5-flash-lite',
  name: 'Gemini 3.5 Flash-Lite',
  provider: 'Google',
};

// Used by free users who paste their own Gemini API key; the app calls the
// Google Generative Language API directly, so `id` must be a native Gemini model ID.
// Must be a model on the Gemini API Free tier (AI Studio key, no billing account);
// 2.5 models are restricted to accounts that already used them.
export const BYO_MODEL: ModelInfo = {
  id: 'gemini-3.5-flash-lite',
  name: 'Gemini 3.5 Flash-Lite',
  provider: 'Google',
};

// Served by `get-models` to app releases that don't send `?v=2`. Those releases use
// `free_model.id` for BYO-key calls and their Rust only accepts 'gemini-2.5-flash',
// so this must stay frozen until those releases are retired.
export const LEGACY_FREE_MODEL: ModelInfo = {
  id: 'gemini-2.5-flash',
  name: 'Gemini 2.5 Flash',
  provider: 'Google',
};

// Deepgram speech-to-text model for live transcription (returned by `deepgram-key`).
export const TRANSCRIPTION_MODEL = 'nova-3';

export const DEFAULT_PRO_MODEL = 'gpt-5.2-codex';

export const PRO_MODEL_IDS = PRO_MODELS.map(m => m.id);

export const MODEL_MAP: Record<string, string> = {
  'gpt-5.2-codex': 'openai/gpt-5.2-codex',
  'claude-sonnet-5': 'anthropic/claude-sonnet-5',
  'gemini-3-flash': 'google/gemini-3-flash-preview',
  'grok-4.3': 'x-ai/grok-4.3',
  'gemini-3.5-flash-lite': 'google/gemini-3.5-flash-lite',
};
