// Shows a hotkey as separate key boxes, e.g. [⌘] + [1] on macOS or [Alt] + [1] on Windows.
// Use this everywhere a hotkey is displayed so every screen looks the same.
import { Fragment } from 'react';
import { hotkeyKeys } from '../services/hotkeys';
import './HotkeyKeys.css';

interface Props {
  hotkey: string;
  isMac: boolean;
  /** "compact" is smaller with light borders, for use inside coloured buttons. */
  size?: 'normal' | 'compact';
  /** Optional text before the keys, e.g. "Hotkey:". */
  label?: string;
  className?: string;
}

export default function HotkeyKeys({ hotkey, isMac, size = 'normal', label, className = '' }: Props) {
  const keys = hotkeyKeys(hotkey, isMac);
  if (keys.length === 0) return null;
  return (
    <span className={`hotkey-keys hotkey-keys-${size} ${className}`.trim()}>
      {label && <span className="hotkey-keys-label">{label}</span>}
      {keys.map((k, i) => (
        <Fragment key={i}>
          {i > 0 && <span className="hotkey-keys-plus">+</span>}
          <kbd>{k}</kbd>
        </Fragment>
      ))}
    </span>
  );
}
