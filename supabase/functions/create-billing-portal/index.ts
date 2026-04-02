// Supabase Edge Function: Create Stripe Billing Portal Session

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

    const { customerId } = JSON.parse(rawBody);
    
    if (!customerId) {
      throw new Error(`Missing required field: customerId`);
    }

    console.log(`[Billing Portal] Creating session for customer: ${customerId}`);

    // Use test key for now - change to STRIPE_SECRET_KEY for production
    const stripeSecretKey = Deno.env.get("STRIPE_SECRET_KEY");

    if (!stripeSecretKey) {
      throw new Error('STRIPE_SECRET_KEY not configured');
    }

    // Create billing portal session using Stripe REST API
    const portalResponse = await fetch(
      "https://api.stripe.com/v1/billing_portal/sessions",
      {
        method: "POST",
        headers: {
          "Authorization": `Bearer ${stripeSecretKey}`,
          "Content-Type": "application/x-www-form-urlencoded",
        },
        body: new URLSearchParams({
          customer: customerId,
          return_url: "https://crackinginterview.org/",
        }).toString(),
      }
    );

    if (!portalResponse.ok) {
      const error = await portalResponse.text();
      throw new Error(`Failed to create portal session: ${error}`);
    }

    const portalSession = await portalResponse.json();

    console.log(`[Billing Portal] Session created: ${portalSession.id}`);
    console.log(`[Billing Portal] Portal URL: ${portalSession.url}`);

    return new Response(JSON.stringify({ url: portalSession.url }), {
      status: 200,
      headers: { ...corsHeaders, "Content-Type": "application/json" },
    });
  } catch (err: any) {
    console.error(`[Billing Portal Error]`, err?.message ?? err);
    return new Response(
      JSON.stringify({ error: err?.message ?? "Unknown error" }),
      {
        status: 400,
        headers: { ...corsHeaders, "Content-Type": "application/json" },
      }
    );
  }
});

