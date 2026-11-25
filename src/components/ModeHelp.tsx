// ⓘ button beside the Content / Screenshot toggle. Opens a panel comparing the three ways to solve.
import { useEffect, useState } from 'react';
import HotkeyKeys from './HotkeyKeys';
import './ModeHelp.css';

interface Props {
  isMac: boolean;
  textHotkey?: string;
  screenshotHotkey?: string;
  selectionHotkey?: string;
  audioHotkey?: string;
  /** Free users see a PRO badge on Pro-only rows. */
  isPro?: boolean;
}

const MODES = [
  {
    icon: '📝',
    name: 'Content',
    hotkey: 'text',
    worksWith: 'Chrome tabs only',
    desc: 'Reads the whole page, including text, frames and images (diagrams, charts), even past what fits on screen. Exact text, fast, low cost.',
  },
  {
    icon: '📸',
    name: 'Screenshot',
    hotkey: 'screenshot',
    worksWith: "Any display or Chrome tab, including apps Chrome can't open",
    desc: "Captures exactly what's visible on a display or Chrome tab, from any app. Only the visible area is read.",
  },
  {
    icon: '🎧',
    name: 'Live audio',
    hotkey: 'audio',
    worksWith: 'System audio from calls (Zoom, Teams, Meet)',
    desc: 'Listens live and transcribes spoken questions. Press the hotkey again to stop and solve.',
    proOnly: true,
  },
  {
    icon: '✂️',
    name: 'Selected text',
    hotkey: 'selection',
    worksWith: 'Any app where you can highlight text, even if it hides its window from screen capture',
    desc: 'Highlight the question and press the hotkey. No Input Source needed.',
  },
] as const;

export default function ModeHelp({ isMac, textHotkey, screenshotHotkey, selectionHotkey, audioHotkey, isPro = true }: Props) {
  const [open, setOpen] = useState(false);
  const hotkeyFor = { text: textHotkey, screenshot: screenshotHotkey, selection: selectionHotkey, audio: audioHotkey };

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false); };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [open]);

  return (
    <>
      <button
        type="button"
        className="mode-help-btn"
        onClick={() => setOpen(true)}
        aria-label="Ways to solve"
        title="Ways to solve"
      >
        ⓘ
      </button>
      {open && (
        <div className="mode-help-backdrop" onClick={() => setOpen(false)}>
          <div
            className="mode-help-panel"
            role="dialog"
            aria-modal="true"
            aria-labelledby="mode-help-title"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mode-help-header">
              <h4 id="mode-help-title">Ways to solve</h4>
              <button type="button" className="mode-help-close" onClick={() => setOpen(false)} aria-label="Close">✕</button>
            </div>
            {MODES.map(m => (
              <div className="mode-help-row" key={m.name}>
                <div className="mode-help-row-title">
                  <span>{m.icon} {m.name}</span>
                  <span className="mode-help-row-meta">
                    {hotkeyFor[m.hotkey] && <HotkeyKeys hotkey={hotkeyFor[m.hotkey]!} isMac={isMac} />}
                    {'proOnly' in m && !isPro && <span className="source-pro-badge">PRO</span>}
                  </span>
                </div>
                <div className="mode-help-best"><strong>Works with:</strong> {m.worksWith}</div>
                <div className="mode-help-desc">{m.desc}</div>
              </div>
            ))}
          </div>
        </div>
      )}
    </>
  );
}
