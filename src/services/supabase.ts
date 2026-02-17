/**
 * Supabase Client Configuration
 * 
 * This module initializes and exports the Supabase client for authentication
 * and database operations.
 */

import { createClient, SupabaseClient, User, Session } from '@supabase/supabase-js';

// Supabase configuration from credentials
const SUPABASE_URL = 'https://uudwpcjxbwtszhhcgybj.supabase.co';
const SUPABASE_ANON_KEY = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InV1ZHdwY2p4Ynd0c3poaGNneWJqIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NjQ5MTAzMDksImV4cCI6MjA4MDQ4NjMwOX0.wKsiXAAK3q2pQdR8UGT7gXeBsXUDki-YAuB0CtJ9ZUI';

// Edge Function base URL
export const EDGE_FUNCTION_URL = `${SUPABASE_URL}/functions/v1`;
export const SUPABASE_API_KEY = SUPABASE_ANON_KEY; // Export for edge function calls

// Initialize Supabase client
export const supabase: SupabaseClient = createClient(SUPABASE_URL, SUPABASE_ANON_KEY, {
  auth: {
    autoRefreshToken: true,
    persistSession: true,
    detectSessionInUrl: false, // Desktop app doesn't use URL-based auth
  },
});

/**
 * Utility: Race a promise against a timeout
 * Returns the fallback value if the promise takes too long
 */
async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, fallback: T): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((resolve) => setTimeout(() => {
      resolve(fallback);
    }, timeoutMs))
  ]);
}

/**
 * User subscription data from the users table
 */
export interface UserSubscription {
  id: string;
  email: string;
  subscription_status: 'active' | 'inactive' | 'cancelled' | 'cancelling' | null;
  subscription_tier: string | null;
  stripe_customer_id: string | null;
  lifetime_ai_calls: number;
  subscription_start_date: string | null;
  subscription_end_date: string | null;
}

/**
 * Usage statistics for quota display
 */
export interface UsageStats {
  requests_used: number;
  requests_limit: number;
  period_start: Date;
  period_end: Date;
}

/**
 * Get the current user's subscription data
 */
export async function getUserSubscription(userId: string): Promise<UserSubscription | null> {
  const fetchSubscription = async (): Promise<UserSubscription | null> => {
    const { data, error } = await supabase
      .from('users')
      .select('*')
      .eq('id', userId)
      .single();

    if (error) {
      console.error('Error fetching user subscription:', error);
      return null;
    }

    return data as UserSubscription;
  };

  // 3 second timeout to avoid VPN hangs
  return withTimeout(fetchSubscription(), 3000, null);
}

/**
 * Get usage statistics for the current billing period
 * Counts requests from subscription_start_date to subscription_end_date
 */
export async function getUsageStats(userId: string, subscription?: UserSubscription | null): Promise<UsageStats> {
  const now = new Date();
  
  // Use subscription dates if available, otherwise fall back to all-time
  let periodStart: Date;
  let periodEnd: Date;
  
  if (subscription?.subscription_start_date) {
    periodStart = new Date(subscription.subscription_start_date);
    periodEnd = subscription.subscription_end_date 
      ? new Date(subscription.subscription_end_date) 
      : new Date(now.getTime() + 30 * 24 * 60 * 60 * 1000); // +30 days if no end date
  } else {
    // No subscription - count all time (for display purposes)
    periodStart = new Date('2020-01-01');
    periodEnd = new Date('2099-12-31');
  }

  const defaultStats: UsageStats = {
    requests_used: 0,
    requests_limit: 150,
    period_start: periodStart,
    period_end: periodEnd,
  };

  const fetchStats = async (): Promise<UsageStats> => {
    // Note: Session should already be set by auth flow, no need to call setSession here
    // (calling setSession triggers onAuthStateChange which creates a loop!)
    
    const { count, error } = await supabase
      .from('api_usage')
      .select('*', { count: 'exact', head: true })
      .eq('user_id', userId)
      .gte('created_at', periodStart.toISOString())
      .lt('created_at', periodEnd.toISOString());

    if (error) {
      console.error('[Usage] Query error:', error.message);
      return defaultStats;
    }


    return {
      requests_used: count || 0,
      requests_limit: 150,
      period_start: periodStart,
      period_end: periodEnd,
    };
  };

  // 5 second timeout to avoid VPN hangs
  return withTimeout(fetchStats(), 5000, defaultStats);
}

