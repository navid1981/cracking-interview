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
  isWindows: boolean;
  runAsAdmin: boolean;
  isElevated: boolean;
  adminRestartStatus: string;
  onRunAsAdminChange: (enabled: boolean) => void;
  onRestartNow: () => void;
  onRequestAccessibility: () => void;
  onRefreshStealthStatus: () => void;
  setupChecklist: Omit<ComponentProps<typeof SetupChecklist>, 'variant'>;
}

const STEALTH_FEATURES = (
  <>
    <div className="stealth-box-text">• <strong>Hidden hotkeys:</strong> your shortcuts are hidden from other apps and browsers.</div>
    <div className="stealth-box-text">• <strong>Hidden window:</strong> not shown in screen sharing, recordings, screenshots, the Dock (macOS) or the Taskbar (Windows).</div>
  </>
);

export default function AppSettingsTab({
  windowOpacity, onOpacityChange, theme, onThemeChange,
  isPro, stealthMode, onStealthModeChange, hotkeySwallowingActive, isMac,
  isWindows, runAsAdmin, isElevated, adminRestartStatus, onRunAsAdminChange, onRestartNow,
  onRequestAccessibility, onRefreshStealthStatus, setupChecklist,
}: Props) {
  const restartNote = isMac && (
    <div className="stealth-note">* Restart the app for changes to take effect.</div>
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
        <div className="app-settings-desc">Keeps the app out of sight while you use it.</div>
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
            <div className="stealth-box-title">🛡️ Stealth Active</div>
            {STEALTH_FEATURES}
            {restartNote}
            {isMac && !hotkeySwallowingActive && (
              <div className="stealth-actions">
                <div className="stealth-box-text">Hidden hotkeys need Accessibility permission (System Settings → Privacy & Security → Accessibility).</div>
                <button type="button" className="stealth-btn primary" onClick={onRequestAccessibility}>🔑 Request macOS Permission</button>
                <button type="button" className="stealth-btn" onClick={onRefreshStealthStatus}>🔄 Re-check Status</button>
              </div>
            )}
          </div>
        ) : isPro ? (
          <div className="stealth-box is-inactive">
            <div className="stealth-box-title">🛡️ Stealth Off</div>
            <div className="stealth-box-text">
              Turn on Stealth Mode above to hide your hotkeys from other apps and hide the window from screen sharing, recordings, screenshots, the Dock (macOS) and the Taskbar (Windows).
            </div>
            {restartNote}
          </div>
        ) : (
          <div className="stealth-box is-locked">
            <div className="stealth-box-title">🔒 Pro Feature: Stealth Mode</div>
            <div className="stealth-box-text">
              Upgrade to Pro to hide your hotkeys and keep the window out of screen shares, recordings, screenshots, the Dock and the Taskbar.
            </div>
          </div>
        )}
      </div>

      {isWindows && (
        <div className="app-settings-group">
          <div className="app-settings-label stealth-heading">
            Run as Administrator
            {!isPro && <span className="pro-only-tag">Pro Only</span>}
          </div>
          <div className="app-settings-desc">
            Automatically prompts for Windows elevation at launch so Lockdown Browser text capture can read protected windows.
          </div>
          <div className="stealth-toggle-row">
            <label className={`toggle-switch ${!isPro ? 'disabled' : ''}`}>
              <input
                type="checkbox"
                checked={isPro && runAsAdmin}
                disabled={!isPro}
                onChange={(e) => isPro && onRunAsAdminChange(e.target.checked)}
              />
              <span className="toggle-switch-slider" />
            </label>
            <span className="stealth-label">
              {!isPro
                ? 'Disabled (Pro Only)'
                : runAsAdmin
                  ? (isElevated ? 'Enabled (Elevated)' : 'Enabled (Restart required)')
                  : (isElevated ? 'Disabled (Currently Elevated)' : 'Disabled')}
            </span>
          </div>

          {isPro ? (
            <div className={`stealth-box ${isElevated ? 'is-active' : 'is-inactive'}`}>
              <div className="stealth-box-title">
                {isElevated ? '🛡️ Running as Administrator' : '🛡️ Running with Standard Privileges'}
              </div>
              <div className="stealth-box-text">
                {runAsAdmin
                  ? (isElevated
                    ? 'The app will automatically request Windows administrator elevation on future launches.'
                    : 'Administrator mode is saved. Restart the app to trigger the Windows elevation prompt.')
                  : (isElevated
                    ? 'Administrator mode is turned off. Quit and reopen the app to return to standard privileges.'
                    : 'Enable Run as Administrator if Lockdown Browser blocks text capture.')}
              </div>
              {runAsAdmin && !isElevated && (
                <div className="stealth-actions">
                  <button type="button" className="stealth-btn primary" onClick={onRestartNow}>
                    🔄 Restart Now
                  </button>
                </div>
              )}
              {adminRestartStatus && (
                <div className="stealth-note">{adminRestartStatus}</div>
              )}
            </div>
          ) : (
            <div className="stealth-box is-locked">
              <div className="stealth-box-title">🔒 Pro Feature: Run as Administrator</div>
              <div className="stealth-box-text">
                Upgrade to Pro to launch elevated on Windows for Lockdown Browser text capture.
              </div>
            </div>
          )}
        </div>
      )}

      <div className="app-settings-group">
        <SetupChecklist variant="panel" {...setupChecklist} />
      </div>
    </>
  );
}
