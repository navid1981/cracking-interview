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
  // Email is always remembered; password only if "Remember me" was checked
  const rememberedEmail = localStorage.getItem('cracking_interview_remembered_email') || '';
  const rememberedPassword = localStorage.getItem('cracking_interview_remembered_password') || '';
  const hadRememberedPassword = !!rememberedPassword;
  
  const [email, setEmail] = useState(rememberedEmail);
  const [password, setPassword] = useState(rememberedPassword);
  const [showPassword, setShowPassword] = useState(false);
  const [rememberMe, setRememberMe] = useState(hadRememberedPassword);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsLoading(true);

    try {
      const result = await signIn(email, password);
      
      setIsLoading(false);

      if (result.error) {
        // Map common errors to user-friendly messages
        if (result.error.includes('Invalid login credentials')) {
          setError('Invalid email or password. Please try again.');
        } else if (result.error.includes('Email not confirmed')) {
          setError('Please confirm your email address before signing in.');
        } else {
          setError(result.error);
        }
      } else {
        // Always remember email
        localStorage.setItem('cracking_interview_remembered_email', email);
        
        // "Remember me" controls password only
        if (rememberMe) {
          localStorage.setItem('cracking_interview_remember_me', 'true');
          localStorage.setItem('cracking_interview_remembered_password', password);
        } else {
          localStorage.removeItem('cracking_interview_remember_me');
          localStorage.removeItem('cracking_interview_remembered_password');
        }
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
        <div className="password-input-wrapper">
          <input
            id="password"
            type={showPassword ? 'text' : 'password'}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="••••••••"
            required
            autoFocus={!!rememberedEmail}
            autoComplete="current-password"
          />
          <button
            type="button"
            className="password-toggle-btn"
            onClick={() => setShowPassword(v => !v)}
            tabIndex={-1}
            title={showPassword ? 'Hide password' : 'Show password'}
          >
            {showPassword ? '🙈' : '👁️'}
          </button>
        </div>
      </div>

      <div className="form-row-between">
        <label className="remember-me-label">
          <input
            type="checkbox"
            checked={rememberMe}
            onChange={(e) => setRememberMe(e.target.checked)}
            className="remember-me-checkbox"
          />
          Remember me
        </label>
        <button
          type="button"
          className="forgot-password-link"
          onClick={onForgotPassword}
        >
          Forgot password?
        </button>
      </div>

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





