import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

const NON_TEXT_INPUTS = new Set(['checkbox', 'radio', 'button', 'submit', 'reset', 'range', 'color', 'file', 'image']);

const isTextField = (el: EventTarget | null): el is HTMLElement => {
  if (!(el instanceof HTMLElement)) return false;
  if (el.isContentEditable || el.tagName === 'TEXTAREA') return true;
  return el.tagName === 'INPUT' && !NON_TEXT_INPUTS.has((el as HTMLInputElement).type);
};

/**
 * On Windows stealth mode the window never activates (so clicks don't take focus from the exam
 * tab), which also blocks typing. While a text field is in use, ask the backend to let the window
 * take keyboard focus; restore non-activating when focus leaves text fields (including when the
 * user clicks back into another app). The backend ignores this on macOS and outside stealth mode.
 */
export function useKeyboardInputMode() {
  useEffect(() => {
    let enabled = false;
    const setMode = (on: boolean) => {
      enabled = on;
      return invoke('set_keyboard_input_mode', { enabled: on });
    };

    const onPointerDown = (e: PointerEvent) => {
      const target = e.target;
      if (enabled || !isTextField(target)) return;
      setMode(true)
        .then(() => target.focus())
        .catch(() => { enabled = false; });
    };

    const onFocusOut = (e: FocusEvent) => {
      if (enabled && !isTextField(e.relatedTarget)) setMode(false).catch(() => {});
    };

    document.addEventListener('pointerdown', onPointerDown, true);
    document.addEventListener('focusout', onFocusOut, true);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown, true);
      document.removeEventListener('focusout', onFocusOut, true);
    };
  }, []);
}
