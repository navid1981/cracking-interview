// Supabase Edge Function: Stripe Webhook Handler (TEST MODE)
// Handles subscription lifecycle events from Stripe

import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

// ============ CRYPTO UTILITIES ============

/** Convert ArrayBuffer to lowercase hex string */
function bufferToHex(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/** Timing-safe string comparison to prevent timing attacks */
function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let result = 0;
  for (let i = 0; i < a.length; i++) {
    result |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return result === 0;
}

/** Verify Stripe webhook signature using HMAC-SHA256 */
async function verifyStripeSignature(
  payload: string,
  header: string,
  secret: string,
  tolerance: number = 300 // 5 minutes
): Promise<boolean> {
  const parts = header.split(",");
  const timestampPart = parts.find((p) => p.startsWith("t="));
  const v1Part = parts.find((p) => p.startsWith("v1="));

  if (!timestampPart || !v1Part) {
    console.error("[Webhook TEST] Missing timestamp or signature in header");
    return false;
  }

  const timestamp = Number(timestampPart.split("=")[1]);
  const signature = v1Part.split("=")[1];

  if (!timestamp || !signature) {
    console.error("[Webhook TEST] Invalid timestamp or signature format");
    return false;
  }

  // Check timestamp tolerance (prevent replay attacks)
  const now = Math.floor(Date.now() / 1000);
  if (Math.abs(now - timestamp) > tolerance) {
    console.error("[Webhook TEST] Timestamp outside tolerance window");
    return false;
  }

  // Compute expected signature
  const signedPayload = `${timestamp}.${payload}`;
  const enc = new TextEncoder();
  const key = await crypto.subtle.importKey(
    "raw",
    enc.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const digestBuffer = await crypto.subtle.sign("HMAC", key, enc.encode(signedPayload));
  const expectedSignature = bufferToHex(digestBuffer);

  const valid = timingSafeEqual(expectedSignature, signature);
  if (!valid) {
    console.error("[Webhook TEST] Signature mismatch");
  }
  return valid;
}

// ============ MAIN HANDLER ============

Deno.serve(async (req) => {
  const signature = req.headers.get("stripe-signature");
  const webhookSecret = Deno.env.get("STRIPE_WEBHOOK_SECRET_TEST");

  if (!signature || !webhookSecret) {
    console.error("[Webhook TEST] Missing signature or webhook secret");
    return new Response(
      JSON.stringify({ error: "Missing signature or secret" }),
      { status: 400, headers: { "Content-Type": "application/json" } }
    );
  }

  const body = await req.text();

  try {
    // Verify webhook signature
    const isValid = await verifyStripeSignature(body, signature, webhookSecret);
    if (!isValid) {
      return new Response(
        JSON.stringify({ error: "Invalid signature" }),
        { status: 400, headers: { "Content-Type": "application/json" } }
      );
    }

    const event = JSON.parse(body);
    console.log(`[Webhook TEST] Event received: ${event.type}`);

    const supabase = createClient(
      Deno.env.get("SUPABASE_URL") ?? "",
      Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? ""
    );

    switch (event.type) {
      // ============ CHECKOUT COMPLETED ============
      // First event when user completes payment - saves customer ID
      case "checkout.session.completed": {
        const session = event.data.object as any;
        const customerId = session.customer;
        const userId = session.client_reference_id || session.metadata?.supabase_user_id;

        console.log(`[Webhook TEST] Checkout completed - customer: ${customerId}, user: ${userId}`);

        if (userId) {
          const { error } = await supabase
            .from("users")
            .update({
              stripe_customer_id: customerId,
              subscription_status: "active",
              subscription_tier: "pro",
            })
            .eq("id", userId);

          if (error) console.error("[Webhook TEST] Checkout update error:", error.message);
          else console.log("[Webhook TEST] User updated from checkout");
        }
        break;
      }

      // ============ SUBSCRIPTION CREATED/UPDATED ============
      // Main subscription events - includes period dates
      case "customer.subscription.created":
      case "customer.subscription.updated": {
        const subscription = event.data.object as any;
        const customerId = subscription.customer;

        console.log(`[Webhook TEST] Subscription ${event.type.split(".")[2]} - customer: ${customerId}`);
        console.log(`[Webhook TEST] Stripe status: ${subscription.status}`);
        console.log(`[Webhook TEST] cancel_at_period_end: ${subscription.cancel_at_period_end}, cancel_at: ${subscription.cancel_at}`);
        console.log(`[Webhook TEST] cancellation_details: ${JSON.stringify(subscription.cancellation_details)}`);

        // Extract period dates (handle different Stripe response formats)
        let rawStart = subscription.current_period_start;
        let rawEnd = subscription.current_period_end;

        if (!rawStart && subscription.items?.data?.[0]) {
          rawStart = subscription.items.data[0].current_period_start;
          rawEnd = subscription.items.data[0].current_period_end;
        }
        if (!rawStart && subscription.start_date) {
          rawStart = subscription.start_date;
        }

        const periodStart = Number(rawStart);
        const periodEnd = Number(rawEnd);

        // Determine our subscription_status based on Stripe status + cancellation indicators
        // - "active" = subscription is healthy and will renew
        // - "cancelling" = user cancelled but still has access until period end
        // - "past_due" = payment failed, needs attention
        // - "cancelled" = fully cancelled (no access)
        //
        // Stripe indicates pending cancellation in TWO ways:
        // 1. cancel_at_period_end = true (older method)
        // 2. cancel_at = <timestamp> (newer method - explicit cancellation date)
        let ourStatus = subscription.status;
        const hasPendingCancellation = 
          subscription.cancel_at_period_end === true || 
          subscription.cancel_at !== null ||
          subscription.cancellation_details?.reason === "cancellation_requested";
        
        if (subscription.status === "active" && hasPendingCancellation) {
          ourStatus = "cancelling"; // User cancelled but still has access
          console.log("[Webhook TEST] Detected pending cancellation - setting status to 'cancelling'");
        }

        // Build update payload
        const updatePayload: Record<string, any> = {
          subscription_status: ourStatus,
          subscription_tier: "pro", // Keep pro access until fully cancelled
          stripe_subscription_id: subscription.id,
          stripe_customer_id: customerId,
        };

        if (Number.isFinite(periodStart)) {
          updatePayload.subscription_start_date = new Date(periodStart * 1000).toISOString();
        }
        if (Number.isFinite(periodEnd)) {
          updatePayload.subscription_end_date = new Date(periodEnd * 1000).toISOString();
        }

        console.log("[Webhook TEST] Update payload:", JSON.stringify(updatePayload));

        const { error } = await supabase
          .from("users")
          .update(updatePayload)
          .eq("stripe_customer_id", customerId);

        if (error) console.error("[Webhook TEST] Subscription update error:", error.message);
        else console.log("[Webhook TEST] Subscription updated successfully");
        break;
      }

      // ============ SUBSCRIPTION DELETED ============
      // User cancelled and subscription ended
      case "customer.subscription.deleted": {
        const subscription = event.data.object as any;
        const customerId = subscription.customer;

        console.log(`[Webhook TEST] Subscription deleted - customer: ${customerId}`);

        const { error } = await supabase
          .from("users")
          .update({
            subscription_status: "cancelled",
            subscription_tier: "free",
          })
          .eq("stripe_customer_id", customerId);

        if (error) console.error("[Webhook TEST] Deletion update error:", error.message);
        else console.log("[Webhook TEST] User downgraded to free tier");
        break;
      }

      // ============ PAYMENT SUCCEEDED ============
      // Monthly renewal payment successful - IMPORTANT: Update period dates for quota reset!
      case "invoice.payment_succeeded": {
        const invoice = event.data.object as any;
        const customerId = invoice.customer;

        console.log(`[Webhook TEST] Payment succeeded - customer: ${customerId}`);
        console.log(`[Webhook TEST] Billing reason: ${invoice.billing_reason}`);

        // Build update payload
        const updatePayload: Record<string, any> = {
          subscription_status: "active",
        };

        // For subscription renewals/cycles, update the period dates from invoice lines
        // This resets the quota for the new billing period
        if (invoice.lines?.data?.[0]?.period) {
          const period = invoice.lines.data[0].period;
          if (period.start) {
            updatePayload.subscription_start_date = new Date(period.start * 1000).toISOString();
            console.log(`[Webhook TEST] New period start: ${updatePayload.subscription_start_date}`);
          }
          if (period.end) {
            updatePayload.subscription_end_date = new Date(period.end * 1000).toISOString();
            console.log(`[Webhook TEST] New period end: ${updatePayload.subscription_end_date}`);
          }
        }

        console.log("[Webhook TEST] Payment update payload:", JSON.stringify(updatePayload));

        const { error } = await supabase
          .from("users")
          .update(updatePayload)
          .eq("stripe_customer_id", customerId);

        if (error) console.error("[Webhook TEST] Payment success update error:", error.message);
        else console.log("[Webhook TEST] Status and period dates updated");
        break;
      }

      // ============ PAYMENT FAILED ============
      // Monthly renewal payment failed
      case "invoice.payment_failed": {
        const invoice = event.data.object as any;
        const customerId = invoice.customer;

        console.log(`[Webhook TEST] Payment failed - customer: ${customerId}`);

        // Mark as past_due but keep tier (they might fix payment)
        const { error } = await supabase
          .from("users")
          .update({ subscription_status: "past_due" })
          .eq("stripe_customer_id", customerId);

        if (error) console.error("[Webhook TEST] Payment failed update error:", error.message);
        else console.log("[Webhook TEST] Status set to past_due");
        break;
      }

      // ============ UNHANDLED EVENTS ============
      default:
        console.log(`[Webhook TEST] Unhandled event type: ${event.type}`);
    }

    return new Response(JSON.stringify({ received: true }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  } catch (err: any) {
    console.error("[Webhook TEST] Error:", err?.message ?? err);
    return new Response(
      JSON.stringify({ error: err?.message ?? "Unknown error" }),
      { status: 400, headers: { "Content-Type": "application/json" } }
    );
  }
});
