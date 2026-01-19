import { useState, useEffect } from 'react';
import {
  CustomPromptsManager,
  CustomPrompt,
  PromptTemplate,
  isBuiltInTemplate,
  getDefaultSystemPrompt,
  getDefaultUserTemplate,
} from '../services/prompts';
import './PromptEditor.css';

interface PromptEditorProps {
  currentTemplateId: string;
  onCustomPromptChange?: () => void;
}

export default function PromptEditor({ currentTemplateId, onCustomPromptChange }: PromptEditorProps) {
  const [systemPrompt, setSystemPrompt] = useState('');
  const [userPrompt, setUserPrompt] = useState('');
  const [promptName, setPromptName] = useState('');
  const [showSaveDialog, setShowSaveDialog] = useState(false);
  const [editingCustomPromptId, setEditingCustomPromptId] = useState<string | null>(null);
  const [supportsLanguage, setSupportsLanguage] = useState(false);
  const [customPrompts, setCustomPrompts] = useState<CustomPrompt[]>([]);

  useEffect(() => {
    loadPromptForTemplate(currentTemplateId);
    loadCustomPrompts();
  }, [currentTemplateId]);

  const loadCustomPrompts = () => {
    setCustomPrompts(CustomPromptsManager.getAll());
  };

  const loadPromptForTemplate = (templateId: string) => {
    if (isBuiltInTemplate(templateId)) {
      setSystemPrompt(getDefaultSystemPrompt());
      setUserPrompt(getDefaultUserTemplate(templateId as PromptTemplate));
      setEditingCustomPromptId(null);
      setPromptName('');
      setSupportsLanguage(false);
    } else {
      const customPrompt = CustomPromptsManager.getById(templateId);
      if (customPrompt) {
        setSystemPrompt(customPrompt.systemPrompt);
        setUserPrompt(customPrompt.userPrompt);
        setEditingCustomPromptId(customPrompt.id);
        setPromptName(customPrompt.name);
        setSupportsLanguage(customPrompt.supportsLanguage);
      }
    }
  };

  const saveAsNewCustomPrompt = () => {
    if (!promptName.trim()) {
      alert('Please enter a name for your custom prompt');
      return;
    }

    if (!editingCustomPromptId) {
      const existing = CustomPromptsManager.getAll();
      if (existing.some(p => p.name.toLowerCase() === promptName.trim().toLowerCase())) {
        alert('A custom prompt with this name already exists. Please choose a different name.');
        return;
      }
    }

    const newPrompt: CustomPrompt = {
      id: Date.now().toString(),
      name: promptName.trim(),
      systemPrompt,
      userPrompt,
      supportsLanguage,
    };

    CustomPromptsManager.save(newPrompt);
    setShowSaveDialog(false);
    setPromptName('');
    loadCustomPrompts();
    alert(`✅ Custom prompt "${newPrompt.name}" saved!`);
    
    if (onCustomPromptChange) {
      onCustomPromptChange();
    }
  };

  const updateExistingCustomPrompt = () => {
    if (!editingCustomPromptId) return;

    const existingPrompt = CustomPromptsManager.getById(editingCustomPromptId);
    if (!existingPrompt) return;

    const updatedPrompt: CustomPrompt = {
      ...existingPrompt,
      systemPrompt,
      userPrompt,
      supportsLanguage,
    };

    CustomPromptsManager.save(updatedPrompt);
    loadCustomPrompts();
    alert(`✅ Custom prompt "${existingPrompt.name}" updated!`);
    
    if (onCustomPromptChange) {
      onCustomPromptChange();
    }
  };

  const deleteCurrentCustomPrompt = () => {
    if (!editingCustomPromptId) return;

    const existingPrompt = CustomPromptsManager.getById(editingCustomPromptId);
    if (!existingPrompt) return;

    if (confirm(`Delete custom prompt "${existingPrompt.name}"?`)) {
      CustomPromptsManager.delete(editingCustomPromptId);
      loadCustomPrompts();
      alert(`✅ Custom prompt "${existingPrompt.name}" deleted!`);
      
      if (onCustomPromptChange) {
        onCustomPromptChange();
      }
    }
  };

  const loadCustomPromptInEditor = (prompt: CustomPrompt) => {
    setSystemPrompt(prompt.systemPrompt);
    setUserPrompt(prompt.userPrompt);
    setSupportsLanguage(prompt.supportsLanguage);
    setEditingCustomPromptId(prompt.id);
    setPromptName(prompt.name);
  };

  const resetToDefault = () => {
    if (confirm('Reset to default prompts? This will discard your changes.')) {
      loadPromptForTemplate(currentTemplateId);
    }
  };

  const isBuiltIn = isBuiltInTemplate(currentTemplateId);

  return (
    <div className="prompt-editor">
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
          <h3>👤 User Prompt Template</h3>
          <p className="help-text">
            Use placeholders: {'{CONTENT}'}, {'{LANGUAGE}'}, {'{PROBLEM}'}
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
        {isBuiltIn && (
          <>
            <button onClick={resetToDefault} className="action-btn secondary">
              🔄 Reset
            </button>
            <button onClick={() => setShowSaveDialog(true)} className="action-btn primary">
              💾 Save as Custom
            </button>
          </>
        )}

        {!isBuiltIn && editingCustomPromptId && (
          <>
            <button onClick={deleteCurrentCustomPrompt} className="action-btn danger">
              🗑️ Delete
            </button>
            <button onClick={updateExistingCustomPrompt} className="action-btn primary">
              💾 Update
            </button>
          </>
        )}
      </div>

      {showSaveDialog && (
        <div className="save-dialog">
          <h4>Save as Custom Prompt</h4>
          <input
            type="text"
            value={promptName}
            onChange={(e) => setPromptName(e.target.value)}
            placeholder="Enter prompt name..."
            className="input-field"
            autoFocus
          />
          <div className="checkbox-container" style={{ marginTop: '12px' }}>
            <input
              type="checkbox"
              id="newSupportsLanguage"
              checked={supportsLanguage}
              onChange={(e) => setSupportsLanguage(e.target.checked)}
            />
            <label htmlFor="newSupportsLanguage">
              Supports language selection
            </label>
          </div>
          <div className="dialog-actions">
            <button onClick={() => {
              setShowSaveDialog(false);
              setPromptName('');
            }} className="action-btn secondary">
              Cancel
            </button>
            <button onClick={saveAsNewCustomPrompt} className="action-btn primary">
              Save
            </button>
          </div>
        </div>
      )}

      {customPrompts.length > 0 && (
        <div className="custom-prompts-section">
          <h3>📚 Your Custom Prompts</h3>
          <div className="custom-prompts-list">
            {customPrompts.map((prompt) => (
              <div key={prompt.id} className="custom-prompt-item">
                <span className="prompt-name">{prompt.name}</span>
                <div className="prompt-actions">
                  <button
                    onClick={() => loadCustomPromptInEditor(prompt)}
                    className="action-btn-small primary"
                  >
                    Load
                  </button>
                  <button
                    onClick={() => {
                      if (confirm(`Delete "${prompt.name}"?`)) {
                        CustomPromptsManager.delete(prompt.id);
                        loadCustomPrompts();
                        if (onCustomPromptChange) onCustomPromptChange();
                      }
                    }}
                    className="action-btn-small danger"
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
