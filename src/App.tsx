import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import AIResponseDisplay from './components/AIResponseDisplay';
import { buildPrompt, PromptTemplate, ProgrammingLanguage, getTemplateLabel, supportsLanguageSelection } from './services/prompts';
import './App.css';

interface ChromeTab {
  id: string;
  url: string;
  title: string;
  tab_type: string;
}

interface AIConfig {
  selected_model: string;
  gemini_api_key: string;
  claude_api_key: string;
}

function App() {
  const [cdpStatus, setCdpStatus] = useState('🔴 CDP Not Running');
  const [cdpReady, setCdpReady] = useState(false);
  const [tabs, setTabs] = useState<ChromeTab[]>([]);
  const [selectedTab, setSelectedTab] = useState<ChromeTab | null>(null);
  const [aiResponse, setAiResponse] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [isOpeningChrome, setIsOpeningChrome] = useState(false);
  const [message, setMessage] = useState('');
  
  const [selectedTemplate, setSelectedTemplate] = useState<PromptTemplate>(
    (localStorage.getItem('prompt_template') as PromptTemplate) || PromptTemplate.AlgorithmOptimal
  );
  const [selectedLanguage, setSelectedLanguage] = useState<ProgrammingLanguage>(
    (localStorage.getItem('language') as ProgrammingLanguage) || ProgrammingLanguage.Java
  );
  
  const [aiConfig, setAiConfig] = useState<AIConfig>(() => {
    const storedModel = localStorage.getItem('ai_model');
    if (storedModel === 'gemini-2.0-flash-exp' || storedModel === 'gemini-2.0-flash' || storedModel === 'gemini-1.5-pro') {
      localStorage.setItem('ai_model', 'gemini-2.5-flash');
    }
    
    return {
      selected_model: localStorage.getItem('ai_model') || 'gemini-2.5-flash',
      gemini_api_key: localStorage.getItem('gemini_key') || '',
      claude_api_key: localStorage.getItem('claude_key') || '',
    };
  });
  const [showSettings, setShowSettings] = useState(false);

  useEffect(() => {
    checkCdpStatus();
    const interval = setInterval(checkCdpStatus, 3000);
    return () => clearInterval(interval);
  }, []);

  // Auto-fetch tabs when CDP becomes ready
  useEffect(() => {
    if (cdpReady && tabs.length === 0) {
      fetchTabs();
    }
  }, [cdpReady]);

  const checkCdpStatus = async () => {
    try {
      const status = await invoke<string>('get_cdp_status');
      setCdpStatus(status);
      setCdpReady(status.includes('🟢'));
    } catch (error) {
      setCdpStatus('🔴 Error');
      setCdpReady(false);
    }
  };

  const openChromeCdp = async () => {
    if (isOpeningChrome || isLoading) return;

    setIsOpeningChrome(true);
    setIsLoading(true);
    setMessage('🚀 Opening Chrome CDP...');
    
    try {
      const result = await invoke<string>('open_chrome_cdp');
      setMessage(`✅ ${result}`);
      setTimeout(() => checkCdpStatus(), 1000);
    } catch (error) {
      setMessage(`❌ ${error}`);
    } finally {
      setIsLoading(false);
      setTimeout(() => setIsOpeningChrome(false), 2000);
    }
  };

  const fetchTabs = async () => {
    if (!cdpReady) return;

    try {
      const chromeTabs = await invoke<ChromeTab[]>('get_chrome_tabs');
      setTabs(chromeTabs);
      
      // Auto-select first tab if none selected
      if (chromeTabs.length > 0 && !selectedTab) {
        setSelectedTab(chromeTabs[0]);
      }
    } catch (error) {
      console.error('Failed to fetch tabs:', error);
      setTabs([]);
    }
  };

  const solveWithAI = async () => {
    if (!selectedTab) {
      setMessage('❌ Select a tab from Input Source');
      return;
    }

    if (!aiConfig.gemini_api_key && !aiConfig.claude_api_key) {
      setMessage('❌ Configure API keys in Settings');
      setShowSettings(true);
      return;
    }

    setIsLoading(true);
    setMessage('📝 Extracting content...');
    setAiResponse(''); // Clear previous response
    
    try {
      // Step 1: Activate tab
      await invoke('activate_tab', { tabId: selectedTab.id });
      await new Promise(resolve => setTimeout(resolve, 300));
      
      // Step 2: Extract text
      setMessage('📝 Extracting text from tab...');
      const text = await invoke<string>('extract_tab_text', { tabId: selectedTab.id });
      
      // Step 3: Send to AI
      setMessage('🤖 Asking AI for solution...');
      const prompt = buildPrompt(selectedTemplate, selectedLanguage, text);

      const response = await invoke<string>('query_ai', {
        prompt,
        config: aiConfig,
      });
      
      setAiResponse(response);
      setMessage('✅ Solution ready!');
    } catch (error) {
      setMessage(`❌ Error: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const saveSettings = (newConfig: AIConfig) => {
    setAiConfig(newConfig);
    localStorage.setItem('ai_model', newConfig.selected_model);
    localStorage.setItem('gemini_key', newConfig.gemini_api_key);
    localStorage.setItem('claude_key', newConfig.claude_api_key);
    setShowSettings(false);
    setMessage('✅ Settings saved');
  };

  const updateTemplate = (template: PromptTemplate) => {
    setSelectedTemplate(template);
    localStorage.setItem('prompt_template', template);
  };

  const updateLanguage = (language: ProgrammingLanguage) => {
    setSelectedLanguage(language);
    localStorage.setItem('language', language);
  };

  return (
    <div className="app-container">
      <header className="app-header">
        <div className="header-left">
          <span className="app-icon">💻</span>
          <h1>CrackingInterview</h1>
        </div>
        <div className="header-right">
          <button 
            onClick={() => setShowSettings(true)}
            className="settings-btn"
            title="Settings"
          >
            ⚙️
          </button>
          <span className="status-text">{cdpStatus}</span>
        </div>
      </header>

      <div className="content">
        <div className="main-section">
          
          {/* Chrome CDP Control */}
          <div className="chrome-controls">
            <button 
              onClick={openChromeCdp}
              disabled={isLoading || isOpeningChrome}
              className="action-btn primary"
            >
              🚀 {isOpeningChrome ? 'Opening...' : 'Open Chrome CDP'}
            </button>
          </div>

          {/* Prompt Settings */}
          <div className="prompt-settings">
            <div className="setting-group">
              <label>Template:</label>
              <select 
                value={selectedTemplate}
                onChange={(e) => updateTemplate(e.target.value as PromptTemplate)}
                className="select-field"
              >
                <option value={PromptTemplate.AlgorithmOptimal}>{getTemplateLabel(PromptTemplate.AlgorithmOptimal)}</option>
                <option value={PromptTemplate.AlgorithmBeginner}>{getTemplateLabel(PromptTemplate.AlgorithmBeginner)}</option>
                <option value={PromptTemplate.SystemDesign}>{getTemplateLabel(PromptTemplate.SystemDesign)}</option>
                <option value={PromptTemplate.CodeReview}>{getTemplateLabel(PromptTemplate.CodeReview)}</option>
                <option value={PromptTemplate.ExplainConcept}>{getTemplateLabel(PromptTemplate.ExplainConcept)}</option>
              </select>
            </div>

              {supportsLanguageSelection(selectedTemplate) && (
                <div className="setting-group">
                  <label>Language:</label>
                  <select 
                    value={selectedLanguage}
                    onChange={(e) => updateLanguage(e.target.value as ProgrammingLanguage)}
                    className="select-field"
                  >
                    <option value={ProgrammingLanguage.Java}>{ProgrammingLanguage.Java}</option>
                    <option value={ProgrammingLanguage.Python}>{ProgrammingLanguage.Python}</option>
                    <option value={ProgrammingLanguage.JavaScript}>{ProgrammingLanguage.JavaScript}</option>
                    <option value={ProgrammingLanguage.Cpp}>{ProgrammingLanguage.Cpp}</option>
                    <option value={ProgrammingLanguage.Swift}>{ProgrammingLanguage.Swift}</option>
                  </select>
                </div>
              )}

            {/* Input Source Dropdown */}
            <div className="setting-group">
              <label>Input Source:</label>
              <select 
                value={selectedTab?.id || ''}
                onChange={(e) => {
                  const tab = tabs.find(t => t.id === e.target.value);
                  setSelectedTab(tab || null);
                  if (tab) setMessage(`Selected: ${tab.title}`);
                }}
                className="select-field input-source-select"
                disabled={!cdpReady || tabs.length === 0}
              >
                <option value="">
                  {tabs.length === 0 ? 'No tabs available' : 'Select a Chrome tab...'}
                </option>
                {tabs.map((tab) => (
                  <option key={tab.id} value={tab.id}>
                    {tab.title.substring(0, 50)}{tab.title.length > 50 ? '...' : ''}
                  </option>
                ))}
              </select>
            </div>

            {/* Solve Button */}
            <button 
              onClick={solveWithAI}
              disabled={isLoading || !selectedTab}
              className="action-btn solve-btn"
              title={!selectedTab ? "Select an input source first" : "Extract and solve with AI"}
            >
              🚀 Solve
            </button>
          </div>

          {!cdpReady && (
            <div className="info-banner warning">
              <p>⚠️  Chrome CDP not running</p>
              <p className="small">Click "Open Chrome CDP" button above</p>
            </div>
          )}

          {message && (
            <div className="message-box">
              {message}
            </div>
          )}

          {/* AI Response with Syntax Highlighting */}
          <AIResponseDisplay 
            response={aiResponse} 
            language={selectedLanguage.toLowerCase()}
          />

          {isLoading && (
            <div className="loading">
              <div className="spinner"></div>
              <p>Processing...</p>
            </div>
          )}
        </div>

        <div className="info-box">
          <h3>✨ Quick Start</h3>
          <ul>
            <li>🚀 Click "Open Chrome CDP"</li>
            <li>🌐 Navigate to LeetCode in CDP Chrome</li>
            <li>📋 Select tab from "Input Source"</li>
            <li>🚀 Click "Solve" button</li>
            <li>🎉 Get AI solution!</li>
          </ul>
        </div>
      </div>

      {/* Settings Modal */}
      {showSettings && (
        <div className="modal-overlay" onClick={() => setShowSettings(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h2>⚙️ Settings</h2>
              <button onClick={() => setShowSettings(false)} className="close-btn">✕</button>
            </div>
            
            <div className="modal-body">
              <div className="form-group">
                <label>AI Model:</label>
                <select 
                  value={aiConfig.selected_model}
                  onChange={(e) => setAiConfig({...aiConfig, selected_model: e.target.value})}
                  className="input-field"
                >
                  <option value="gemini-2.5-flash">Gemini 2.5 Flash</option>
                  <option value="claude-sonnet-4-20250514">Claude Sonnet 4</option>
                  <option value="claude-3-5-haiku-20241022">Claude 3.5 Haiku</option>
                </select>
              </div>

              <div className="form-group">
                <label>Gemini API Key:</label>
                <input 
                  type="password"
                  value={aiConfig.gemini_api_key}
                  onChange={(e) => setAiConfig({...aiConfig, gemini_api_key: e.target.value})}
                  placeholder="Enter Gemini API key"
                  className="input-field"
                />
              </div>

              <div className="form-group">
                <label>Claude API Key:</label>
                <input 
                  type="password"
                  value={aiConfig.claude_api_key}
                  onChange={(e) => setAiConfig({...aiConfig, claude_api_key: e.target.value})}
                  placeholder="Enter Claude API key"
                  className="input-field"
                />
              </div>

              <div className="modal-footer">
                <button onClick={() => saveSettings(aiConfig)} className="action-btn primary">
                  💾 Save Settings
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
