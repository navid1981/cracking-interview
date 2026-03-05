/**
 * log-audio-usage Edge Function
 *
 * Logs the duration of a completed audio recording session for the
 * authenticated Pro user.  Returns the updated cumulative total so the
 * frontend can refresh its usage display without an extra round-trip.
 */

import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type',
};

const MONTHLY_AUDIO_LIMIT = 36000; // 10 hours in seconds
const MAX_SESSION_SECONDS = 5400;  // 90 minutes cap per session

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

    // Parse request body
    const body = await req.json().catch(() => null);
    const durationSeconds = body?.duration_seconds;

    if (typeof durationSeconds !== 'number' || durationSeconds <= 0) {
      return new Response(
        JSON.stringify({ error: 'duration_seconds must be a positive number' }),
        { status: 400, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    // Server-side cap: never log more than 90 minutes per session
    const cappedDuration = Math.min(Math.round(durationSeconds), MAX_SESSION_SECONDS);

    // Verify Pro subscription
    const { data: subscription } = await supabase
      .from('users')
      .select('subscription_status, subscription_start_date, subscription_end_date')
      .eq('id', user.id)
      .single();

    const isPro = subscription?.subscription_status === 'active' ||
                  subscription?.subscription_status === 'cancelling';

    if (!isPro) {
      return new Response(
        JSON.stringify({ error: 'Pro subscription required' }),
        { status: 403, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    // Insert usage record
    const { error: insertError } = await supabase
      .from('audio_usage')
      .insert({
        user_id: user.id,
        duration_seconds: cappedDuration,
      });

    if (insertError) {
      console.error('[log-audio-usage] Insert error:', insertError);
      return new Response(
        JSON.stringify({ error: 'Failed to log audio usage' }),
        { status: 500, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      );
    }

    // Compute updated total for current billing period
    const periodStart = subscription.subscription_start_date
      ? new Date(subscription.subscription_start_date)
      : new Date('2020-01-01');
    const periodEnd = subscription.subscription_end_date
      ? new Date(subscription.subscription_end_date)
      : new Date('2099-12-31');

    const { data: sumData, error: sumError } = await supabase
      .rpc('sum_audio_seconds', {
        p_user_id: user.id,
        p_start: periodStart.toISOString(),
        p_end: periodEnd.toISOString(),
      });

    let audioSecondsUsed = cappedDuration; // fallback
    if (!sumError && sumData !== null) {
      audioSecondsUsed = sumData;
    } else {
      // Fallback: manual query
      const { data: rows } = await supabase
        .from('audio_usage')
        .select('duration_seconds')
        .eq('user_id', user.id)
        .gte('created_at', periodStart.toISOString())
        .lt('created_at', periodEnd.toISOString());

      if (rows) {
        audioSecondsUsed = rows.reduce((sum: number, r: { duration_seconds: number }) => sum + r.duration_seconds, 0);
      }
    }

    return new Response(
      JSON.stringify({
        audio_seconds_used: audioSecondsUsed,
        audio_seconds_limit: MONTHLY_AUDIO_LIMIT,
      }),
      { status: 200, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    );

  } catch (error) {
    console.error('[log-audio-usage] Error:', error);
    return new Response(
      JSON.stringify({ error: 'Internal server error' }),
      { status: 500, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    );
  }
});
