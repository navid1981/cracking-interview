import { invoke } from '@tauri-apps/api/core';

export interface ModelInfo {
  id: string;
  name: string;
  provider: string;
}

// Model lists, free-tier site list, and usage limits come exclusively from the `get-models` edge function
// (supabase/functions/_shared/); the app must not hardcode model IDs or limits.
export interface ModelConfig {
  pro_models: ModelInfo[];
  free_model: ModelInfo;
  byo_model: ModelInfo;
  default_pro_model: string;
  free_call_limit: number;
  pro_request_limit: number;
  pro_audio_seconds_limit: number;
  audio_session_max_seconds: number;
  free_allowed_domains: string[];
  max_output_tokens: number;
}

// Server sentinel in `free_allowed_domains` meaning free/BYO users have no site restriction.
const ALL_DOMAINS = 'all';

export function allowsAllDomains(config: ModelConfig): boolean {
  return config.free_allowed_domains.includes(ALL_DOMAINS);
}

export function isFreeTierUrlAllowed(config: ModelConfig, url: string): boolean {
  return allowsAllDomains(config) || config.free_allowed_domains.some(domain => url.includes(domain));
}

const isNonNegativeInt = (v: unknown) => Number.isInteger(v) && (v as number) >= 0;

const CACHE_KEY = 'cached_models';

export function isValidModelConfig(config: any): config is ModelConfig {
  return !!(
    config?.pro_models?.length &&
    config?.free_model?.id &&
    config?.byo_model?.id &&
    config?.default_pro_model &&
    isNonNegativeInt(config?.free_call_limit) &&
    isNonNegativeInt(config?.pro_request_limit) &&
    isNonNegativeInt(config?.pro_audio_seconds_limit) &&
    isNonNegativeInt(config?.audio_session_max_seconds) &&
    config?.free_allowed_domains?.length &&
    Number.isInteger(config?.max_output_tokens) && config.max_output_tokens > 0
  );
}

export function loadCachedModels(): ModelConfig | null {
  try {
    const cached = localStorage.getItem(CACHE_KEY);
    if (cached) {
      const parsed = JSON.parse(cached);
      if (isValidModelConfig(parsed)) return parsed;
    }
  } catch { /* ignore */ }
  return null;
}

/** Fetches config from the server and caches it. An empty token uses the anon key. */
export async function fetchRemoteModelConfig(accessToken = ''): Promise<ModelConfig | null> {
  try {
    const config = await invoke<unknown>('fetch_models', { accessToken });
    if (!isValidModelConfig(config)) return null;
    localStorage.setItem(CACHE_KEY, JSON.stringify(config));
    return config;
  } catch {
    return null;
  }
}
