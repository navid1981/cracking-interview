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
  id: 'gemini-2.5-flash',
  name: 'Gemini 2.5 Flash',
  provider: 'Google',
};

export const DEFAULT_PRO_MODEL = 'gpt-5.2-codex';

export const PRO_MODEL_IDS = PRO_MODELS.map(m => m.id);

export const MODEL_MAP: Record<string, string> = {
  'gpt-5.2-codex': 'openai/gpt-5.2-codex',
  'claude-sonnet-5': 'anthropic/claude-sonnet-5',
  'gemini-3-flash': 'google/gemini-3-flash-preview',
  'grok-4.3': 'x-ai/grok-4.3',
  'gemini-2.5-flash': 'google/gemini-2.5-flash',
};
