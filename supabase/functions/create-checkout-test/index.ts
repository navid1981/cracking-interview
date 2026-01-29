// Supabase Edge Function: Create Stripe Checkout Session (TEST MODE)
// With CORS support and extra debugging

import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type',
  'Access-Control-Allow-Methods': 'POST, OPTIONS',
};

Deno.serve(async (req) => {
  console.log(`[Checkout TEST] Method: ${req.method}`);
  
  // Handle CORS preflight request
  if (req.method === 'OPTIONS') {
    console.log(`[Checkout TEST] Handling OPTIONS preflight`);
    return new Response(null, { status: 204, headers: corsHeaders });
  }

  try {
    // Read raw body first for debugging
    const rawBody = await req.text();
    console.log(`[Checkout TEST] Raw body: ${rawBody}`);
    
    if (!rawBody || rawBody.trim() === '') {
      throw new Error('Request body is empty');
    }

    const { userId, userEmail } = JSON.parse(rawBody);
    
    if (!userId || !userEmail) {
      throw new Error(`Missing required fields. userId: ${userId}, userEmail: ${userEmail}`);
    }

    console.log(`[Checkout TEST] Creating session for user: ${userId}, email: ${userEmail}`);

    const supabase = createClient(
      Deno.env.get("SUPABASE_URL") ?? "",
      Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? ""
    );

    const stripeSecretKey = Deno.env.get("STRIPE_SECRET_KEY_TEST");
    const stripePriceId = Deno.env.get("STRIPE_PRICE_ID_TEST");
    
    if (!stripeSecretKey) {
      throw new Error('STRIPE_SECRET_KEY_TEST not configured');
    }
    if (!stripePriceId) {
      throw new Error('STRIPE_PRICE_ID_TEST not configured');
    }

    // Check if customer already exists in database
    let customerId: string | undefined;

    const { data: userData, error: userError } = await supabase
      .from("users")
      .select("stripe_customer_id")
      .eq("id", userId)
      .single();

    if (userError) {
      console.log(`[Checkout TEST] User lookup error: ${userError.message}`);
    }

    if (userData?.stripe_customer_id) {
      customerId = userData.stripe_customer_id;
      console.log(`[Checkout TEST] Existing customer: ${customerId}`);
    } else {
      // Create new Stripe customer using REST API
      const customerResponse = await fetch("https://api.stripe.com/v1/customers", {
        method: "POST",
        headers: {
          "Authorization": `Bearer ${stripeSecretKey}`,
          "Content-Type": "application/x-www-form-urlencoded",
        },
        body: new URLSearchParams({
          email: userEmail,
          "metadata[supabase_user_id]": userId,
        }).toString(),
      });

      if (!customerResponse.ok) {
        const error = await customerResponse.text();
        throw new Error(`Failed to create customer: ${error}`);
      }

      const customer = await customerResponse.json();
      customerId = customer.id;

      // Save customer ID to database
      const { error: updateError } = await supabase
        .from("users")
        .update({ stripe_customer_id: customerId })
        .eq("id", userId);

      if (updateError) {
        console.error(`[Checkout TEST] Failed to save customer ID: ${updateError.message}`);
      }

      console.log(`[Checkout TEST] New customer: ${customerId}`);
    }

    // Create checkout session using Stripe REST API
    const sessionResponse = await fetch(
      "https://api.stripe.com/v1/checkout/sessions",
      {
        method: "POST",
        headers: {
          "Authorization": `Bearer ${stripeSecretKey}`,
          "Content-Type": "application/x-www-form-urlencoded",
        },
        body: new URLSearchParams({
          mode: "subscription",
          customer: customerId!,
          "payment_method_types[0]": "card",
          "line_items[0][price]": stripePriceId,
          "line_items[0][quantity]": "1",
          success_url: "https://crackinginterview.org/subscription/success?session_id={CHECKOUT_SESSION_ID}",
          cancel_url: "https://crackinginterview.org/subscription/cancelled",
          client_reference_id: userId,
          "metadata[supabase_user_id]": userId,
        }).toString(),
      }
    );

    if (!sessionResponse.ok) {
      const error = await sessionResponse.text();
      throw new Error(`Failed to create session: ${error}`);
    }

    const session = await sessionResponse.json();

    console.log(`[Checkout TEST] Session created: ${session.id}`);
    console.log(`[Checkout TEST] Checkout URL: ${session.url}`);

    return new Response(JSON.stringify({ url: session.url }), {
      status: 200,
      headers: { ...corsHeaders, "Content-Type": "application/json" },
    });
  } catch (err: any) {
    console.error(`[Checkout TEST Error]`, err?.message ?? err);
    return new Response(
      JSON.stringify({ error: err?.message ?? "Unknown error" }),
      {
        status: 400,
        headers: { ...corsHeaders, "Content-Type": "application/json" },
      }
    );
  }
});