/**
 * Create a checkout session for Stripe subscription
 * Uses Tauri command to bypass webview network restrictions
 */
export async function createCheckoutSession(userId: string, email: string): Promise<string | null> {
  
  try {
    // Use dynamic import to avoid issues when running in browser
    const { invoke } = await import('@tauri-apps/api/core');
    
    const checkoutUrl = await invoke<string>('create_checkout_session', {
      userId,
      userEmail: email,
    });
    
    return checkoutUrl;
  } catch (error) {
    console.error('[Checkout] Error:', error);
    throw error;
  }
}

/**
 * Sign up a new user with email and password
 * On corporate VPN, direct Supabase calls fail due to SSL inspection.
 * We use Tauri command (Rust backend) which bypasses SSL issues.
 */
export async function signUp(email: string, password: string): Promise<{ user: User | null; error: string | null }> {
  // On desktop app, always use Tauri backend to bypass SSL inspection issues
  return await signUpViaTauri(email, password);
}

/**
 * Sign up via Tauri command (bypasses SSL inspection)
 * Supabase signup returns user data at ROOT level: { id: "...", email: "...", ... }
 */
async function signUpViaTauri(email: string, password: string): Promise<{ user: User | null; error: string | null }> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const result = await invoke<any>('supabase_sign_up', { email, password });
    
    
    // Supabase signup returns user data at ROOT level (not nested under "user")
    // Response: { id: "uuid", email: "...", confirmation_sent_at: "...", ... }
    if (result.id && result.email) {
      return { user: result as unknown as User, error: null };
    }
    
    // Check for error in response
    const errorMsg = result.error || result.error_description || result.msg || result.message;
    if (errorMsg) {
      return { user: null, error: errorMsg };
    }
    
    return { user: null, error: 'Unexpected response from server' };
  } catch (e) {
    console.error('[Auth] Tauri sign up error:', e);
    return { user: null, error: String(e) };
  }
}

/**
 * Sign in an existing user
 * On corporate VPN, direct Supabase calls fail due to SSL inspection.
 * We use Tauri command (Rust backend) which bypasses SSL issues.
 */
export async function signIn(email: string, password: string): Promise<{ user: User | null; session: Session | null; error: string | null }> {
  // On desktop app, always use Tauri backend to bypass SSL inspection issues
  return await signInViaTauri(email, password);
}

/**
 * Sign in via Tauri command (bypasses SSL inspection)
 * Also sets the session in Supabase client for subsequent calls
 * Supabase signin returns: { access_token, refresh_token, user: {...}, ... }
 */
async function signInViaTauri(email: string, password: string): Promise<{ user: User | null; session: Session | null; error: string | null }> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const result = await invoke<any>('supabase_sign_in', { email, password });
    
    
    // Check for error first
    if (result.error || result.error_description) {
      const errorMsg = result.error_description || result.error || 'Sign in failed';
      return { user: null, session: null, error: errorMsg };
    }
    
    if (result.access_token && result.user) {
      
      // Build the session object
      const session: Session = {
        access_token: result.access_token,
        refresh_token: result.refresh_token || '',
        expires_in: result.expires_in || 3600,
        expires_at: Math.floor(Date.now() / 1000) + (result.expires_in || 3600),
        token_type: 'bearer',
        user: result.user as unknown as User,
      };
      
      // Store in localStorage directly (instant, no network)
      const storageKey = `sb-${SUPABASE_URL.split('//')[1].split('.')[0]}-auth-token`;
      localStorage.setItem(storageKey, JSON.stringify(session));
      
      // Fire-and-forget: try to set session in Supabase client (don't wait)
      supabase.auth.setSession({
        access_token: result.access_token,
        refresh_token: result.refresh_token || '',
      }).then(() => {
      }).catch(() => {
        // Silent fail - setSession error (VPN/SSL issue)
      });
      
      return { user: result.user as unknown as User, session, error: null };
    }
    
    return { user: null, session: null, error: 'Sign in failed - no token received' };
  } catch (e) {
    console.error('[Auth] Tauri sign in error:', e);
    return { user: null, session: null, error: String(e) };
  }
}

