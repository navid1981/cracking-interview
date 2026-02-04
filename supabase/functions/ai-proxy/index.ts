/**
 * ai-proxy Edge Function
 * 
 * Proxies AI requests to OpenRouter with quota enforcement.
 * 
 * Features:
 * - Authenticates requests via Supabase JWT
 * - Checks user subscription status
 * - Enforces monthly quota (150 requests for paid users)
 * - Enforces lifetime quota (3 calls for free users)
 * - Free users can only use Grok Code Fast model
 * - Logs usage to api_usage table
 * - Supports streaming responses
 */

// Direct import for Supabase Dashboard deployment
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type',
};

// Monthly request limit for paid subscribers
const MONTHLY_REQUEST_LIMIT = 150;
// Lifetime limit for free users
const FREE_LIFETIME_LIMIT = 3;

// Free tier model - using Grok Code Fast for better latency
const FREE_MODEL = 'gemini-2.5-flash';
const FREE_MODEL_OPENROUTER = 'google/gemini-2.5-flash';

// Pro models available for paid subscribers
const PRO_MODELS = ['gpt-5.2-codex', 'claude-sonnet-4.5', 'gemini-3-flash', 'grok-4.1-fast'];

// Timeout for OpenRouter API calls (30 seconds)
const API_TIMEOUT_MS = 30000;

// Allowed domains for free tier users
const FREE_TIER_ALLOWED_DOMAINS = ['leetcode.com', 'codewars.com', 'codeforces.com', 'neetcode.io'];

interface AIRequest {
  model: string;
  source_url?: string;  // URL of the source tab for domain validation
  messages: Array<{
    role: 'user' | 'assistant' | 'system';
    content: string | Array<{ type: string; text?: string; image_url?: { url: string } }>;
  }>;
  stream?: boolean;
  max_tokens?: number;
}

interface UserSubscription {
  id: string;
  subscription_status: 'active' | 'inactive' | 'cancelled' | 'cancelling' | null;
  lifetime_ai_calls: number;
  subscription_start_date: string | null;
  subscription_end_date: string | null;
}

