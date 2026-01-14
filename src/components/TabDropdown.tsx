// Custom Dropdown with Thumbnails - Supports Chrome tabs and Displays
import { useState, useRef, useEffect } from 'react';
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

type InputSource = ChromeTab | DisplayInfo;

function isDisplay(source: InputSource): source is DisplayInfo {
  return 'width' in source && 'height' in source;
}

function getSourceTitle(source: InputSource): string {
  if (isDisplay(source)) {
    return `${source.name} (${source.width}x${source.height})${source.is_main ? ' - Main' : ''}`;
  }
  return source.title;
}

function getSourceSubtitle(source: InputSource): string {
  if (isDisplay(source)) {
    return source.is_main ? 'Main Display' : 'External Display';
  }
  return source.url;
}

interface Props {
  sources: InputSource[];
  selectedSource: InputSource | null;
  onSelect: (source: InputSource) => void;
  disabled?: boolean;
}

export default function TabDropdown({ sources, selectedSource, onSelect, disabled }: Props) {
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
        onClick={() => !disabled && setIsOpen(!isOpen)}
        disabled={disabled}
      >
        <div className="trigger-content">
          {selectedSource ? (
            <>
              {selectedSource.thumbnail && (
                <img src={selectedSource.thumbnail} alt="" className="trigger-thumbnail" />
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
          {sources.length === 0 ? (
            <div className="dropdown-empty">No sources available</div>
          ) : (
            sources.map((source) => (
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
                <div className="item-content">
                  <div className="item-title">{getSourceTitle(source)}</div>
                  <div className="item-url">{getSourceSubtitle(source)}</div>
                </div>
                {selectedSource?.id === source.id && (
                  <span className="checkmark">✓</span>
                )}
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}
