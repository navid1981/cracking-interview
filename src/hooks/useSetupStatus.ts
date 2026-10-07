import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getSetupSteps, SetupStatus, PrivacyPane } from '../components/SetupChecklist';

/**
 * Setup checklist state: re-checked on window focus (no polling). `hidden` is session-only,
 * so the main-screen card comes back on next launch if something is still missing.
 */
export function useSetupStatus(cdpReady: boolean, isPro: boolean, onPermissionChange: () => void) {
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [requested, setRequested] = useState<Partial<Record<PrivacyPane, boolean>>>({});
  const [hidden, setHidden] = useState(false);
  const [justCompleted, setJustCompleted] = useState(false);
  const prevMissingRef = useRef<number | null>(null);

  const refresh = async () => {
    try {
      setStatus(await invoke<SetupStatus>('get_setup_status'));
    } catch (e) {
      console.error('Failed to read setup status:', e);
    }
  };

  useEffect(() => {
    refresh();
    window.addEventListener('focus', refresh);
    return () => window.removeEventListener('focus', refresh);
  }, []);

  const steps = getSetupSteps(status, cdpReady, isPro);
  const oneTimeMissing = steps.filter(s => s.oneTime && !s.done).length;

  useEffect(() => {
    const prev = prevMissingRef.current;
    prevMissingRef.current = status ? oneTimeMissing : null;
    if (prev !== null && prev > 0 && oneTimeMissing === 0 && !hidden) {
      setJustCompleted(true);
      const t = setTimeout(() => setJustCompleted(false), 4000);
      return () => clearTimeout(t);
    }
  }, [oneTimeMissing, status, hidden]);

  const grant = async (pane: PrivacyPane) => {
    try {
      await invoke(pane === 'accessibility' ? 'request_accessibility' : 'request_screen_recording');
      await invoke('open_privacy_settings', { pane });
    } catch (e) {
      console.error(`Failed to request ${pane}:`, e);
    }
    setRequested(prev => ({ ...prev, [pane]: true }));
    refresh();
    onPermissionChange();
  };

  const recheck = () => {
    refresh();
    onPermissionChange();
  };

  const hide = () => {
    setHidden(true);
    setJustCompleted(false);
  };

  return {
    steps,
    oneTimeMissing,
    requested,
    showCard: (oneTimeMissing > 0 && !hidden) || justCompleted,
    show: () => setHidden(false),
    hide,
    grant,
    recheck,
  };
}
