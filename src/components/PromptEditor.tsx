import { useState, useEffect } from 'react';
import {
  CustomPromptsManager,
  PromptTemplate,
  isBuiltInTemplate,
  getTemplateLabel,
  getDefaultSystemPromptForTemplate,
  getDefaultUserTemplate,
} from '../services/prompts';
import './PromptEditor.css';

interface PromptEditorProps {
  currentTemplateId: string;
  onCancel: () => void;
  onSaved?: () => void;
}

export default function PromptEditor({ currentTemplateId, onCancel, onSaved }: PromptEditorProps) {
  const [systemPrompt, setSystemPrompt] = useState('');
  const [userPrompt, setUserPrompt] = useState('');
  const [supportsLanguage, setSupportsLanguage] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    loadPromptForTemplate(currentTemplateId);
  }, [currentTemplateId]);

  const loadPromptForTemplate = (templateId: string) => {
    if (isBuiltInTemplate(templateId)) {
      const overriddenSystem = localStorage.getItem(`custom_${templateId}_system`);
      const overriddenUser = localStorage.getItem(`custom_${templateId}_user`);
      setSystemPrompt(overriddenSystem || getDefaultSystemPromptForTemplate(templateId as PromptTemplate));
      setUserPrompt(overriddenUser || getDefaultUserTemplate(templateId as PromptTemplate));
      setSupportsLanguage(false);
    } else {
      const customPrompt = CustomPromptsManager.getById(templateId);
      if (customPrompt) {
        setSystemPrompt(customPrompt.systemPrompt);
        setUserPrompt(customPrompt.userPrompt);
        setSupportsLanguage(customPrompt.supportsLanguage);
      }
    }
  };

  const isBuiltIn = isBuiltInTemplate(currentTemplateId);
  const promptTitle = getTemplateLabel(currentTemplateId);

  const handleSave = async () => {
    if (isSaving) return;
    setIsSaving(true);
    try {
      if (isBuiltIn) {
        // Persist built-in overrides
        localStorage.setItem(`custom_${currentTemplateId}_system`, systemPrompt);
        localStorage.setItem(`custom_${currentTemplateId}_user`, userPrompt);
      } else {
        // Update the custom prompt in place
        const existingPrompt = CustomPromptsManager.getById(currentTemplateId);
        if (!existingPrompt) {
          throw new Error('Custom prompt not found');
        }
        CustomPromptsManager.save({
          ...existingPrompt,
          systemPrompt,
          userPrompt,
          supportsLanguage,
        });
      }
      onSaved?.();
      onCancel(); // Return to list after saving
    } catch (e: any) {
      alert(`❌ Failed to save prompt: ${e?.message || e}`);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="prompt-editor">
      <div className="prompt-editor-header">
        <div className="prompt-editor-title">
          <h3>Editing: {promptTitle}</h3>
        </div>
      </div>

      <div className="editor-section">
        <div className="section-header">
          <h3>🤖 System Prompt</h3>
          <p className="help-text">Instructions that guide the AI's behavior</p>
        </div>
        <textarea
          value={systemPrompt}
          onChange={(e) => setSystemPrompt(e.target.value)}
          className="prompt-textarea"
          rows={8}
          placeholder="Enter system instructions..."
        />
      </div>

      <div className="editor-section">
        <div className="section-header">
          <h3>👤 User Prompt</h3>
          <p className="help-text">
            Use placeholders: {'{CONTENT}'}, {currentTemplateId === PromptTemplate.VerbalInterviewAudio ? '{INTERVIEW_LANGUAGE}' : '{LANGUAGE}'}, {'{DOC_NAME}'}
          </p>
        </div>
        <textarea
          value={userPrompt}
          onChange={(e) => setUserPrompt(e.target.value)}
          className="prompt-textarea"
          rows={6}
          placeholder="Enter user prompt template..."
        />
      </div>

      {!isBuiltIn && (
        <div className="editor-section">
          <div className="checkbox-container">
            <input
              type="checkbox"
              id="supportsLanguage"
              checked={supportsLanguage}
              onChange={(e) => setSupportsLanguage(e.target.checked)}
            />
            <label htmlFor="supportsLanguage">
              Supports language selection (uses {'{LANGUAGE}'} placeholder)
            </label>
          </div>
        </div>
      )}

      <div className="editor-actions">
        <button onClick={onCancel} className="action-btn secondary" disabled={isSaving}>
          Cancel
        </button>
        <button onClick={handleSave} className="action-btn primary" disabled={isSaving}>
          {isSaving ? 'Saving...' : 'Save'}
        </button>
      </div>
    </div>
  );
}
