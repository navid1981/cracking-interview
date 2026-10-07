// Supabase Edge Function: Notification System
// deno-lint-ignore-file

import { FREE_LIFETIME_CALL_LIMIT, PRO_MONTHLY_REQUEST_LIMIT } from "../_shared/limits.ts";
import { PRO_MODELS } from "../_shared/models.ts";

const LATEST_APP_VERSION = '2.0.0';

/** Numeric semver compare: string compare would put "10.0.0" before "2.0.0". */
function isOlderVersion(version: string, than: string): boolean {
  const a = version.split('.').map(n => parseInt(n, 10) || 0);
  const b = than.split('.').map(n => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const diff = (a[i] ?? 0) - (b[i] ?? 0);
    if (diff !== 0) return diff < 0;
  }
  return false;
}

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type',
}

interface NotificationRequest {
  email: string;
  user_type: 'free' | 'pro';
  app_version: string;
}

// ========== ANNOUNCEMENTS CONFIGURATION ==========
// Define announcements here (just title and message, no conditions)
const ANNOUNCEMENTS = {
  free_welcome: {
    id: 'free-welcome',
    title: 'Welcome to CrackingInterview! 🎉',
    message: `
      <p>Thanks for using CrackingInterview! Here are some quick tips:</p>
      <ul>
        <li>Open a problem in Chrome, pick the tab and press Solve</li>
        <li>You have ${FREE_LIFETIME_CALL_LIMIT} free AI calls to get started</li>
        <li>Upgrade to Pro for stealth mode, display capture and audio input</li>
      </ul>
      <p><strong>Need help?</strong> Check our <a href="https://crackinginterview.org" target="_blank">documentation</a>.</p>
    `,
  },
  
  pro_welcome: {
    id: 'pro-welcome',
    title: 'Welcome Pro User! ⭐',
    message: `
      <p>Thanks for subscribing to CrackingInterview Pro!</p>
      <ul>
        <li>You have ${PRO_MONTHLY_REQUEST_LIMIT} AI calls per month</li>
        <li>Access to all premium models (${PRO_MODELS.map(m => m.name).join(', ')})</li>
        <li>Display capture and audio input unlocked</li>
      </ul>
      <p><strong>Tip:</strong> Turn on Stealth Mode in Settings → App.</p>
    `,
  },
  
  update_available: {
    id: 'update-available',
    title: 'Update Available! 🚀',
    message: `
      <p>A new version of CrackingInterview is available with bug fixes and improvements.</p>
      <p><a href="https://crackinginterview.org/download" target="_blank">Download the latest version</a></p>
    `,
  },
}

// ================================================

function getMatchingAnnouncement(user_type: 'free' | 'pro', app_version: string) {
  // Use if conditions based on user attributes to choose announcement
  
  if (isOlderVersion(app_version, LATEST_APP_VERSION)) {
    return ANNOUNCEMENTS.update_available
  }
  
  // Show welcome message for free users
  if (user_type === 'free') {
    return ANNOUNCEMENTS.free_welcome
  }
  
  // Show welcome message for pro users
  if (user_type === 'pro') {
    return ANNOUNCEMENTS.pro_welcome
  }

  // No announcement
  return null
}

Deno.serve(async (req) => {
  // Handle CORS preflight
  if (req.method === 'OPTIONS') {
    return new Response('ok', { headers: corsHeaders })
  }

  try {
    const { email, user_type, app_version }: NotificationRequest = await req.json()

    console.log(`[Notification] Request (${user_type}, v${app_version})`)

    // Validate input
    if (!email || !user_type || !app_version) {
      return new Response(
        JSON.stringify({ error: 'Missing required fields: email, user_type, app_version' }),
        { status: 400, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      )
    }

    // Get the first matching announcement for this user
    const announcement = getMatchingAnnouncement(user_type, app_version)

    console.log(`[Notification] Returning ${announcement ? `announcement: ${announcement.title}` : 'no announcement'}`)

    return new Response(
      JSON.stringify({ announcement }),
      {
        headers: { ...corsHeaders, 'Content-Type': 'application/json' },
      }
    )
  } catch (error) {
    console.error('[Notification] Error:', error)
    return new Response(
      JSON.stringify({ error: error.message }),
      { status: 500, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    )
  }
})

