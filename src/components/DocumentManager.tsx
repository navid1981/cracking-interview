import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './DocumentManager.css';

export interface DocumentPlaceholder {
  id: string;
  name: string;
  display_name: string;
  file_name: string;
  extracted_text: string;
  created_at: string;
}

interface SlotState {
  id: string;
  placeholderName: string;
  fileName: string;
  extractedText: string;
  charCount: number;
  status: 'empty' | 'extracting' | 'ready' | 'error';
  error: string;
}

function emptySlot(): SlotState {
  return {
    id: '',
    placeholderName: '',
    fileName: '',
    extractedText: '',
    charCount: 0,
    status: 'empty',
    error: '',
  };
}

const RESERVED_NAMES = ['LANGUAGE', 'CONTENT', 'PROBLEM', 'INTERVIEW_LANGUAGE'];

interface DocumentManagerProps {
  onClose: () => void;
  existingPlaceholders: DocumentPlaceholder[];
  onSaved: (placeholders: DocumentPlaceholder[]) => void;
}

export default function DocumentManager({ onClose, existingPlaceholders, onSaved }: DocumentManagerProps) {
  const [slots, setSlots] = useState<SlotState[]>(() => {
    const initial: SlotState[] = [];
    for (let i = 0; i < 3; i++) {
      const existing = existingPlaceholders[i];
      if (existing) {
        initial.push({
          id: existing.id,
          placeholderName: existing.display_name,
          fileName: existing.file_name,
          extractedText: existing.extracted_text,
          charCount: existing.extracted_text.length,
          status: 'ready',
          error: '',
        });
      } else {
        initial.push(emptySlot());
      }
    }
    return initial;
  });
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState('');
  const fileInputRefs = [useRef<HTMLInputElement>(null), useRef<HTMLInputElement>(null), useRef<HTMLInputElement>(null)];

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  const updateSlot = (index: number, updates: Partial<SlotState>) => {
    setSlots(prev => prev.map((s, i) => i === index ? { ...s, ...updates } : s));
  };

  const toPlaceholderKey = (name: string): string => {
    return name.trim().toUpperCase().replace(/\s+/g, '_').replace(/[^A-Z0-9_]/g, '');
  };

  const handleFileSelect = async (index: number, file: File) => {
    updateSlot(index, { fileName: file.name, status: 'extracting', error: '' });

    try {
      const arrayBuffer = await file.arrayBuffer();
      const bytes = new Uint8Array(arrayBuffer);
      let binary = '';
      const chunkSize = 8192;
      for (let i = 0; i < bytes.length; i += chunkSize) {
        binary += String.fromCharCode(...bytes.subarray(i, i + chunkSize));
      }
      const base64 = btoa(binary);

      const text = await invoke<string>('extract_document_text', {
        fileName: file.name,
        fileDataBase64: base64,
      });

      updateSlot(index, {
        extractedText: text,
        charCount: text.length,
        status: 'ready',
        id: slots[index].id || `doc_${Date.now()}_${index}`,
      });
    } catch (err: any) {
      updateSlot(index, {
        status: 'error',
        error: typeof err === 'string' ? err : err.message || 'Extraction failed',
        extractedText: '',
        charCount: 0,
      });
    }
  };

  const handleClearSlot = (index: number) => {
    setSlots(prev => prev.map((s, i) => i === index ? emptySlot() : s));
    if (fileInputRefs[index].current) {
      fileInputRefs[index].current!.value = '';
    }
  };

  const validate = (): string | null => {
    const activeSlots = slots.filter(s => s.status === 'ready' || s.fileName);
    const names = new Set<string>();

    for (let i = 0; i < slots.length; i++) {
      const s = slots[i];
      if (s.status === 'empty' && !s.fileName) continue;

      if (!s.placeholderName.trim()) {
        return `Slot ${i + 1}: Please enter a placeholder name.`;
      }

      const key = toPlaceholderKey(s.placeholderName);
      if (!key) {
        return `Slot ${i + 1}: Placeholder name must contain letters, numbers, or underscores.`;
      }

      if (RESERVED_NAMES.includes(key)) {
        return `Slot ${i + 1}: "${key}" is reserved. Choose a different name.`;
      }

      if (names.has(key)) {
        return `Duplicate placeholder name: {${key}}`;
      }
      names.add(key);

      if (s.status !== 'ready') {
        return `Slot ${i + 1}: File not ready. Please wait or choose a different file.`;
      }
    }

    return null;
  };

  const handleSave = async () => {
    const error = validate();
    if (error) {
      setSaveError(error);
      return;
    }

    setSaving(true);
    setSaveError('');

    try {
      const activePlaceholders: DocumentPlaceholder[] = [];

      for (const oldPlaceholder of existingPlaceholders) {
        const stillExists = slots.some(s => s.id === oldPlaceholder.id && s.status === 'ready');
        if (!stillExists) {
          await invoke('delete_document_placeholder', { id: oldPlaceholder.id });
        }
      }

      for (const s of slots) {
        if (s.status !== 'ready') continue;

        const key = toPlaceholderKey(s.placeholderName);
        const placeholder: DocumentPlaceholder = {
          id: s.id || `doc_${Date.now()}`,
          name: key,
          display_name: s.placeholderName.trim(),
          file_name: s.fileName,
          extracted_text: s.extractedText,
          created_at: new Date().toISOString(),
        };

        await invoke('save_document_placeholder', {
          id: placeholder.id,
          name: placeholder.name,
          displayName: placeholder.display_name,
          fileName: placeholder.file_name,
          extractedText: placeholder.extracted_text,
        });

        activePlaceholders.push(placeholder);
      }

      onSaved(activePlaceholders);
      onClose();
    } catch (err: any) {
      setSaveError(typeof err === 'string' ? err : err.message || 'Failed to save');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="doc-manager-overlay" onClick={onClose}>
      <div className="doc-manager-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="doc-manager-header">
          <h3>Document Placeholders</h3>
          <button className="doc-manager-close" onClick={onClose}>&times;</button>
        </div>

        <p className="doc-manager-hint">
          Upload documents and assign <code>{'{PLACEHOLDER}'}</code> names to use in your prompts.
        </p>

        <div className="doc-slots">
          {slots.map((slot, idx) => (
            <div key={idx} className={`doc-slot ${slot.status}`}>
              <div className="doc-slot-header">
                <span className="doc-slot-brace">{'{'}</span>
                <input
                  type="text"
                  className="doc-slot-name"
                  placeholder="PLACEHOLDER_NAME"
                  value={slot.placeholderName}
                  onChange={(e) => updateSlot(idx, { placeholderName: e.target.value })}
                  disabled={saving}
                />
                <span className="doc-slot-brace">{'}'}</span>
                {(slot.status !== 'empty') && (
                  <button
                    className="doc-slot-clear"
                    onClick={() => handleClearSlot(idx)}
                    disabled={saving}
                    title="Remove"
                  >
                    &times;
                  </button>
                )}
              </div>

              <div className="doc-slot-file-row">
                <input
                  ref={fileInputRefs[idx]}
                  type="file"
                  accept=".pdf,.docx,.doc,.txt,.text,.md"
                  className="doc-slot-file-input"
                  id={`doc-file-${idx}`}
                  onChange={(e) => {
                    const file = e.target.files?.[0];
                    if (file) handleFileSelect(idx, file);
                  }}
                  disabled={saving}
                />
                <label htmlFor={`doc-file-${idx}`} className="doc-slot-file-btn">
                  Choose File
                </label>
                <span className="doc-slot-file-name">
                  {slot.fileName || 'No file chosen'}
                </span>
              </div>

              {slot.status === 'extracting' && (
                <div className="doc-slot-status extracting">Extracting text...</div>
              )}
              {slot.status === 'ready' && (
                <div className="doc-slot-status ready">
                  Extracted ({slot.charCount.toLocaleString()} chars)
                  {slot.charCount >= 50000 && <span className="doc-truncated"> — truncated</span>}
                </div>
              )}
              {slot.status === 'error' && (
                <div className="doc-slot-status error">{slot.error}</div>
              )}
            </div>
          ))}
        </div>

        {saveError && (
          <div className="doc-save-error">{saveError}</div>
        )}

        <div className="doc-manager-footer">
          <p className="doc-manager-tip">
            Use <code>{'{MY_RESUME}'}</code> in any prompt to inject your document text.
          </p>
          <div className="doc-manager-actions">
            <button className="dialog-btn cancel" onClick={onClose} disabled={saving}>
              Cancel
            </button>
            <button className="dialog-btn confirm" onClick={handleSave} disabled={saving}>
              {saving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
