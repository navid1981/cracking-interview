import './SetupChecklist.css';

export interface SetupStatus {
  platform: 'macos' | 'windows' | 'other';
  chrome_installed: boolean;
  accessibility_granted: boolean | null;
  screen_recording_granted: boolean | null;
}

export type PrivacyPane = 'accessibility' | 'screen_recording';

type StepId = 'chrome_installed' | 'chrome_connected' | PrivacyPane;

export interface SetupStep {
  id: StepId;
  label: string;
  /** Used by the compact Settings panel. */
  shortLabel: string;
  description: string;
  done: boolean;
  /** One-time requirements drive the main-screen card and header badge. "Chrome connected" is
   *  per-session and already has its own header button, so it only appears as a row. */
  oneTime: boolean;
}

export function getSetupSteps(status: SetupStatus | null, cdpReady: boolean, isPro: boolean): SetupStep[] {
  if (!status) return [];
  const steps: SetupStep[] = [
    {
      id: 'chrome_installed',
      label: 'Google Chrome installed',
      shortLabel: 'Chrome installed',
      description: 'The app reads problems from your Chrome tabs.',
      done: status.chrome_installed || cdpReady,
      oneTime: true,
    },
    {
      id: 'chrome_connected',
      label: 'Chrome connected',
      shortLabel: 'Chrome connected',
      description: 'Connect once per session to list your tabs.',
      done: cdpReady,
      oneTime: false,
    },
  ];
  if (status.accessibility_granted !== null) {
    steps.push({
      id: 'accessibility',
      label: 'Accessibility',
      shortLabel: 'Accessibility',
      description: 'Lets global hotkeys work while you are in other apps.',
      done: status.accessibility_granted,
      oneTime: true,
    });
  }
  if (status.screen_recording_granted !== null && isPro) {
    steps.push({
      id: 'screen_recording',
      label: 'Screen & System Audio Recording',
      shortLabel: 'Screen & Audio',
      description: 'Needed for display capture and audio transcription.',
      done: status.screen_recording_granted,
      oneTime: true,
    });
  }
  return steps;
}

const RESTART_HINT = 'Turned it on? macOS may need a restart before the app can see it.';

interface Props {
  steps: SetupStep[];
  variant: 'card' | 'panel';
  /** Permissions the user tried to grant this session; macOS may need a restart to report them. */
  requested: Partial<Record<PrivacyPane, boolean>>;
  isOpeningChrome: boolean;
  onOpenChrome: () => void;
  onDownloadChrome: () => void;
  onGrant: (pane: PrivacyPane) => void;
  onRestart: () => void;
  onRecheck: () => void;
  onHide?: () => void;
}

export default function SetupChecklist({
  steps, variant, requested, isOpeningChrome,
  onOpenChrome, onDownloadChrome, onGrant, onRestart, onRecheck, onHide,
}: Props) {
  const doneCount = steps.filter(s => s.done).length;
  const allDone = steps.length > 0 && doneCount === steps.length;
  const compact = variant === 'panel';

  if (variant === 'card' && allDone) {
    return (
      <div className="setup-card setup-card-success" role="status">
        <span className="setup-step-icon done" aria-hidden="true">✓</span>
        <span className="setup-success-text">You're all set. Everything is ready to go.</span>
      </div>
    );
  }

  const renderAction = (step: SetupStep) => {
    if (step.done) return null;
    switch (step.id) {
      case 'chrome_installed':
        return <button className="setup-action" onClick={onDownloadChrome}>{compact ? 'Download' : 'Download Chrome'}</button>;
      case 'chrome_connected':
        return (
          <button className="setup-action" onClick={onOpenChrome} disabled={isOpeningChrome}>
            {isOpeningChrome ? 'Connecting…' : compact ? 'Connect' : 'Open Chrome'}
          </button>
        );
      case 'accessibility':
      case 'screen_recording':
        return requested[step.id] ? (
          <div className="setup-action-group">
            <button className="setup-action secondary" onClick={() => onGrant(step.id as PrivacyPane)}>{compact ? 'Settings' : 'Open Settings'}</button>
            <button className="setup-action" onClick={onRestart} title={RESTART_HINT}>{compact ? 'Restart' : 'Restart app'}</button>
          </div>
        ) : (
          <button className="setup-action" onClick={() => onGrant(step.id as PrivacyPane)}>{compact ? 'Grant' : 'Grant access'}</button>
        );
    }
  };

  if (compact) {
    return (
      <div className="setup-panel">
        <div className="setup-panel-header">
          <span className="app-settings-label">Setup & Permissions</span>
          <span className={`setup-panel-count ${allDone ? 'is-complete' : ''}`}>{doneCount}/{steps.length}</span>
          <button className="setup-link" onClick={onRecheck}>Re-check</button>
        </div>
        <ul className="setup-grid">
          {steps.map(step => (
            <li
              key={step.id}
              className={`setup-chip ${step.done ? 'is-done' : ''}`}
              title={`${step.label}: ${step.description}`}
            >
              <span className={`setup-chip-icon ${step.done ? 'done' : ''}`} aria-hidden="true">{step.done ? '✓' : '!'}</span>
              <span className="setup-chip-label">{step.shortLabel}</span>
              {renderAction(step)}
            </li>
          ))}
        </ul>
      </div>
    );
  }

  return (
    <div className="setup-card">
      <div className="setup-header">
        <div>
          <div className="setup-title">Finish setting up</div>
          <div className="setup-subtitle">{doneCount} of {steps.length} complete</div>
        </div>
        {onHide && (
          <button className="setup-close" onClick={onHide} aria-label="Hide for this session" title="Hide for this session">✕</button>
        )}
      </div>

      <div className="setup-progress" aria-hidden="true">
        <div className="setup-progress-fill" style={{ width: `${steps.length ? (doneCount / steps.length) * 100 : 0}%` }} />
      </div>

      <ul className="setup-steps">
        {steps.map((step, i) => (
          <li key={step.id} className={`setup-step ${step.done ? 'is-done' : ''}`}>
            <span className={`setup-step-icon ${step.done ? 'done' : ''}`} aria-hidden="true">
              {step.done ? '✓' : i + 1}
            </span>
            <div className="setup-step-body">
              <div className="setup-step-label">{step.label}</div>
              <div className="setup-step-desc">
                {!step.done && requested[step.id as PrivacyPane] ? RESTART_HINT : step.description}
              </div>
            </div>
            {renderAction(step)}
          </li>
        ))}
      </ul>

      <div className="setup-footer">
        <button className="setup-link" onClick={onRecheck}>Re-check</button>
      </div>
    </div>
  );
}
