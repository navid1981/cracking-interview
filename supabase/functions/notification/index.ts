// Supabase Edge Function: Notification System
// deno-lint-ignore-file

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
        <li>Try <a href="https://leetcode.com" target="_blank">LeetCode</a> problems in stealth mode</li>
        <li>You have 3 free AI calls to get started</li>
        <li>Upgrade to Pro for any website + screen capture + audio input</li>
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
        <li>You have 150 AI calls per month</li>
        <li>Access to all premium models (GPT-5, Claude, Gemini)</li>
        <li>Display capture feature unlocked</li>
      </ul>
      <p><strong>Tip:</strong> Use stealth mode for real interviews!</p>
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
  
  // Example: Show update notification to users with version 1.0.x
  if (app_version >= '0.0.0' && app_version < '1.0.0') {
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

    console.log(`[Notification] Request from ${email} (${user_type}, v${app_version})`)

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

