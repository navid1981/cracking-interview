import type { HotkeysConfig } from '../../services/hotkeys';
import HotkeyKeys from '../HotkeyKeys';

type Platform = 'macos' | 'windows' | 'linux' | 'unknown';

interface Field {
  key: keyof HotkeysConfig;
  label: string;
}

const SECTIONS: Array<{ title: string; fields: Field[] }> = [
  {
    title: '🎯 Solve',
    fields: [
      { key: 'text', label: 'Chrome Content → Solve' },
      { key: 'screenshot', label: 'Screenshot → Solve' },
      { key: 'audio_toggle', label: 'Start Listening / Stop & Solve' },
      { key: 'capture_selection', label: 'Capture Selected Text → Solve' },
    ],
  },
  {
    title: '🧭 Navigation',
    fields: [
      { key: 'scroll_up', label: 'Scroll up (Explanation)' },
      { key: 'scroll_down', label: 'Scroll down (Explanation)' },
      { key: 'move_up', label: 'Move window up' },
      { key: 'move_down', label: 'Move window down' },
      { key: 'move_left', label: 'Move window left' },
      { key: 'move_right', label: 'Move window right' },
    ],
  },
  {
    title: '⚙️ App',
    fields: [
      { key: 'toggle_visibility', label: 'Show/Hide app window' },
      { key: 'quit_app', label: 'Quit app' },
    ],
  },
];

interface Props {
  draft: HotkeysConfig;
  onDraftChange: (draft: HotkeysConfig) => void;
  platform: Platform;
  /** Platform defaults from Rust (`get_default_hotkeys`), used for placeholders. */
  defaults: HotkeysConfig | null;
  status: string;
  onSave: () => void;
  onReset: () => void;
}

export default function HotkeysSettingsTab({ draft, onDraftChange, platform, defaults, status, onSave, onReset }: Props) {
  const isMac = platform === 'macos';
  return (
    <div className="hotkeys-panel">
      {SECTIONS.map(section => (
        <div className="hotkeys-section" key={section.title}>
          <div className="hotkeys-section-title">{section.title}</div>
          <div className="hotkeys-two-col">
            {section.fields.map(field => (
              <div className="hotkey-field" key={field.key}>
                <div className="hotkey-label">
                  <span>{field.label}</span>
                  {/* Live preview in the same style used everywhere else in the app */}
                  <HotkeyKeys hotkey={draft[field.key] || defaults?.[field.key] || ''} isMac={isMac} />
                </div>
                <input
                  className="input-field hotkey-input"
                  value={draft[field.key]}
                  onChange={(e) => onDraftChange({ ...draft, [field.key]: e.target.value })}
                  placeholder={defaults?.[field.key] ?? ''}
                />
              </div>
            ))}
          </div>
        </div>
      ))}

      <div className="hotkeys-actions">
        <button className="action-btn primary" onClick={onSave}>Save</button>
        <button className="action-btn secondary" onClick={onReset}>Reset to defaults</button>
      </div>

      {status && <div className={`hotkeys-status ${status.startsWith('❌') ? 'is-error' : ''}`}>{status}</div>}

      <p className="hotkeys-footnote">
        Display Input Source auto-uses Screenshot even with the Chrome Content hotkey. Avoid Shift-only shortcuts (e.g. Shift+L).
      </p>
    </div>
  );
}