/**
 * Sign out the current user
 */
export async function signOut(): Promise<{ error: string | null }> {
  // Clear local storage immediately (works even on VPN)
  const storageKey = `sb-${SUPABASE_URL.split('//')[1].split('.')[0]}-auth-token`;
  localStorage.removeItem(storageKey);
  
  // Try to sign out from Supabase server (with short timeout)
  // This invalidates the token server-side, but local logout is already done
  const signOutPromise = supabase.auth.signOut().then(({ error }) => ({ 
    error: error?.message || null 
  }));
  
  // 2 second timeout - local logout already happened, so this is just cleanup
  return withTimeout(signOutPromise, 2000, { error: null });
}

/**
 * Send password reset email
 */
export async function resetPassword(email: string): Promise<{ error: string | null }> {
  const { error } = await supabase.auth.resetPasswordForEmail(email, {
    redirectTo: 'https://crackinginterview.org/reset-password', // User clicks link, updates on web
  });

  return { error: error?.message || null };
}

/**
 * Get current session with timeout to avoid VPN hangs
 * Falls back to localStorage if network is slow
 */
export async function getSession(): Promise<Session | null> {
  // Race between Supabase call and a timeout
  const timeoutMs = 2000; // 2 second max wait
  
  const supabasePromise = supabase.auth.getSession().then(({ data }) => data.session);
  const timeoutPromise = new Promise<Session | null>((resolve) => {
    setTimeout(() => {
      // Try to get session from localStorage as fallback
      const stored = localStorage.getItem(`sb-${SUPABASE_URL.split('//')[1].split('.')[0]}-auth-token`);
      if (stored) {
        try {
          const parsed = JSON.parse(stored);
          resolve(parsed as Session);
        } catch {
          resolve(null);
        }
      } else {
        resolve(null);
      }
    }, timeoutMs);
  });
  
  return Promise.race([supabasePromise, timeoutPromise]);
}

/**
 * Get current user with timeout
 */
export async function getUser(): Promise<User | null> {
  const session = await getSession();
  return session?.user || null;
}

/**
 * Subscribe to auth state changes
 */
export function onAuthStateChange(callback: (event: string, session: Session | null) => void) {
  return supabase.auth.onAuthStateChange((event, session) => {
    callback(event, session);
  });
}

/**
 * Check if user can make AI requests via proxy
 * Returns: { allowed: boolean, reason?: string, remainingCalls?: number }
 */
export async function checkAIQuota(userId: string): Promise<{
  allowed: boolean;
  reason?: string;
  remainingCalls?: number;
  isPaid: boolean;
}> {
  const subscription = await getUserSubscription(userId);
  
  if (!subscription) {
    return { allowed: false, reason: 'User not found', isPaid: false };
  }

  // Include 'cancelling' as paid - they still have access until period ends
  const isPaid = subscription.subscription_status === 'active' || subscription.subscription_status === 'cancelling';

  if (isPaid) {
    // Paid user - check monthly quota
    const usage = await getUsageStats(userId);
    const remaining = usage.requests_limit - usage.requests_used;
    
    if (remaining <= 0) {
      return {
        allowed: false,
        reason: 'Monthly quota exceeded. Resets on ' + usage.period_end.toLocaleDateString(),
        remainingCalls: 0,
        isPaid: true,
      };
    }

    return {
      allowed: true,
      remainingCalls: remaining,
      isPaid: true,
    };
  } else {
    // Free user - check lifetime quota (3 calls)
    const lifetimeUsed = subscription.lifetime_ai_calls || 0;
    const remaining = 3 - lifetimeUsed;

    if (remaining <= 0) {
      return {
        allowed: false,
        reason: 'Free trial expired. Subscribe to continue using AI features.',
        remainingCalls: 0,
        isPaid: false,
      };
    }

    return {
      allowed: true,
      remainingCalls: remaining,
      isPaid: false,
    };
  }
}

