/**
 * deepgram-key Edge Function
 *
 * Securely provides the Deepgram API key to authenticated Pro users.
 * The key is stored as a Supabase secret (DEEPGRAM_API_KEY) and never
 * embedded in the app binary.
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

    // Check subscription — only Pro users (active or cancelling) get the key
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

    return new Response(
      JSON.stringify({ key: deepgramKey }),
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
