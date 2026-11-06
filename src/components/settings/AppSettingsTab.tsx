import SetupChecklist from '../SetupChecklist';
import type { ComponentProps } from 'react';

type Theme = 'light' | 'dark';

interface Props {
  windowOpacity: number;
  onOpacityChange: (opacity: number) => void;
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
  isPro: boolean;
  stealthMode: boolean;
  onStealthModeChange: (enabled: boolean) => void;
  /** macOS only: whether the event tap that hides hotkeys from other apps is running. */
  hotkeySwallowingActive: boolean;
  isMac: boolean;
  onRequestAccessibility: () => void;
  onRefreshStealthStatus: () => void;
  setupChecklist: Omit<ComponentProps<typeof SetupChecklist>, 'variant'>;
}

export default function AppSettingsTab({
  windowOpacity, onOpacityChange, theme, onThemeChange,
  isPro, stealthMode, onStealthModeChange, hotkeySwallowingActive, isMac,
  onRequestAccessibility, onRefreshStealthStatus, setupChecklist,
}: Props) {
  const restartNote = isMac && (
    <div className="stealth-note">* Stealth changes take effect after restarting the app.</div>
  );

  return (
    <>
      <div className="app-settings-group">
        <div className="app-settings-label">Transparency</div>
        <div className="app-settings-desc">Adjust the window opacity from fully visible to semi-transparent.</div>
        <div className="opacity-slider-row">
          <input
            type="range"
            min="10"
            max="100"
            value={Math.round(windowOpacity * 100)}
            onChange={(e) => onOpacityChange(parseInt(e.target.value) / 100)}
            className="opacity-slider"
          />
          <span className="opacity-value">{Math.round(windowOpacity * 100)}%</span>
        </div>
      </div>

      <div className="app-settings-group">
        <div className="app-settings-label">Theme</div>
        <div className="app-settings-desc">Switch between light and dark appearance.</div>
        <div className="theme-toggle-group">
          <button className={`theme-toggle-btn ${theme === 'light' ? 'active' : ''}`} onClick={() => onThemeChange('light')}>
            ☀️ Light
          </button>
          <button className={`theme-toggle-btn ${theme === 'dark' ? 'active' : ''}`} onClick={() => onThemeChange('dark')}>
            🌙 Dark
          </button>
        </div>
      </div>

      <div className="app-settings-group">
        <div className="app-settings-label stealth-heading">
          Stealth Mode
          {!isPro && <span className="pro-only-tag">Pro Only</span>}
        </div>
        <div className="stealth-toggle-row">
          <label className={`toggle-switch ${!isPro ? 'disabled' : ''}`}>
            <input
              type="checkbox"
              checked={isPro && stealthMode}
              disabled={!isPro}
              onChange={(e) => isPro && onStealthModeChange(e.target.checked)}
            />
            <span className="toggle-switch-slider" />
          </label>
          <span className="stealth-label">
            {isPro ? (stealthMode ? 'Enabled' : 'Disabled') : 'Disabled (Pro Only)'}
          </span>
        </div>

        {isPro && stealthMode ? (
          <div className="stealth-box is-active">
            <div className="stealth-box-title">🛡️ Anti-Detection Protection Active</div>
            <div className="stealth-box-text">• <strong>Hotkey Hiding:</strong> Chrome can never catch your HotKey.</div>
            <div className="stealth-box-text">• <strong>Screen & System Protection:</strong> Hidden from screen sharing, screenshots, Dock (macOS) and Taskbar (Windows).</div>
            {restartNote}
            {isMac && !hotkeySwallowingActive && (
              <div className="stealth-actions">
                <button type="button" className="stealth-btn primary" onClick={onRequestAccessibility}>🔑 Request macOS Permission</button>
                <button type="button" className="stealth-btn" onClick={onRefreshStealthStatus}>🔄 Re-check Status</button>
              </div>
            )}
          </div>
        ) : isPro ? (
          <div className="stealth-box is-inactive">
            <div className="stealth-box-title">🛡️ Anti-Detection Protection Inactive</div>
            <div className="stealth-box-text">
              Enable Stealth Mode above so Chrome cannot catch your HotKey, and the window is hidden from screen sharing, screenshots, Dock (macOS) and Taskbar (Windows).
            </div>
            {restartNote}
          </div>
        ) : (
          <div className="stealth-box is-locked">
            <div className="stealth-box-title">🔒 Pro Feature: Stealth Mode</div>
            <div className="stealth-box-text">
              Upgrade to Pro to unlock Stealth Mode (Undetectable and fully private. Hidden hotkeys, and the window never appears in screen shares, recordings, screenshots, the Dock or the Taskbar).
            </div>
          </div>
        )}
      </div>

      <div className="app-settings-group">
        <SetupChecklist variant="panel" {...setupChecklist} />
      </div>
    </>
  );
}
