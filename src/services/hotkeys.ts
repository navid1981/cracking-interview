export interface HotkeysConfig {
  text: string;
  screenshot: string;
  audio_toggle: string;
  scroll_up: string;
  scroll_down: string;
  move_up: string;
  move_down: string;
  move_left: string;
  move_right: string;
  toggle_visibility: string;
  quit_app: string;
}

const MAC_SYMBOLS: Record<string, string> = {
  cmd: '⌘', command: '⌘', commandorcontrol: '⌘', cmdorctrl: '⌘', super: '⌘', meta: '⌘',
  shift: '⇧', alt: '⌥', option: '⌥', ctrl: '⌃', control: '⌃',
  up: '↑', down: '↓', left: '←', right: '→',
};

/** "Cmd+Shift+1" → "⌘⇧1" on macOS, "Ctrl+Shift+1" elsewhere. */
export function formatHotkey(hotkey: string, isMac: boolean): string {
  if (!hotkey) return '';
  const parts = hotkey.split('+').map(p => p.trim()).filter(Boolean);
  if (isMac) return parts.map(p => MAC_SYMBOLS[p.toLowerCase()] ?? p.toUpperCase()).join('');
  return parts
    .map(p => (/^(cmd|command|commandorcontrol|cmdorctrl)$/i.test(p) ? 'Ctrl' : p))
    .join('+');
}
