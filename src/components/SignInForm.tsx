/**
 * SignInForm - Email/password sign in component
 */

import { useState } from 'react';
import { signIn } from '../services/supabase';

interface SignInFormProps {
  onSuccess: () => void;
  onSwitchToSignUp: () => void;
  onForgotPassword: () => void;
}

export default function SignInForm({ onSuccess, onSwitchToSignUp, onForgotPassword }: SignInFormProps) {
  // Pre-fill email from remembered value (saved when user last signed in)
  const rememberedEmail = localStorage.getItem('cracking_interview_remembered_email') || '';
  
  const [email, setEmail] = useState(rememberedEmail);
  const [password, setPassword] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsLoading(true);
    
    console.log('[SignInForm] Starting sign in...');

    try {
      const result = await signIn(email, password);
      console.log('[SignInForm] signIn returned:', result);
      
      setIsLoading(false);

      if (result.error) {
        console.log('[SignInForm] Sign in error:', result.error);
        // Map common errors to user-friendly messages
        if (result.error.includes('Invalid login credentials')) {
          setError('Invalid email or password. Please try again.');
        } else if (result.error.includes('Email not confirmed')) {
          setError('Please confirm your email address before signing in.');
        } else {
          setError(result.error);
        }
      } else {
        console.log('[SignInForm] Sign in successful, calling onSuccess');
        onSuccess();
      }
    } catch (err) {
      console.error('[SignInForm] Exception during sign in:', err);
      setIsLoading(false);
      setError(String(err));
    }
  };

  return (
    <form className="auth-form" onSubmit={handleSubmit}>
      <h2>Welcome Back</h2>

      {error && <p className="auth-message error">{error}</p>}

      <div className="form-group">
        <label htmlFor="email">Email</label>
        <input
          id="email"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder="you@example.com"
          required
          autoFocus={!rememberedEmail}
          autoComplete="email"
        />
      </div>

      <div className="form-group">
        <label htmlFor="password">Password</label>
        <input
          id="password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="••••••••"
          required
          autoFocus={!!rememberedEmail}
          autoComplete="current-password"
        />
      </div>

      <button
        type="button"
        className="forgot-password-link"
        onClick={onForgotPassword}
      >
        Forgot your password?
      </button>

      <button type="submit" className="auth-button primary" disabled={isLoading}>
        {isLoading ? 'Signing in...' : 'Sign In'}
      </button>

      <div className="auth-divider">
        <span>New to CrackingInterview?</span>
      </div>

      <button
        type="button"
        className="auth-button secondary"
        onClick={onSwitchToSignUp}
      >
        Create an Account
      </button>
    </form>
  );
}





