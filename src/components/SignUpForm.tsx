/**
 * SignUpForm - Email/password sign up component
 */

import { useState } from 'react';
import { signUp } from '../services/supabase';

interface SignUpFormProps {
  onSuccess: () => void;
  onSwitchToSignIn: () => void;
}

export default function SignUpForm({ onSuccess, onSwitchToSignIn }: SignUpFormProps) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const validateForm = (): boolean => {
    if (password.length < 6) {
      setError('Password must be at least 6 characters long.');
      return false;
    }
    if (password !== confirmPassword) {
      setError('Passwords do not match.');
      return false;
    }
    return true;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!validateForm()) {
      return;
    }

    setIsLoading(true);

    const { error: signUpError } = await signUp(email, password);

    setIsLoading(false);

    if (signUpError) {
      // Map common errors to user-friendly messages
      if (signUpError.includes('already registered')) {
        setError('This email is already registered. Try signing in instead.');
      } else if (signUpError.includes('valid email')) {
        setError('Please enter a valid email address.');
      } else {
        setError(signUpError);
      }
    } else {
      onSuccess();
    }
  };

  return (
    <form className="auth-form" onSubmit={handleSubmit}>
      <h2>Create Account</h2>
      <p className="auth-hint">Start your free trial with 2 AI-powered solves</p>

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
          placeholder="At least 6 characters"
          required
          minLength={6}
          autoComplete="new-password"
        />
      </div>

      <div className="form-group">
        <label htmlFor="confirmPassword">Confirm Password</label>
        <input
          id="confirmPassword"
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          placeholder="••••••••"
          required
          autoComplete="new-password"
        />
      </div>

      <button type="submit" className="auth-button primary" disabled={isLoading}>
        {isLoading ? 'Creating Account...' : 'Create Account'}
      </button>

      <div className="auth-divider">
        <span>Already have an account?</span>
      </div>

      <button
        type="button"
        className="auth-button secondary"
        onClick={onSwitchToSignIn}
      >
        Sign In
      </button>

      <div className="signup-benefits">
        <h4>What you get:</h4>
        <ul>
          <li>✓ 2 free AI-powered solves</li>
          <li>✓ Support for LeetCode, HackerRank & more</li>
          <li>✓ Multiple AI models (Claude, Gemini)</li>
          <li>✓ System audio capture for verbal interviews</li>
        </ul>
      </div>
    </form>
  );
}





