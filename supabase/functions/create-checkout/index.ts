// Supabase Edge Function: Create Stripe Checkout Session (PRODUCTION)

import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type',
  'Access-Control-Allow-Methods': 'POST, OPTIONS',
};

Deno.serve(async (req) => {
  // Handle CORS preflight request
  if (req.method === 'OPTIONS') {
    return new Response(null, { status: 204, headers: corsHeaders });
  }

  try {
    // Validate request body
    const rawBody = await req.text();
    if (!rawBody || rawBody.trim() === '') {
      throw new Error('Request body is empty');
    }

    const { userId, userEmail } = JSON.parse(rawBody);
    
    if (!userId || !userEmail) {
      throw new Error(`Missing required fields. userId: ${userId}, userEmail: ${userEmail}`);
    }

    console.log(`[Checkout] Creating session for user: ${userId}`);

    const supabase = createClient(
      Deno.env.get("SUPABASE_URL") ?? "",
      Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? ""
    );

    const stripeSecretKey = Deno.env.get("STRIPE_SECRET_KEY");
    const stripePriceId = Deno.env.get("STRIPE_PRICE_ID");

    if (!stripeSecretKey) {
      throw new Error('STRIPE_SECRET_KEY not configured');
    }
    if (!stripePriceId) {
      throw new Error('STRIPE_PRICE_ID not configured');
    }

    // Check if customer already exists in database
    let customerId: string | undefined;

    const { data: userData, error: userError } = await supabase
      .from("users")
      .select("stripe_customer_id")
      .eq("id", userId)
      .single();

    if (userError) {
      console.log(`[Checkout] User lookup error: ${userError.message}`);
    }

    if (userData?.stripe_customer_id) {
      customerId = userData.stripe_customer_id;
      console.log(`[Checkout] Existing customer: ${customerId}`);
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
        console.error(`[Checkout] Failed to save customer ID: ${updateError.message}`);
      }

      console.log(`[Checkout] New customer: ${customerId}`);
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

    console.log(`[Checkout] Session created: ${session.id}`);
    console.log(`[Checkout] Checkout URL: ${session.url}`);

    return new Response(JSON.stringify({ url: session.url }), {
      status: 200,
      headers: { ...corsHeaders, "Content-Type": "application/json" },
    });
  } catch (err: any) {
    console.error(`[Checkout Error]`, err?.message ?? err);
    return new Response(
      JSON.stringify({ error: err?.message ?? "Unknown error" }),
      {
        status: 400,
        headers: { ...corsHeaders, "Content-Type": "application/json" },
      }
    );
  }
});