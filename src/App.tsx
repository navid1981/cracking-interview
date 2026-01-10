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
  const [settingsTab, setSettingsTab] = useState<'api' | 'prompts'>('api');

  useEffect(() => {
    checkCdpStatus();
    const interval = setInterval(checkCdpStatus, 3000);
    return () => clearInterval(interval);
  }, []);

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
    if (!cdpReady) {
      console.log('⚠️  CDP not ready, skipping fetch');
      return;
    }

    console.log('📡 Fetching tabs...');
    try {
      const chromeTabs = await invoke<ChromeTab[]>('get_chrome_tabs');
      console.log('✅ Got tabs:', chromeTabs.length, chromeTabs);
      setTabs(chromeTabs);
      
      // Auto-select first tab if none selected
      if (chromeTabs.length > 0 && !selectedTab) {
        setSelectedTab(chromeTabs[0]);
        console.log('✅ Auto-selected first tab:', chromeTabs[0].title);
      }
    } catch (error) {
      console.error('❌ Failed to fetch tabs:', error);
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
    setAiResponse('');
    
    try {
      await invoke('activate_tab', { tabId: selectedTab.id });
      await new Promise(resolve => setTimeout(resolve, 300));
      
      setMessage('📝 Extracting text...');
      const text = await invoke<string>('extract_tab_text', { tabId: selectedTab.id });
      
      setMessage('🤖 Asking AI...');
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

  const saveSettings = () => {
    localStorage.setItem('ai_model', aiConfig.selected_model);
    localStorage.setItem('gemini_key', aiConfig.gemini_api_key);
    localStorage.setItem('claude_key', aiConfig.claude_api_key);
    localStorage.setItem('prompt_template', selectedTemplate);
    localStorage.setItem('language', selectedLanguage);
    setShowSettings(false);
    setMessage('✅ Settings saved');
  };

  return (
    <div className="app-container">
      <header className="app-header">
        <div className="header-left">
          <span className="app-icon">💻</span>
          <h1>CrackingInterview</h1>
        </div>
        <div className="header-right">
          <span className="status-indicator">{cdpStatus}</span>
          <button 
            onClick={() => setShowSettings(true)}
            className="settings-btn"
            title="Settings"
          >
            ⚙️
          </button>
        </div>
      </header>

      <div className="content">
        <div className="main-section">
          
          {/* Top Action Bar */}
          <div className="action-bar">
            <div className="input-source-group">
              <label>Input Source</label>
              <div className="input-with-refresh">
                <select 
                  value={selectedTab?.id || ''}
                  onChange={(e) => {
                    const tab = tabs.find(t => t.id === e.target.value);
                    setSelectedTab(tab || null);
                    if (tab) setMessage(`Selected: ${tab.title}`);
                  }}
                  className="input-source-dropdown"
                  disabled={!cdpReady}
                >
                  <option value="" disabled>
                    {!cdpReady ? 'Open Chrome CDP first' : tabs.length === 0 ? 'Click refresh to load tabs' : 'Select a Chrome tab...'}
                  </option>
                  {tabs.map((tab) => (
                    <option key={tab.id} value={tab.id}>
                      {tab.title.substring(0, 60)}{tab.title.length > 60 ? '...' : ''}
                    </option>
                  ))}
                </select>
                <button
                  onClick={(e) => {
                    e.preventDefault();
                    console.log('🔄 Refresh clicked, fetching tabs...');
                    fetchTabs();
                  }}
                  disabled={!cdpReady}
                  className="refresh-btn"
                  title="Refresh tabs"
                >
                  🔄
                </button>
              </div>
            </div>

            <button 
              onClick={solveWithAI}
              disabled={isLoading || !selectedTab}
              className="solve-button"
              title={!selectedTab ? "Select an input source first" : "Extract and solve with AI"}
            >
              🚀 Solve
            </button>
          </div>

          {!cdpReady && (
            <div className="info-banner warning">
              <p>⚠️  Chrome CDP not running</p>
              <button onClick={openChromeCdp} className="action-btn primary" style={{marginTop: '8px'}}>
                🚀 Open Chrome CDP
              </button>
            </div>
          )}

          {message && (
            <div className="message-box">
              {message}
            </div>
          )}

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
      </div>

      {/* Settings Modal with Tabs */}
      {showSettings && (
        <div className="modal-overlay" onClick={() => setShowSettings(false)}>
          <div className="modal-content settings-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h2>⚙️ Settings</h2>
              <button onClick={() => setShowSettings(false)} className="close-btn">✕</button>
            </div>

            {/* Settings Tabs */}
            <div className="settings-tabs">
              <button 
                className={`tab-btn ${settingsTab === 'api' ? 'active' : ''}`}
                onClick={() => setSettingsTab('api')}
              >
                🔑 API Keys
              </button>
              <button 
                className={`tab-btn ${settingsTab === 'prompts' ? 'active' : ''}`}
                onClick={() => setSettingsTab('prompts')}
              >
                📝 Prompts
              </button>
            </div>
            
            <div className="modal-body">
              {/* API Keys Tab */}
              {settingsTab === 'api' && (
                <>
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
                </>
              )}

              {/* Prompts Tab */}
              {settingsTab === 'prompts' && (
                <>
                  <div className="form-group">
                    <label>Prompt Template:</label>
                    <select 
                      value={selectedTemplate}
                      onChange={(e) => setSelectedTemplate(e.target.value as PromptTemplate)}
                      className="input-field"
                    >
                      <option value={PromptTemplate.AlgorithmOptimal}>{getTemplateLabel(PromptTemplate.AlgorithmOptimal)}</option>
                      <option value={PromptTemplate.AlgorithmBeginner}>{getTemplateLabel(PromptTemplate.AlgorithmBeginner)}</option>
                      <option value={PromptTemplate.SystemDesign}>{getTemplateLabel(PromptTemplate.SystemDesign)}</option>
                      <option value={PromptTemplate.CodeReview}>{getTemplateLabel(PromptTemplate.CodeReview)}</option>
                      <option value={PromptTemplate.ExplainConcept}>{getTemplateLabel(PromptTemplate.ExplainConcept)}</option>
                    </select>
                  </div>

                  {supportsLanguageSelection(selectedTemplate) && (
                    <div className="form-group">
                      <label>Programming Language:</label>
                      <select 
                        value={selectedLanguage}
                        onChange={(e) => setSelectedLanguage(e.target.value as ProgrammingLanguage)}
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

                  <div className="info-note">
                    <p><strong>Template Info:</strong></p>
                    <p className="small">
                      {selectedTemplate === PromptTemplate.AlgorithmOptimal && 'Provides optimal time/space complexity solutions with analysis.'}
                      {selectedTemplate === PromptTemplate.AlgorithmBeginner && 'Beginner-friendly explanations with step-by-step guidance.'}
                      {selectedTemplate === PromptTemplate.SystemDesign && 'High-level architecture and scalability discussions.'}
                      {selectedTemplate === PromptTemplate.CodeReview && 'Comprehensive code review with improvements.'}
                      {selectedTemplate === PromptTemplate.ExplainConcept && 'Clear explanations of technical concepts.'}
                    </p>
                  </div>
                </>
              )}

              <div className="modal-footer">
                <button onClick={() => saveSettings()} className="action-btn primary">
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
