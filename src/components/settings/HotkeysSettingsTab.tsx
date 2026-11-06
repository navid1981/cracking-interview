import type { HotkeysConfig } from '../../services/hotkeys';

type Platform = 'macos' | 'windows' | 'linux' | 'unknown';

interface Field {
  key: keyof HotkeysConfig;
  label: string;
  /** Placeholder suffix after the platform modifier(s). */
  keys: string;
  shift?: boolean;
  /** Scroll keys use Ctrl on Windows instead of Alt. */
  ctrlOnWindows?: boolean;
}

const SECTIONS: Array<{ title: string; fields: Field[] }> = [
  {
    title: '🎯 Solve',
    fields: [
      { key: 'text', label: 'Extract text → Solve', keys: '1' },
      { key: 'screenshot', label: 'Screenshot → Solve', keys: '2' },
      { key: 'audio_toggle', label: 'Audio Start/Stop → Solve', keys: '3' },
    ],
  },
  {
    title: '🧭 Navigation',
    fields: [
      { key: 'scroll_up', label: 'Scroll up (Explanation)', keys: 'Up', ctrlOnWindows: true },
      { key: 'scroll_down', label: 'Scroll down (Explanation)', keys: 'Down', ctrlOnWindows: true },
      { key: 'move_up', label: 'Move window up', keys: 'Up', shift: true },
      { key: 'move_down', label: 'Move window down', keys: 'Down', shift: true },
      { key: 'move_left', label: 'Move window left', keys: 'Left', shift: true },
      { key: 'move_right', label: 'Move window right', keys: 'Right', shift: true },
    ],
  },
  {
    title: '⚙️ App',
    fields: [
      { key: 'toggle_visibility', label: 'Show/Hide app window', keys: 'H', shift: true },
      { key: 'quit_app', label: 'Quit app', keys: 'Q', shift: true },
    ],
  },
];

function placeholder(field: Field, platform: Platform): string {
  const modifier = platform === 'macos' ? 'Command' : platform === 'windows' && !field.ctrlOnWindows ? 'Alt' : 'Ctrl';
  return [modifier, field.shift && 'Shift', field.keys].filter(Boolean).join(' + ');
}

interface Props {
  draft: HotkeysConfig;
  onDraftChange: (draft: HotkeysConfig) => void;
  platform: Platform;
  status: string;
  onSave: () => void;
  onReset: () => void;
}

export default function HotkeysSettingsTab({ draft, onDraftChange, platform, status, onSave, onReset }: Props) {
  return (
    <div className="hotkeys-panel">
      {SECTIONS.map(section => (
        <div className="hotkeys-section" key={section.title}>
          <div className="hotkeys-section-title">{section.title}</div>
          <div className="hotkeys-two-col">
            {section.fields.map(field => (
              <div className="hotkey-field" key={field.key}>
                <div className="hotkey-label">{field.label}</div>
                <input
                  className="input-field hotkey-input"
                  value={draft[field.key]}
                  onChange={(e) => onDraftChange({ ...draft, [field.key]: e.target.value })}
                  placeholder={placeholder(field, platform)}
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
        Display Input Source auto-uses Screenshot even with the Extract hotkey. Avoid Shift-only shortcuts (e.g. Shift+L).
      </p>
    </div>
  );
}