Deno.serve(async (req) => {
  console.log('[ai-proxy] ========== REQUEST START ==========');
  console.log('[ai-proxy] Method:', req.method);
  console.log('[ai-proxy] URL:', req.url);

  // Handle CORS preflight
  if (req.method === 'OPTIONS') {
    console.log('[ai-proxy] CORS preflight request');
    return new Response(null, { headers: corsHeaders });
  }

  try {
    // Get authorization header
    const authHeader = req.headers.get('Authorization');
    console.log('[ai-proxy] Auth header present:', !!authHeader);
    
    if (!authHeader) {
      console.log('[ai-proxy] ERROR: Missing authorization header');
      return new Response(
        JSON.stringify({ error: 'Missing authorization header' }),
        { status: 401, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    // Initialize Supabase client
    const supabaseUrl = Deno.env.get('SUPABASE_URL')!;
    const supabaseServiceKey = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')!;
    console.log('[ai-proxy] Supabase URL configured:', !!supabaseUrl);
    console.log('[ai-proxy] Service key configured:', !!supabaseServiceKey);
    
    const supabase = createClient(supabaseUrl, supabaseServiceKey);

    // Verify JWT and get user
    const token = authHeader.replace('Bearer ', '');
    console.log('[ai-proxy] Token length:', token.length);
    
    const { data: { user }, error: authError } = await supabase.auth.getUser(token);
    console.log('[ai-proxy] User found:', !!user);
    console.log('[ai-proxy] Auth error:', authError?.message || 'none');

    if (authError || !user) {
      console.log('[ai-proxy] ERROR: Invalid or expired token');
      return new Response(
        JSON.stringify({ error: 'Invalid or expired token' }),
        { status: 401, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    console.log('[ai-proxy] User ID:', user.id);
    console.log('[ai-proxy] User email:', user.email);

    // Get user subscription status
    const { data: subscription, error: subError } = await supabase
      .from('users')
      .select('id, subscription_status, lifetime_ai_calls, subscription_start_date, subscription_end_date')
      .eq('id', user.id)
      .single();

    console.log('[ai-proxy] Subscription found:', !!subscription);
    console.log('[ai-proxy] Subscription error:', subError?.message || 'none');

    if (subError || !subscription) {
      console.log('[ai-proxy] Creating new user record...');
      // User exists in auth but not in users table - create entry
      const { error: insertError } = await supabase
        .from('users')
        .insert({ id: user.id, email: user.email, lifetime_ai_calls: 0 });
      
      if (insertError) {
        console.error('[ai-proxy] Failed to create user record:', insertError);
      } else {
        console.log('[ai-proxy] User record created successfully');
      }

      // Treat as free user with 0 lifetime calls
      const newSubscription: UserSubscription = {
        id: user.id,
        subscription_status: null,
        lifetime_ai_calls: 0,
      };
      return await processRequest(req, user.id, newSubscription, supabase);
    }

    console.log('[ai-proxy] Subscription status:', subscription.subscription_status);
    console.log('[ai-proxy] Lifetime AI calls:', subscription.lifetime_ai_calls);
    
    return await processRequest(req, user.id, subscription as UserSubscription, supabase);

  } catch (error) {
    console.error('[ai-proxy] CRITICAL ERROR:', error);
    console.error('[ai-proxy] Error stack:', (error as Error).stack);
    return new Response(
      JSON.stringify({ error: `Server error: ${(error as Error).message}` }),
      { status: 500, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    );
  }
});

async function processRequest(
  req: Request,
  userId: string,
  subscription: UserSubscription,
  supabase: ReturnType<typeof createClient>
): Promise<Response> {
  console.log('[ai-proxy] ========== PROCESS REQUEST ==========');
  
  // Include 'cancelling' as paid - they still have access until period ends
  const isPaid = subscription.subscription_status === 'active' || subscription.subscription_status === 'cancelling';
  const lifetimeCalls = subscription.lifetime_ai_calls || 0;
  
  console.log('[ai-proxy] isPaid:', isPaid);
  console.log('[ai-proxy] lifetimeCalls:', lifetimeCalls);

  // Parse request body with timeout
  console.log('[ai-proxy] Parsing request body...');
  let body: AIRequest;
  try {
    // Clone request to avoid body consumption issues
    const bodyText = await req.text();
    console.log('[ai-proxy] Body size:', bodyText.length, 'chars');
    body = JSON.parse(bodyText);
  } catch (parseError) {
    console.error('[ai-proxy] Body parse error:', parseError);
    return new Response(
      JSON.stringify({ error: `Failed to parse request body: ${(parseError as Error).message}` }),
      { status: 400, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    );
  }
  let { model, messages, stream = false, max_tokens = 4096, source_url } = body;
  
  console.log('[ai-proxy] Requested model:', model);
  console.log('[ai-proxy] Messages count:', messages?.length);
  console.log('[ai-proxy] Stream:', stream);
  console.log('[ai-proxy] Max tokens:', max_tokens);
  console.log('[ai-proxy] Source URL:', source_url || 'not provided');

  // Free users - validate domain restriction
  if (!isPaid && source_url) {
    const isAllowedDomain = FREE_TIER_ALLOWED_DOMAINS.some(domain => source_url.includes(domain));
    if (!isAllowedDomain) {
      console.log(`[ai-proxy] Free user attempted to use blocked domain: ${source_url}`);
      return new Response(
        JSON.stringify({ 
          error: `Domain restriction: Free tier only works on coding practice sites (${FREE_TIER_ALLOWED_DOMAINS.join(', ')}). Upgrade to Pro for unlimited access.`
        }),
        { status: 403, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }
    console.log('[ai-proxy] Domain validation passed for free user');
  }

  // Free users can only use free model
  if (!isPaid) {
    if (model !== FREE_MODEL) {
      console.log(`[ai-proxy] Free user attempted to use ${model}, forcing to ${FREE_MODEL}`);
      model = FREE_MODEL;
    }
  } else {
    // Pro users can only use pro models
    if (!PRO_MODELS.includes(model)) {
      console.log(`[ai-proxy] Unknown model ${model}, defaulting to gpt-5.2-codex`);
      model = 'gpt-5.2-codex';
    }
  }

  // Check quota
  if (isPaid) {
    // Paid user - check quota for current billing period (subscription_start_date to subscription_end_date)
    const periodStart = subscription.subscription_start_date 
      ? new Date(subscription.subscription_start_date)
      : new Date('2020-01-01'); // Fallback to all-time if no start date
    const periodEnd = subscription.subscription_end_date
      ? new Date(subscription.subscription_end_date)
      : new Date('2099-12-31'); // Fallback to far future if no end date
    
    console.log(`[ai-proxy] Checking quota from: ${periodStart.toISOString()} to ${periodEnd.toISOString()}`);
    
    const { count, error: countError } = await supabase
      .from('api_usage')
      .select('*', { count: 'exact', head: true })
      .eq('user_id', userId)
      .gte('created_at', periodStart.toISOString())
      .lt('created_at', periodEnd.toISOString());

    if (countError) {
      console.error('Failed to check usage:', countError);
    }

    const usedThisPeriod = count || 0;
    console.log(`[ai-proxy] Used this billing period: ${usedThisPeriod}/${MONTHLY_REQUEST_LIMIT}`);
    
    if (usedThisPeriod >= MONTHLY_REQUEST_LIMIT) {
      return new Response(
        JSON.stringify({ 
          error: `Billing period quota exceeded (${MONTHLY_REQUEST_LIMIT} requests). Resets on ${periodEnd.toLocaleDateString()}.`,
          usage: {
            requests_used: usedThisPeriod,
            requests_limit: MONTHLY_REQUEST_LIMIT,
            is_paid: true,
          }
        }),
        { status: 429, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }
  } else {
    // Free user - check lifetime quota
    if (lifetimeCalls >= FREE_LIFETIME_LIMIT) {
      return new Response(
        JSON.stringify({ 
          error: `Free trial expired (${FREE_LIFETIME_LIMIT} lifetime calls used). Subscribe to continue using AI features, or configure your own API key.`,
          usage: {
            requests_used: lifetimeCalls,
            requests_limit: FREE_LIFETIME_LIMIT,
            is_paid: false,
          }
        }),
        { status: 429, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }
  }

  // Get OpenRouter API key
  const openrouterKey = Deno.env.get('OPENROUTER_API_KEY');
  console.log('[ai-proxy] OpenRouter API key configured:', !!openrouterKey);
  console.log('[ai-proxy] OpenRouter API key length:', openrouterKey?.length || 0);
  
  if (!openrouterKey) {
    console.log('[ai-proxy] ERROR: OpenRouter API key not configured');
    return new Response(
      JSON.stringify({ error: 'OpenRouter API key not configured' }),
      { status: 500, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    );
  }

  // Map model names to OpenRouter format
  const modelMap: Record<string, string> = {
    // Pro models
    'gpt-5.2-codex': 'openai/gpt-5.2-codex',
    'claude-sonnet-4.5': 'anthropic/claude-sonnet-4.5',
    'gemini-3-flash': 'google/gemini-3-flash-preview',
    'grok-4.1-fast': 'x-ai/grok-4.1-fast',
    // Free model
    'gemini-2.5-flash': FREE_MODEL_OPENROUTER,
  };

  const openrouterModel = modelMap[model] || model;
  console.log(`[ai-proxy] Using model: ${model} -> ${openrouterModel}`);

  // Call OpenRouter API with timeout
  console.log('[ai-proxy] ========== CALLING OPENROUTER ==========');
  console.log('[ai-proxy] Timeout:', API_TIMEOUT_MS, 'ms');
  const startTime = Date.now();
  
  const controller = new AbortController();
  const timeoutId = setTimeout(() => {
    console.log('[ai-proxy] TIMEOUT TRIGGERED after', API_TIMEOUT_MS, 'ms');
    controller.abort();
  }, API_TIMEOUT_MS);

  let openrouterResponse: Response;
  try {
    console.log('[ai-proxy] Sending request to OpenRouter...');
    openrouterResponse = await fetch('https://openrouter.ai/api/v1/chat/completions', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${openrouterKey}`,
        'Content-Type': 'application/json',
        'HTTP-Referer': 'https://crackinginterview.org',
        'X-Title': 'CrackingInterview',
      },
      body: JSON.stringify({
        model: openrouterModel,
        messages,
        max_tokens,
        stream,
        provider: {
          sort: 'throughput',  // Prioritize fastest provider
        },
      }),
      signal: controller.signal,
    });
    console.log('[ai-proxy] OpenRouter response received in', Date.now() - startTime, 'ms');
    console.log('[ai-proxy] OpenRouter status:', openrouterResponse.status);
  } catch (fetchError) {
    clearTimeout(timeoutId);
    const elapsed = Date.now() - startTime;
    console.error('[ai-proxy] Fetch error after', elapsed, 'ms:', fetchError);
    console.error('[ai-proxy] Error name:', (fetchError as Error).name);
    console.error('[ai-proxy] Error message:', (fetchError as Error).message);
    
    if ((fetchError as Error).name === 'AbortError') {
      console.error('[ai-proxy] Request timed out after 30 seconds');
      return new Response(
        JSON.stringify({ error: 'AI request timed out after 30 seconds. Please try again.' }),
        { status: 504, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }
    throw fetchError;
  } finally {
    clearTimeout(timeoutId);
  }

  if (!openrouterResponse.ok) {
    const errorText = await openrouterResponse.text();
    console.error('[ai-proxy] OpenRouter error status:', openrouterResponse.status);
    console.error('[ai-proxy] OpenRouter error body:', errorText);
    return new Response(
      JSON.stringify({ error: `AI provider error: ${errorText}` }),
      { status: openrouterResponse.status, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    );
  }

  console.log('[ai-proxy] OpenRouter call successful!');

  // Log usage
  console.log('[ai-proxy] Logging usage...');
  const { error: logError } = await supabase
    .from('api_usage')
    .insert({
      user_id: userId,
      ai_model: model,
      tokens_used: 0, // We don't have exact count yet; could parse from response
      request_count: 1,
    });

  if (logError) {
    console.error('[ai-proxy] Failed to log usage:', logError);
  } else {
    console.log('[ai-proxy] Usage logged successfully');
  }

  // Update lifetime_ai_calls for free users
  if (!isPaid) {
    console.log('[ai-proxy] Updating lifetime calls for free user...');
    const { error: updateError } = await supabase
      .from('users')
      .update({ lifetime_ai_calls: lifetimeCalls + 1 })
      .eq('id', userId);

    if (updateError) {
      console.error('[ai-proxy] Failed to update lifetime calls:', updateError);
    } else {
      console.log('[ai-proxy] Lifetime calls updated to:', lifetimeCalls + 1);
    }
  }

  // Handle streaming response
  if (stream) {
    console.log('[ai-proxy] Returning streaming response');
    return new Response(openrouterResponse.body, {
      headers: {
        ...corsHeaders,
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive',
      },
    });
  }

  // Non-streaming response
  console.log('[ai-proxy] Parsing non-streaming response...');
  const responseData = await openrouterResponse.json();
  console.log('[ai-proxy] Response has choices:', !!responseData.choices);
  console.log('[ai-proxy] Choices count:', responseData.choices?.length || 0);

  // Calculate usage info for response - use billing period dates (same as quota check)
  let usageInfo;
  if (isPaid) {
    // Use subscription dates for billing period (consistent with quota check above)
    const periodStart = subscription.subscription_start_date 
      ? new Date(subscription.subscription_start_date)
      : new Date('2020-01-01');
    const periodEnd = subscription.subscription_end_date
      ? new Date(subscription.subscription_end_date)
      : new Date('2099-12-31');

    const { count } = await supabase
      .from('api_usage')
      .select('*', { count: 'exact', head: true })
      .eq('user_id', userId)
      .gte('created_at', periodStart.toISOString())
      .lt('created_at', periodEnd.toISOString());

    usageInfo = {
      requests_used: (count || 0),
      requests_limit: MONTHLY_REQUEST_LIMIT,
      period_end: periodEnd.toISOString(),
      is_paid: true,
    };
    console.log(`[ai-proxy] Usage for response: ${usageInfo.requests_used}/${MONTHLY_REQUEST_LIMIT}`);
  } else {
    usageInfo = {
      requests_used: lifetimeCalls + 1,
      requests_limit: FREE_LIFETIME_LIMIT,
      is_paid: false,
    };
  }

  // Extract the response text
  const responseText = responseData.choices?.[0]?.message?.content || '';
  console.log('[ai-proxy] Response text length:', responseText.length);

  console.log('[ai-proxy] ========== REQUEST COMPLETE ==========');
  console.log('[ai-proxy] Returning successful response');
  
  return new Response(
    JSON.stringify({
      response: responseText,
      usage: usageInfo,
      raw: responseData, // Include raw response for debugging
    }),
    { headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
  );
}
