// Subscription limits. Served to the app by `get-models?v=2` and enforced by `ai-proxy`,
// `deepgram-key`, and `log-audio-usage`; the app must not hardcode these values.
export const FREE_LIFETIME_CALL_LIMIT = 3;

export const PRO_MONTHLY_REQUEST_LIMIT = 150;
export const PRO_MONTHLY_AUDIO_SECONDS = 36000;   // 10 hours per billing period
export const PRO_AUDIO_SESSION_MAX_SECONDS = 5400; // 90 minutes per live session

// Sites free (and BYO-key) users may run AI on; matched as substrings of the tab URL.
// `['all']` lifts the site restriction. An empty list is treated by the app as "config not loaded".
export const ALL_DOMAINS = 'all';
// export const FREE_TIER_ALLOWED_DOMAINS = ['leetcode.com', 'codewars.com', 'codeforces.com', 'neetcode.io'];
export const FREE_TIER_ALLOWED_DOMAINS = ['all'];

export function isFreeTierUrlAllowed(url: string): boolean {
  return FREE_TIER_ALLOWED_DOMAINS.some(domain => domain === ALL_DOMAINS || url.includes(domain));
}

// Max LLM output tokens per request (proxy and BYO-key calls).
export const MAX_OUTPUT_TOKENS = 16384;
