import { useState } from 'react';
import { getAllTemplates, CustomPromptsManager, PromptTemplate, ProgrammingLanguage, supportsLanguageSelection, getDefaultSystemPrompt, getDefaultUserTemplate } from '../services/prompts';
import './PromptListView.css';

interface PromptListViewProps {
  selectedTemplate: string;
  onSelectTemplate: (templateId: string) => void;
  onEditPrompt: (templateId: string) => void;
  selectedLanguage: ProgrammingLanguage;
  onLanguageChange: (language: ProgrammingLanguage) => void;
}

export default function PromptListView({
  selectedTemplate,
  onSelectTemplate,
  onEditPrompt,
  selectedLanguage,
  onLanguageChange
}: PromptListViewProps) {
  const [templates, setTemplates] = useState(getAllTemplates());
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [showRestoreConfirm, setShowRestoreConfirm] = useState(false);
  const [showRenameDialog, setShowRenameDialog] = useState(false);
  const [showDuplicateDialog, setShowDuplicateDialog] = useState(false);
  const [showNewPromptDialog, setShowNewPromptDialog] = useState(false);
  const [showErrorDialog, setShowErrorDialog] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');
  const [targetTemplateId, setTargetTemplateId] = useState<string>('');
  const [inputValue, setInputValue] = useState('');

  const customPrompts = CustomPromptsManager.getAll();
  const canAddMore = customPrompts.length < 3;

  // Helper: Check if name already exists (case-insensitive)
  const nameExists = (name: string, excludeId?: string): boolean => {
    const lowerName = name.trim().toLowerCase();
    const currentTemplates = getAllTemplates();
    
    return currentTemplates.some(t => {
      // Remove emoji from label before comparing
      const cleanLabel = t.label.replace(/📝/g, '').trim().toLowerCase();
      return cleanLabel === lowerName && t.id !== excludeId;
    });
  };

  // Helper: Generate unique name
  const generateUniqueName = (baseName: string): string => {
    let uniqueName = baseName;
    let counter = 2;
    while (nameExists(uniqueName)) {
      uniqueName = `${baseName} ${counter}`;
      counter++;
    }
    return uniqueName;
  };

  const handleRestore = (templateId: string) => {
    setTargetTemplateId(templateId);
    setShowRestoreConfirm(true);
  };

  const confirmRestore = () => {
    localStorage.removeItem(`custom_${targetTemplateId}_system`);
    localStorage.removeItem(`custom_${targetTemplateId}_user`);
    setTemplates(getAllTemplates());
    setShowRestoreConfirm(false);
  };

  const handleDuplicate = (templateId: string) => {
    if (!canAddMore) {
      setErrorMessage('Maximum 3 custom prompts allowed. Please delete an existing custom prompt first.');
      setShowErrorDialog(true);
      return;
    }
    
    const template = templates.find(t => t.id === templateId);
    const baseName = `${template?.label} (Copy)`;
    const uniqueName = generateUniqueName(baseName);
    
    setTargetTemplateId(templateId);
    setInputValue(uniqueName);
    setShowDuplicateDialog(true);
  };

  const confirmDuplicate = () => {
    const trimmedName = inputValue.trim();
    
    if (!trimmedName) {
      setShowDuplicateDialog(false);
      setErrorMessage('Please enter a prompt name');
      setShowErrorDialog(true);
      return;
    }
    
    if (nameExists(trimmedName)) {
      setShowDuplicateDialog(false);
      setErrorMessage('A prompt with this name already exists. Please choose a different name.');
      setShowErrorDialog(true);
      return;
    }

    const newId = `custom_${Date.now()}`;
    const sourceTemplate = templates.find(t => t.id === targetTemplateId);
    
    let systemPrompt = 'You are an expert technical assistant.';
    let userPrompt = 'Solve this problem:\n\n{CONTENT}';
    let supportsLang = false;
    
    if (sourceTemplate?.isCustom) {
      // Copying a custom prompt - get its content
      const sourcePrompt = CustomPromptsManager.getById(targetTemplateId);
      if (sourcePrompt) {
        systemPrompt = sourcePrompt.systemPrompt;
        userPrompt = sourcePrompt.userPrompt;
        supportsLang = sourcePrompt.supportsLanguage;
      }
    } else {
      // Copying a built-in template - get its default content
      systemPrompt = getDefaultSystemPrompt();
      userPrompt = getDefaultUserTemplate(targetTemplateId as PromptTemplate);
      supportsLang = supportsLanguageSelection(targetTemplateId);
    }
    
    CustomPromptsManager.save({
      id: newId,
      name: trimmedName,
      systemPrompt,
      userPrompt,
      supportsLanguage: supportsLang,
    });
    setTemplates(getAllTemplates());
    setShowDuplicateDialog(false);
  };

  const handleRename = (templateId: string) => {
    const prompt = CustomPromptsManager.getById(templateId);
    if (prompt) {
      setTargetTemplateId(templateId);
      setInputValue(prompt.name);
      setShowRenameDialog(true);
    }
  };

  const confirmRename = () => {
    const trimmedName = inputValue.trim();
    if (!trimmedName) {
      setShowRenameDialog(false);
      setErrorMessage('Please enter a prompt name');
      setShowErrorDialog(true);
      return;
    }

    if (nameExists(trimmedName, targetTemplateId)) {
      setShowRenameDialog(false);
      setErrorMessage('A prompt with this name already exists. Please choose a different name.');
      setShowErrorDialog(true);
      return;
    }

    const prompt = CustomPromptsManager.getById(targetTemplateId);
    if (prompt) {
      CustomPromptsManager.save({ ...prompt, name: trimmedName });
      setTemplates(getAllTemplates());
    }
    setShowRenameDialog(false);
  };

  const handleDelete = (templateId: string) => {
    setTargetTemplateId(templateId);
    setShowDeleteConfirm(true);
  };

  const confirmDelete = () => {
    CustomPromptsManager.delete(targetTemplateId);
    if (selectedTemplate === targetTemplateId) {
      onSelectTemplate(PromptTemplate.AlgorithmOptimal);
    }
    setTemplates(getAllTemplates());
    setShowDeleteConfirm(false);
  };

  const handleCreateNew = () => {
    if (!canAddMore) {
      setErrorMessage('Maximum 3 custom prompts allowed. Please delete an existing custom prompt first.');
      setShowErrorDialog(true);
      return;
    }
    setInputValue('');
    setShowNewPromptDialog(true);
  };

  const confirmCreateNew = () => {
    const trimmedName = inputValue.trim();
    if (!trimmedName) {
      setShowNewPromptDialog(false);
      setErrorMessage('Please enter a prompt name');
      setShowErrorDialog(true);
      return;
    }

    if (nameExists(trimmedName)) {
      setShowNewPromptDialog(false);
      setErrorMessage('A prompt with this name already exists. Please choose a different name.');
      setShowErrorDialog(true);
      return;
    }

    const newId = `custom_${Date.now()}`;
    CustomPromptsManager.save({
      id: newId,
      name: trimmedName,
      systemPrompt: 'You are an expert technical assistant.',
      userPrompt: 'Solve this problem:\n\n{CONTENT}',
      supportsLanguage: false,
    });
    setTemplates(getAllTemplates());
    setShowNewPromptDialog(false);
  };

  const showLanguageSelector = supportsLanguageSelection(selectedTemplate);

  return (
    <div className="prompt-list-view">
      <div className="list-header">
        <button
          onClick={handleCreateNew}
          className="new-prompt-btn"
          disabled={!canAddMore}
        >
          + New Prompt
        </button>
      </div>

      {showLanguageSelector && (
        <div className="language-selector">
          <label>Programming Language:</label>
          <select
            value={selectedLanguage}
            onChange={(e) => onLanguageChange(e.target.value as ProgrammingLanguage)}
            className="input-field"
          >
            <option value={ProgrammingLanguage.Java}>{ProgrammingLanguage.Java}</option>
            <option value={ProgrammingLanguage.Python}>{ProgrammingLanguage.Python}</option>
            <option value={ProgrammingLanguage.JavaScript}>{ProgrammingLanguage.JavaScript}</option>
            <option value={ProgrammingLanguage.Cpp}>{ProgrammingLanguage.Cpp}</option>
            <option value={ProgrammingLanguage.Swift}>{ProgrammingLanguage.Swift}</option>
          </select>
        </div>
      )}

      <div className="prompts-list">
        {templates.map(template => (
          <div key={template.id} className="prompt-item">
            <input
              type="radio"
              name="active-prompt"
              checked={selectedTemplate === template.id}
              onChange={() => onSelectTemplate(template.id)}
              className="prompt-radio"
            />

            <div className="prompt-info">
              <span className="prompt-name">{template.label}</span>
              {selectedTemplate === template.id && (
                <span className="active-badge">Active</span>
              )}
            </div>

            <div className="prompt-actions">
              <button
                onClick={() => onEditPrompt(template.id)}
                className="action-btn"
                title="Edit"
                style={{
                  background: 'linear-gradient(135deg, #e3f2fd, #bbdefb)',
                  border: '1px solid #90caf9',
                  padding: '3px 6px',
                  borderRadius: '4px',
                  fontSize: '9px',
                  width: '75px',
                  color: '#1976d2',
                  cursor: 'pointer',
                  textAlign: 'center',
                }}
              >
                ✏️ Edit
              </button>

              {!template.isCustom ? (
                <>
                  <button
                    onClick={() => handleRestore(template.id)}
                    className="action-btn"
                    title="Restore to default"
                    style={{
                      background: 'linear-gradient(135deg, #e3f2fd, #bbdefb)',
                      border: '1px solid #90caf9',
                      padding: '3px 6px',
                      borderRadius: '4px',
                      fontSize: '9px',
                      width: '75px',
                      color: '#1976d2',
                      cursor: 'pointer',
                      textAlign: 'center',
                    }}
                  >
                    🔄 Restore
                  </button>
                  <button
                    onClick={() => handleDuplicate(template.id)}
                    className="action-btn"
                    title="Duplicate"
                    style={{
                      background: 'linear-gradient(135deg, #e3f2fd, #bbdefb)',
                      border: '1px solid #90caf9',
                      padding: '3px 6px',
                      borderRadius: '4px',
                      fontSize: '9px',
                      width: '75px',
                      color: '#1976d2',
                      cursor: 'pointer',
                      textAlign: 'center',
                    }}
                  >
                    📋 Duplicate
                  </button>
                </>
              ) : (
                <>
                  <button
                    onClick={() => handleRename(template.id)}
                    className="action-btn"
                    title="Rename"
                    style={{
                      background: 'linear-gradient(135deg, #e3f2fd, #bbdefb)',
                      border: '1px solid #90caf9',
                      padding: '3px 6px',
                      borderRadius: '4px',
                      fontSize: '9px',
                      width: '75px',
                      color: '#1976d2',
                      cursor: 'pointer',
                      textAlign: 'center',
                    }}
                  >
                    📝 Rename
                  </button>
                  <button
                    onClick={() => handleDelete(template.id)}
                    className="action-btn delete-btn"
                    title="Delete"
                    style={{
                      background: 'linear-gradient(135deg, #ffebee, #ffcdd2)',
                      border: '1px solid #ef9a9a',
                      padding: '3px 6px',
                      borderRadius: '4px',
                      fontSize: '9px',
                      width: '75px',
                      color: '#c62828',
                      cursor: 'pointer',
                      textAlign: 'center',
                    }}
                  >
                    🗑️ Delete
                  </button>
                </>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Delete Confirmation Dialog */}
      {showDeleteConfirm && (
        <div className="dialog-overlay" onClick={() => setShowDeleteConfirm(false)}>
          <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
            <h3>Delete Prompt?</h3>
            <p>Are you sure you want to delete this custom prompt?</p>
            <div className="dialog-actions">
              <button onClick={() => setShowDeleteConfirm(false)} className="dialog-btn cancel">
                Cancel
              </button>
              <button onClick={confirmDelete} className="dialog-btn confirm">
                Delete
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Restore Confirmation Dialog */}
      {showRestoreConfirm && (
        <div className="dialog-overlay" onClick={() => setShowRestoreConfirm(false)}>
          <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
            <h3>Restore Prompt?</h3>
            <p>This will reset the prompt to its default content.</p>
            <div className="dialog-actions">
              <button onClick={() => setShowRestoreConfirm(false)} className="dialog-btn cancel">
                Cancel
              </button>
              <button onClick={confirmRestore} className="dialog-btn confirm">
                Restore
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Rename Dialog */}
      {showRenameDialog && (
        <div className="dialog-overlay" onClick={() => setShowRenameDialog(false)}>
          <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
            <h3>Rename Prompt</h3>
            <input
              type="text"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              className="input-field"
              placeholder="Prompt name"
              autoFocus
            />
            <div className="dialog-actions">
              <button onClick={() => setShowRenameDialog(false)} className="dialog-btn cancel">
                Cancel
              </button>
              <button onClick={confirmRename} className="dialog-btn confirm">
                Rename
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Duplicate Dialog */}
      {showDuplicateDialog && (
        <div className="dialog-overlay" onClick={() => setShowDuplicateDialog(false)}>
          <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
            <h3>Duplicate Prompt</h3>
            <input
              type="text"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              className="input-field"
              placeholder="Prompt name"
              autoFocus
            />
            <div className="dialog-actions">
              <button onClick={() => setShowDuplicateDialog(false)} className="dialog-btn cancel">
                Cancel
              </button>
              <button onClick={confirmDuplicate} className="dialog-btn confirm">
                Create
              </button>
            </div>
          </div>
        </div>
      )}

      {/* New Prompt Dialog */}
      {showNewPromptDialog && (
        <div className="dialog-overlay" onClick={() => setShowNewPromptDialog(false)}>
          <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
            <h3>New Custom Prompt</h3>
            <input
              type="text"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              className="input-field"
              placeholder="Prompt name"
              autoFocus
            />
            <div className="dialog-actions">
              <button onClick={() => setShowNewPromptDialog(false)} className="dialog-btn cancel">
                Cancel
              </button>
              <button onClick={confirmCreateNew} className="dialog-btn confirm">
                Create
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Error Dialog */}
      {showErrorDialog && (
        <div className="dialog-overlay" onClick={() => setShowErrorDialog(false)}>
          <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
            <h3>⚠️ Error</h3>
            <p>{errorMessage}</p>
            <div className="dialog-actions">
              <button onClick={() => setShowErrorDialog(false)} className="dialog-btn confirm">
                OK
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
