/**
 * ai-proxy Edge Function
 * 
 * Proxies AI requests to OpenRouter with quota enforcement.
 * 
 * Features:
 * - Authenticates requests via Supabase JWT
 * - Checks user subscription status
 * - Enforces monthly quota (150 requests for paid users)
 * - Enforces lifetime quota (2 calls for free users)
 * - Logs usage to api_usage table
 * - Supports streaming responses
 */

// Use import map from deno.json (no external URLs needed)
import { createClient } from "@supabase/supabase-js";

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type',
};

// Monthly request limit for paid subscribers
const MONTHLY_REQUEST_LIMIT = 150;
// Lifetime limit for free users
const FREE_LIFETIME_LIMIT = 2;

interface AIRequest {
  model: string;
  messages: Array<{
    role: 'user' | 'assistant' | 'system';
    content: string | Array<{ type: string; text?: string; image_url?: { url: string } }>;
  }>;
  stream?: boolean;
  max_tokens?: number;
}

interface UserSubscription {
  id: string;
  subscription_status: 'active' | 'inactive' | 'cancelled' | null;
  lifetime_ai_calls: number;
}

Deno.serve(async (req) => {
  // Handle CORS preflight
  if (req.method === 'OPTIONS') {
    return new Response(null, { headers: corsHeaders });
  }

  try {
    // Get authorization header
    const authHeader = req.headers.get('Authorization');
    if (!authHeader) {
      return new Response(
        JSON.stringify({ error: 'Missing authorization header' }),
        { status: 401, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    // Initialize Supabase client
    const supabaseUrl = Deno.env.get('SUPABASE_URL')!;
    const supabaseServiceKey = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')!;
    const supabase = createClient(supabaseUrl, supabaseServiceKey);

    // Verify JWT and get user
    const token = authHeader.replace('Bearer ', '');
    const { data: { user }, error: authError } = await supabase.auth.getUser(token);

    if (authError || !user) {
      return new Response(
        JSON.stringify({ error: 'Invalid or expired token' }),
        { status: 401, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    // Get user subscription status
    const { data: subscription, error: subError } = await supabase
      .from('users')
      .select('id, subscription_status, lifetime_ai_calls')
      .eq('id', user.id)
      .single();

    if (subError || !subscription) {
      // User exists in auth but not in users table - create entry
      const { error: insertError } = await supabase
        .from('users')
        .insert({ id: user.id, email: user.email, lifetime_ai_calls: 0 });
      
      if (insertError) {
        console.error('Failed to create user record:', insertError);
      }

      // Treat as free user with 0 lifetime calls
      const newSubscription: UserSubscription = {
        id: user.id,
        subscription_status: null,
        lifetime_ai_calls: 0,
      };
      return await processRequest(req, user.id, newSubscription, supabase);
    }

    return await processRequest(req, user.id, subscription as UserSubscription, supabase);

  } catch (error) {
    console.error('AI Proxy error:', error);
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
  const isPaid = subscription.subscription_status === 'active';
  const lifetimeCalls = subscription.lifetime_ai_calls || 0;

  // Parse request body
  const body: AIRequest = await req.json();
  const { model, messages, stream = false, max_tokens = 4096 } = body;

  // Check quota
  if (isPaid) {
    // Paid user - check monthly quota
    const now = new Date();
    const monthStart = new Date(now.getFullYear(), now.getMonth(), 1);
    
    const { count, error: countError } = await supabase
      .from('api_usage')
      .select('*', { count: 'exact', head: true })
      .eq('user_id', userId)
      .gte('created_at', monthStart.toISOString());

    if (countError) {
      console.error('Failed to check usage:', countError);
    }

    const usedThisMonth = count || 0;
    
    if (usedThisMonth >= MONTHLY_REQUEST_LIMIT) {
      const nextMonth = new Date(now.getFullYear(), now.getMonth() + 1, 1);
      return new Response(
        JSON.stringify({ 
          error: `Monthly quota exceeded (${MONTHLY_REQUEST_LIMIT} requests). Resets on ${nextMonth.toLocaleDateString()}.`,
          usage: {
            requests_used: usedThisMonth,
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
  if (!openrouterKey) {
    return new Response(
      JSON.stringify({ error: 'OpenRouter API key not configured' }),
      { status: 500, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    );
  }

  // Map model names to OpenRouter format
  const modelMap: Record<string, string> = {
    'claude-sonnet-4-20250514': 'anthropic/claude-sonnet-4',
    'claude-3-5-haiku-20241022': 'anthropic/claude-3.5-haiku',
    'gemini-2.5-flash': 'google/gemini-2.5-flash-preview',
  };

  const openrouterModel = modelMap[model] || model;

  // Call OpenRouter API
  const openrouterResponse = await fetch('https://openrouter.ai/api/v1/chat/completions', {
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
    }),
  });

  if (!openrouterResponse.ok) {
    const errorText = await openrouterResponse.text();
    console.error('OpenRouter error:', errorText);
    return new Response(
      JSON.stringify({ error: `AI provider error: ${errorText}` }),
      { status: openrouterResponse.status, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    );
  }

  // Log usage
  const { error: logError } = await supabase
    .from('api_usage')
    .insert({
      user_id: userId,
      ai_model: model,
      tokens_used: 0, // We don't have exact count yet; could parse from response
      request_count: 1,
    });

  if (logError) {
    console.error('Failed to log usage:', logError);
  }

  // Update lifetime_ai_calls for free users
  if (!isPaid) {
    const { error: updateError } = await supabase
      .from('users')
      .update({ lifetime_ai_calls: lifetimeCalls + 1 })
      .eq('id', userId);

    if (updateError) {
      console.error('Failed to update lifetime calls:', updateError);
    }
  }

  // Handle streaming response
  if (stream) {
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
  const responseData = await openrouterResponse.json();

  // Calculate usage info for response
  const now = new Date();
  const monthStart = new Date(now.getFullYear(), now.getMonth(), 1);
  const monthEnd = new Date(now.getFullYear(), now.getMonth() + 1, 1);

  let usageInfo;
  if (isPaid) {
    const { count } = await supabase
      .from('api_usage')
      .select('*', { count: 'exact', head: true })
      .eq('user_id', userId)
      .gte('created_at', monthStart.toISOString());

    usageInfo = {
      requests_used: (count || 0),
      requests_limit: MONTHLY_REQUEST_LIMIT,
      period_end: monthEnd.toISOString(),
      is_paid: true,
    };
  } else {
    usageInfo = {
      requests_used: lifetimeCalls + 1,
      requests_limit: FREE_LIFETIME_LIMIT,
      is_paid: false,
    };
  }

  // Extract the response text
  const responseText = responseData.choices?.[0]?.message?.content || '';

  return new Response(
    JSON.stringify({
      response: responseText,
      usage: usageInfo,
      raw: responseData, // Include raw response for debugging
    }),
    { headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
  );
}
