import { PRO_MODELS, FREE_MODEL, BYO_MODEL, LEGACY_FREE_MODEL, DEFAULT_PRO_MODEL } from "../_shared/models.ts";
import {
  FREE_LIFETIME_CALL_LIMIT,
  PRO_MONTHLY_REQUEST_LIMIT,
  PRO_MONTHLY_AUDIO_SECONDS,
  PRO_AUDIO_SESSION_MAX_SECONDS,
  FREE_TIER_ALLOWED_DOMAINS,
  MAX_OUTPUT_TOKENS,
} from "../_shared/limits.ts";

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type',
};

Deno.serve(async (req) => {
  if (req.method === 'OPTIONS') {
    return new Response('ok', { headers: corsHeaders });
  }

  const version = new URL(req.url).searchParams.get('v');

  const body = version === '2'
    ? {
        pro_models: PRO_MODELS,
        free_model: FREE_MODEL,
        byo_model: BYO_MODEL,
        default_pro_model: DEFAULT_PRO_MODEL,
        free_call_limit: FREE_LIFETIME_CALL_LIMIT,
        pro_request_limit: PRO_MONTHLY_REQUEST_LIMIT,
        pro_audio_seconds_limit: PRO_MONTHLY_AUDIO_SECONDS,
        audio_session_max_seconds: PRO_AUDIO_SESSION_MAX_SECONDS,
        free_allowed_domains: FREE_TIER_ALLOWED_DOMAINS,
        max_output_tokens: MAX_OUTPUT_TOKENS,
      }
    : {
        pro_models: PRO_MODELS,
        free_model: LEGACY_FREE_MODEL,
        default_pro_model: DEFAULT_PRO_MODEL,
      };

  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { ...corsHeaders, 'Content-Type': 'application/json' },
  });
});
