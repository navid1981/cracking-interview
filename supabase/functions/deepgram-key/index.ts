/**
 * deepgram-key Edge Function
 *
 * Generates a temporary Deepgram JWT (30s TTL) for authenticated Pro users.
 * The permanent API key never leaves the server — only a short-lived token
 * is returned. The token expires after 30 seconds but an already-opened
 * WebSocket connection stays alive beyond the TTL.
 *
 * @see https://developers.deepgram.com/guides/fundamentals/token-based-authentication
 */

import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type',
};

Deno.serve(async (req) => {
  if (req.method === 'OPTIONS') {
    return new Response(null, { headers: corsHeaders });
  }

  try {
    const authHeader = req.headers.get('Authorization');
    if (!authHeader) {
      return new Response(
        JSON.stringify({ error: 'Missing authorization header' }),
        { status: 401, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    const supabaseUrl = Deno.env.get('SUPABASE_URL')!;
    const supabaseServiceKey = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')!;
    const supabase = createClient(supabaseUrl, supabaseServiceKey);

    const token = authHeader.replace('Bearer ', '');
    const { data: { user }, error: authError } = await supabase.auth.getUser(token);

    if (authError || !user) {
      return new Response(
        JSON.stringify({ error: 'Invalid or expired token' }),
        { status: 401, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    const { data: subscription } = await supabase
      .from('users')
      .select('subscription_status')
      .eq('id', user.id)
      .single();

    const isPro = subscription?.subscription_status === 'active' ||
                  subscription?.subscription_status === 'cancelling';

    if (!isPro) {
      return new Response(
        JSON.stringify({ error: 'Pro subscription required for live transcription' }),
        { status: 403, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    const deepgramKey = Deno.env.get('DEEPGRAM_API_KEY');
    if (!deepgramKey) {
      console.error('[deepgram-key] DEEPGRAM_API_KEY secret not configured');
      return new Response(
        JSON.stringify({ error: 'Transcription service not configured' }),
        { status: 500, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    // Request a temporary JWT from Deepgram (default 30s TTL).
    // The WebSocket only needs the token to be valid during the handshake;
    // the connection stays open beyond the TTL.
    const grantResp = await fetch('https://api.deepgram.com/v1/auth/grant', {
      method: 'POST',
      headers: {
        'Authorization': `Token ${deepgramKey}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ ttl_seconds: 3600 }),
    });

    if (!grantResp.ok) {
      const errText = await grantResp.text();
      console.error('[deepgram-key] Deepgram /auth/grant failed:', grantResp.status, errText);
      return new Response(
        JSON.stringify({ error: 'Failed to generate transcription token' }),
        { status: 502, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    const grantData = await grantResp.json();

    return new Response(
      JSON.stringify({ key: grantData.access_token }),
      { status: 200, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    );

  } catch (error) {
    console.error('[deepgram-key] Error:', error);
    return new Response(
      JSON.stringify({ error: 'Internal server error' }),
      { status: 500, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    );
  }
});
