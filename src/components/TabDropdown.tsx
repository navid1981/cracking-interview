// Custom Dropdown with Thumbnails - Supports Chrome tabs and Displays
import { useState, useRef, useEffect, type ReactNode } from 'react';
import './TabDropdown.css';

interface ChromeTab {
  id: string;
  url: string;
  title: string;
  tab_type: string;
  thumbnail?: string;
}

interface DisplayInfo {
  id: string;
  name: string;
  width: number;
  height: number;
  is_main: boolean;
  thumbnail?: string;
}

interface AudioSource {
  id: string;
  name: string;
  source_type: 'audio';
  thumbnail?: string;
}

type InputSource = ChromeTab | DisplayInfo | AudioSource;

function isDisplay(source: InputSource): source is DisplayInfo {
  return 'width' in source && 'height' in source;
}

function isAudio(source: InputSource): source is AudioSource {
  return (source as any).source_type === 'audio';
}

function decodeHtmlEntities(text: string): string {
  const div = document.createElement('div');
  div.innerHTML = text;
  return div.textContent || div.innerText || text;
}

function ChromeIcon() {
  return (
    <svg className="chrome-icon" viewBox="0 0 48 48" width="13" height="13" aria-hidden="true">
      <path fill="#4caf50" d="M44,24c0,11.044-8.956,20-20,20S4,35.044,4,24S12.956,4,24,4S44,12.956,44,24z" />
      <path fill="#ffc107" d="M24,4v20l8,4l-8.843,16c0.317,0,0.526,0,0.843,0c11.053,0,20-8.947,20-20S35.053,4,24,4z" />
      <path fill="#f44336" d="M41.84,15H24v13l-3-1L7.16,13.26H7.14C10.68,7.69,16.91,4,24,4C31.8,4,38.55,8.48,41.84,15z" />
      <path fill="#dd2c00" d="M7.158,13.264l8.843,14.862L21,27L7.158,13.264z" />
      <path fill="#558b2f" d="M23.157,44l8.934-16.059L28,25L23.157,44z" />
      <path fill="#f9a825" d="M41.865,15H24l-1.579,4.58L41.865,15z" />
      <path fill="#fff" d="M33,24c0,4.969-4.031,9-9,9s-9-4.031-9-9s4.031-9,9-9S33,19.031,33,24z" />
      <path fill="#2196f3" d="M31,24c0,3.867-3.133,7-7,7s-7-3.133-7-7s3.133-7,7-7S31,20.133,31,24z" />
    </svg>
  );
}

/** Dropdown sections, in display order. */
const SOURCE_GROUPS: ReadonlyArray<{ key: 'displays' | 'audio' | 'chrome'; icon: ReactNode; label: string }> = [
  { key: 'displays', icon: '🖥', label: 'Displays' },
  { key: 'audio', icon: '🎧', label: 'Audio' },
  { key: 'chrome', icon: <ChromeIcon />, label: 'Chrome tabs' },
];

function sourceGroup(source: InputSource): number {
  if (isDisplay(source)) return 0;
  if (isAudio(source)) return 1;
  return 2;
}

function getSourceTitle(source: InputSource): string {
  if (isDisplay(source)) {
    return `${source.name} (${source.width}x${source.height})${source.is_main ? ' - Main' : ''}`;
  }
  if (isAudio(source)) {
    return source.name;
  }
  return decodeHtmlEntities(source.title);
}

function getSourceSubtitle(source: InputSource): string {
  if (isDisplay(source)) {
    return source.is_main ? 'Main Display' : 'External Display';
  }
  if (isAudio(source)) {
    return 'Listens live to system audio (Zoom, Teams, Meet)';
  }
  return decodeHtmlEntities(source.url);
}

interface Props {
  sources: InputSource[];
  selectedSource: InputSource | null;
  onSelect: (source: InputSource) => void;
  disabled?: boolean;
  onOpen?: () => void;
  /** When false, the Chrome tabs section shows a "connect" row instead of disappearing. */
  chromeConnected?: boolean;
  onOpenChrome?: () => void;
  /** Free users see a PRO badge on Pro-only sources (Audio). */
  isPro?: boolean;
}

export default function TabDropdown({ sources, selectedSource, onSelect, disabled, onOpen, chromeConnected = true, onOpenChrome, isPro = true }: Props) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  return (
    <div className="tab-dropdown" ref={dropdownRef}>
      <button
        className={`dropdown-trigger ${isOpen ? 'open' : ''}`}
        onClick={() => {
          if (disabled) return;
          if (!isOpen) onOpen?.();
          setIsOpen(!isOpen);
        }}
        disabled={disabled}
      >
        <div className="trigger-content">
          {selectedSource ? (
            <>
              {selectedSource.thumbnail && (
                <img src={selectedSource.thumbnail} alt="" className="trigger-thumbnail" />
              )}
              {!selectedSource.thumbnail && isAudio(selectedSource) && (
                <span className="trigger-audio-icon">🎧</span>
              )}
              <span className="trigger-text">{getSourceTitle(selectedSource)}</span>
            </>
          ) : (
            <span className="trigger-placeholder">
              {disabled ? 'Open Chrome CDP first' : 'Select a source...'}
            </span>
          )}
        </div>
        <span className="dropdown-arrow">▼</span>
      </button>

      {isOpen && !disabled && (
        <div className="dropdown-menu">
          {sources.length === 0 && chromeConnected ? (
            <div className="dropdown-empty">No sources available</div>
          ) : (
            SOURCE_GROUPS.map((group, groupIndex) => {
              const items = sources.filter(s => sourceGroup(s) === groupIndex);
              const showChromePlaceholder = group.key === 'chrome' && !chromeConnected;
              if (items.length === 0 && !showChromePlaceholder) return null;
              return (
                <div key={group.key} className="dropdown-group" role="group" aria-label={group.label}>
                  <div className="dropdown-group-header">
                    <span className="dropdown-group-icon" aria-hidden="true">{group.icon}</span>
                    <span>{group.label}</span>
                    {items.length > 1 && <span className="dropdown-group-count">· {items.length}</span>}
                  </div>
                  {showChromePlaceholder ? (
                    <div className="dropdown-placeholder">
                      <span>Chrome not connected</span>
                      {onOpenChrome && (
                        <button
                          type="button"
                          className="dropdown-placeholder-btn"
                          onClick={() => {
                            onOpenChrome();
                            setIsOpen(false);
                          }}
                        >
                          Open Chrome
                        </button>
                      )}
                    </div>
                  ) : (
                    items.map((source) => (
                      <div
                        key={source.id}
                        className={`dropdown-item ${selectedSource?.id === source.id ? 'selected' : ''}`}
                        onClick={() => {
                          onSelect(source);
                          setIsOpen(false);
                        }}
                      >
                        {source.thumbnail && (
                          <img src={source.thumbnail} alt="" className="item-thumbnail" />
                        )}
                        {!source.thumbnail && isAudio(source) && (
                          <span className="item-audio-icon" aria-hidden="true">🎧</span>
                        )}
                        <div className="item-content">
                          <div className="item-title">
                            {getSourceTitle(source)}
                            {!isPro && isAudio(source) && <span className="source-pro-badge">PRO</span>}
                          </div>
                          <div className="item-url">{getSourceSubtitle(source)}</div>
                        </div>
                        {selectedSource?.id === source.id && (
                          <span className="checkmark">✓</span>
                        )}
                      </div>
                    ))
                  )}
                </div>
              );
            })
          )}
        </div>
      )}
    </div>
  );
}
