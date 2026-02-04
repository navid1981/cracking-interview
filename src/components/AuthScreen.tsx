/**
 * AuthScreen - Container component for authentication flows
 * 
 * Handles switching between Sign In, Sign Up, and Forgot Password views.
 */

import { useState } from 'react';
import SignInForm from './SignInForm';
import SignUpForm from './SignUpForm';
import './AuthScreen.css';

type AuthView = 'signin' | 'signup' | 'forgot-password' | 'check-email';

interface AuthScreenProps {
  onAuthSuccess: () => void;
}

export default function AuthScreen({ onAuthSuccess }: AuthScreenProps) {
  const [view, setView] = useState<AuthView>('signin');
  const [message, setMessage] = useState<string>('');
  const [emailForReset, setEmailForReset] = useState<string>('');

  const handleSignUpSuccess = () => {
    setMessage('Check your email for a confirmation link!');
    setView('check-email');
  };

  const handleForgotPasswordSuccess = (email: string) => {
    setEmailForReset(email);
    setMessage(`Password reset link sent to ${email}`);
    setView('check-email');
  };

  return (
    <div className="auth-screen">
      <div className="auth-container">
        <div className="auth-header">
          <div className="auth-logo">
            <img src="/icon.png" alt="CrackingInterview" className="logo-icon-img" />
            <h1>CrackingInterview</h1>
          </div>
          <p className="auth-tagline">Your AI-powered technical interview assistant</p>
        </div>

        {view === 'signin' && (
          <SignInForm
            onSuccess={onAuthSuccess}
            onSwitchToSignUp={() => setView('signup')}
            onForgotPassword={() => setView('forgot-password')}
          />
        )}

        {view === 'signup' && (
          <SignUpForm
            onSuccess={handleSignUpSuccess}
            onSwitchToSignIn={() => setView('signin')}
          />
        )}

        {view === 'forgot-password' && (
          <ForgotPasswordForm
            onSuccess={handleForgotPasswordSuccess}
            onBack={() => setView('signin')}
          />
        )}

        {view === 'check-email' && (
          <div className="auth-form check-email">
            <div className="check-email-icon">✉️</div>
            <h2>Check Your Email</h2>
            <p className="auth-message success">{message}</p>
            <p className="auth-hint">
              {emailForReset 
                ? 'Click the link in the email to reset your password.'
                : 'Click the confirmation link to activate your account.'}
            </p>
            <button
              className="auth-button secondary"
              onClick={() => setView('signin')}
            >
              Back to Sign In
            </button>
          </div>
        )}

        <div className="auth-footer">
          <p>By continuing, you agree to our</p>
          <div className="auth-links">
            <a href="https://crackinginterview.org/terms.html" target="_blank" rel="noopener">
              Terms of Service
            </a>
            <span>•</span>
            <a href="https://crackinginterview.org/privacy.html" target="_blank" rel="noopener">
              Privacy Policy
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}

/**
 * Forgot Password Form Component
 */
interface ForgotPasswordFormProps {
  onSuccess: (email: string) => void;
  onBack: () => void;
}

import { resetPassword } from '../services/supabase';

function ForgotPasswordForm({ onSuccess, onBack }: ForgotPasswordFormProps) {
  const [email, setEmail] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsLoading(true);

    const { error: resetError } = await resetPassword(email);

    setIsLoading(false);

    if (resetError) {
      setError(resetError);
    } else {
      onSuccess(email);
    }
  };

  return (
    <form className="auth-form" onSubmit={handleSubmit}>
      <h2>Reset Password</h2>
      <p className="auth-hint">Enter your email and we'll send you a reset link.</p>

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
          autoFocus
        />
      </div>

      <button type="submit" className="auth-button primary" disabled={isLoading}>
        {isLoading ? 'Sending...' : 'Send Reset Link'}
      </button>

      <button type="button" className="auth-button text" onClick={onBack}>
        ← Back to Sign In
      </button>
    </form>
  );
}





