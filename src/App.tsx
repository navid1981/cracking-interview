import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import AIResponseDisplay from './components/AIResponseDisplay';
import TabDropdown from './components/TabDropdown';
import { buildPrompt, PromptTemplate, ProgrammingLanguage, getTemplateLabel, supportsLanguageSelection } from './services/prompts';
import './App.css';

interface ChromeTab {
  id: string;
  url: string;
  title: string;
  tab_type: string;
  thumbnail?: string; // Base64 data URL
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

interface AIConfig {
  selected_model: string;
  gemini_api_key: string;
  claude_api_key: string;
}

function App() {
  const [cdpStatus, setCdpStatus] = useState('🔴 Chrome Not Running');
  const [cdpReady, setCdpReady] = useState(false);
  const [allSources, setAllSources] = useState<InputSource[]>([]);
  const [selectedTab, setSelectedTab] = useState<InputSource | null>(null);
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
  const [settingsTab, setSettingsTab] = useState<'models' | 'prompts' | 'input'>('models');
  const [useScreenshot, setUseScreenshot] = useState(
    localStorage.getItem('use_screenshot') === 'true'
  );
  const [googleTokenExists, setGoogleTokenExists] = useState(false);
  const [isAuthenticating, setIsAuthenticating] = useState(false);

  useEffect(() => {
    checkCdpStatus();
    const interval = setInterval(checkCdpStatus, 3000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    // Fetch displays and tabs on mount
    fetchTabs();
    
    // Check if Google tokens exist
    checkGoogleTokens();
  }, []);

  const checkGoogleTokens = async () => {
    try {
      const exists = await invoke<boolean>('get_google_token_status');
      setGoogleTokenExists(exists);
    } catch (error) {
      setGoogleTokenExists(false);
    }
  };

  const signInWithGoogle = async () => {
    setIsAuthenticating(true);
    setMessage('🔐 Opening Google Sign-In in browser...');
    
    try {
      const result = await invoke<string>('start_google_oauth');
      setIsAuthenticating(false);
      setMessage(`✅ ${result}`);
      setGoogleTokenExists(true);
    } catch (error) {
      setIsAuthenticating(false);
      setMessage(`❌ ${error}`);
    }
  };

  const signOutGoogle = async () => {
    try {
      const result = await invoke<string>('clear_google_tokens');
      setGoogleTokenExists(false);
      setMessage(`✅ ${result}`);
      await checkGoogleTokens(); // Recheck status
    } catch (error) {
      setMessage(`❌ ${error}`);
    }
  };

  useEffect(() => {
    // Fetch tabs when CDP becomes ready
    if (cdpReady) {
      fetchTabs();
    } else {
      // CDP closed - remove Chrome tabs from sources and reselect if needed
      setAllSources(prev => {
        const displaysOnly = prev.filter(s => isDisplay(s));
        
        // If current selection was a Chrome tab, switch to first display
        if (selectedTab && !isDisplay(selectedTab)) {
          if (displaysOnly.length > 0) {
            setSelectedTab(displaysOnly[0]);
            setMessage('Chrome CDP closed, switched to display');
          } else {
            setSelectedTab(null);
            setMessage('Chrome CDP closed');
          }
        }
        
        return displaysOnly;
      });
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
    try {
      // Fetch displays (always available)
      const displaysData = await invoke<DisplayInfo[]>('get_displays');
      
      // Fetch thumbnails for displays
      const displaysWithThumbnails = await Promise.all(
        displaysData.map(async (display) => {
          try {
            const thumbnail = await invoke<string>('get_display_thumbnail', { displayId: display.id });
            return { ...display, thumbnail };
          } catch (error) {
            console.warn(`Failed to get display thumbnail for ${display.id}:`, error);
            return display;
          }
        })
      );
      
      // Fetch Chrome tabs if CDP ready
      let chromeTabs: ChromeTab[] = [];
      if (cdpReady) {
        chromeTabs = await invoke<ChromeTab[]>('get_chrome_tabs');
        
        // Fetch thumbnails for Chrome tabs
        chromeTabs = await Promise.all(
          chromeTabs.map(async (tab) => {
            try {
              const thumbnail = await invoke<string>('get_tab_thumbnail', { tabId: tab.id });
              return { ...tab, thumbnail };
            } catch (error) {
              console.warn(`Failed to get thumbnail for ${tab.id}:`, error);
              return tab;
            }
          })
        );
      }
      
      // Combine all sources (Chrome tabs first, then displays)
      const combined: InputSource[] = [...chromeTabs, ...displaysWithThumbnails];
      setAllSources(combined);
      
      // If selected tab is no longer in the list, select first available source
      if (selectedTab && !combined.find(s => s.id === selectedTab.id)) {
        if (combined.length > 0) {
          setSelectedTab(combined[0]);
          setMessage('Previous selection unavailable, auto-selected first source');
        } else {
          setSelectedTab(null);
          setMessage('No sources available');
        }
      }
      
      // Auto-select first source if none selected
      else if (combined.length > 0 && !selectedTab) {
        setSelectedTab(combined[0]);
      }
    } catch (error) {
      console.error('Failed to fetch sources:', error);
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
    setAiResponse('');
    
    try {
      let response: string;
      
      if (isDisplay(selectedTab)) {
        // Display/Screen capture - always uses screenshot
        setMessage('📸 Capturing display...');
        const screenshotPath = await invoke<string>('capture_display_screenshot', { 
          displayId: selectedTab.id 
        });
        
        setMessage('🤖 Analyzing screenshot with AI...');
        const prompt = buildPrompt(selectedTemplate, selectedLanguage);
        
        response = await invoke<string>('query_ai_with_image', {
          prompt,
          imagePath: screenshotPath,
          config: aiConfig,
        });
      } else if (useScreenshot) {
        // Chrome tab - screenshot mode
        setMessage('📸 Taking screenshot...');
        await invoke('activate_tab', { tabId: selectedTab.id });
        await new Promise(resolve => setTimeout(resolve, 500));
        
        const screenshotPath = await invoke<string>('capture_tab_screenshot', { 
          tabId: selectedTab.id 
        });
        
        setMessage('🤖 Analyzing screenshot with AI...');
        const prompt = buildPrompt(selectedTemplate, selectedLanguage);
        
        response = await invoke<string>('query_ai_with_image', {
          prompt,
          imagePath: screenshotPath,
          config: aiConfig,
        });
      } else {
        // Chrome tab - text mode
        setMessage('📝 Extracting text...');
        await invoke('activate_tab', { tabId: selectedTab.id });
        await new Promise(resolve => setTimeout(resolve, 300));
        
        const text = await invoke<string>('extract_tab_text', { tabId: selectedTab.id });
        
        setMessage('🤖 Asking AI...');
        const prompt = buildPrompt(selectedTemplate, selectedLanguage, text);
        
        response = await invoke<string>('query_ai', {
          prompt,
          config: aiConfig,
        });
      }
      
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
    localStorage.setItem('use_screenshot', useScreenshot.toString());
    setShowSettings(false);
    setMessage('✅ Settings saved');
  };


  return (
    <div className="app-container">
      <header className="app-header">
        <div className="header-left">
          <img src="/icon.png" alt="CrackingInterview" className="app-icon-img" />
          <h1>CrackingInterview</h1>
        </div>
        <div className="header-right">
          <span className="status-indicator">{cdpStatus}</span>
          <button 
            onClick={() => setShowSettings(true)}
            className="settings-btn"
          >
            ⚙️
          </button>
        </div>
      </header>

      <div className="top-bar">
        <div className="input-source-container">
          <label>Input Source</label>
          <div className="input-with-refresh">
            <TabDropdown
              sources={allSources}
              selectedSource={selectedTab}
              onSelect={(source) => {
                setSelectedTab(source);
                const title = isDisplay(source) ? source.name : source.title;
                setMessage(`Selected: ${title}`);
              }}
              disabled={false}
            />
            <button
              onClick={fetchTabs}
              className="refresh-btn"
              title="Refresh sources"
            >
              🔄
            </button>
          </div>
        </div>

        <button 
          onClick={solveWithAI}
          disabled={isLoading || !selectedTab}
          className="solve-button"
        >
          🚀 Solve
        </button>
      </div>

      <div className="content">
        <div className="main-section">
          
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


      {showSettings && (
        <div className="modal-overlay" onClick={() => setShowSettings(false)}>
          <div className="modal-content settings-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h2>⚙️ Settings</h2>
              <button onClick={() => setShowSettings(false)} className="close-btn">✕</button>
            </div>

            <div className="settings-tabs">
              <button 
                className={`tab-btn ${settingsTab === 'models' ? 'active' : ''}`}
                onClick={() => setSettingsTab('models')}
              >
                🤖 AI Models
              </button>
              <button 
                className={`tab-btn ${settingsTab === 'input' ? 'active' : ''}`}
                onClick={() => setSettingsTab('input')}
              >
                📥 Input Mode
              </button>
              <button 
                className={`tab-btn ${settingsTab === 'prompts' ? 'active' : ''}`}
                onClick={() => setSettingsTab('prompts')}
              >
                📝 Prompts
              </button>
            </div>
            
            <div className="modal-body">
              {settingsTab === 'models' && (
                <>
                  <div className="form-group">
                    <label>AI Model:</label>
                    <select 
                      value={aiConfig.selected_model}
                      onChange={(e) => setAiConfig({...aiConfig, selected_model: e.target.value})}
                      className="input-field"
                    >
                      <option value="gemini-2.5-flash">Gemini 2.5 Flash (Free)</option>
                      <option value="claude-sonnet-4-20250514">Claude Sonnet 4</option>
                      <option value="claude-3-5-haiku-20241022">Claude 3.5 Haiku</option>
                    </select>
                  </div>

                  {aiConfig.selected_model.startsWith('gemini') && (
                    <div className="form-group">
                      <label>
                        Gemini API Key:
                        {!aiConfig.gemini_api_key && !googleTokenExists && <span className="alert-badge">⚠️ Required</span>}
                        {googleTokenExists && <span className="alert-badge" style={{color: '#4CAF50'}}>✓ OAuth Active</span>}
                      </label>
                      <input 
                        type="password"
                        value={aiConfig.gemini_api_key}
                        onChange={(e) => setAiConfig({...aiConfig, gemini_api_key: e.target.value})}
                        placeholder={googleTokenExists ? "Using OAuth token" : "Enter API key or sign in with Google"}
                        className="input-field"
                        disabled={googleTokenExists}
                      />
                      <button 
                        className="action-btn secondary" 
                        style={{marginTop: '8px', width: '100%'}}
                        onClick={googleTokenExists ? signOutGoogle : signInWithGoogle}
                        disabled={isAuthenticating}
                      >
                        {isAuthenticating ? '⏳ Authenticating...' : googleTokenExists ? '🚪 Sign Out from Google' : '🔐 Sign in with Google'}
                      </button>
                      
                      {googleTokenExists && (
                        <p style={{fontSize: '12px', color: '#4CAF50', marginTop: '8px', textAlign: 'center'}}>
                          ✓ Using OAuth token - API key not needed
                        </p>
                      )}
                    </div>
                  )}

                  {aiConfig.selected_model.startsWith('claude') && (
                    <div className="form-group">
                      <label>
                        Claude API Key:
                        {!aiConfig.claude_api_key && <span className="alert-badge">⚠️ Required</span>}
                      </label>
                      <input 
                        type="password"
                        value={aiConfig.claude_api_key}
                        onChange={(e) => setAiConfig({...aiConfig, claude_api_key: e.target.value})}
                        placeholder="Enter Claude API key"
                        className="input-field"
                      />
                    </div>
                  )}

                  <div className="info-note">
                    <p className="small">
                      <strong>💡 Tip:</strong> Get free Gemini API key at{' '}
                      <a href="https://aistudio.google.com/apikey" target="_blank" rel="noopener noreferrer">
                        aistudio.google.com/apikey
                      </a>
                    </p>
                  </div>
                </>
              )}

              {settingsTab === 'input' && (
                <>
                  <div className="form-group">
                    <label>Input Mode:</label>
                    <div className="toggle-group">
                      <button
                        onClick={() => setUseScreenshot(false)}
                        className={`toggle-btn ${!useScreenshot ? 'active' : ''}`}
                      >
                        📝 Text Extraction
                      </button>
                      <button
                        onClick={() => setUseScreenshot(true)}
                        className={`toggle-btn ${useScreenshot ? 'active' : ''}`}
                      >
                        📸 Screenshot Capture
                      </button>
                    </div>
                  </div>

                  <div className="info-note">
                    <h4 style={{marginBottom: '8px'}}>Mode Comparison:</h4>
                    <p className="small" style={{marginBottom: '12px'}}>
                      <strong>📝 Text:</strong> Extracts text content only. Fast and efficient. Best for text-based problems.
                    </p>
                    <p className="small">
                      <strong>📸 Screenshot:</strong> Captures visual content including diagrams, tables, and formatting. Best for problems with images or complex layouts.
                    </p>
                  </div>
                </>
              )}

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
                      {selectedTemplate === PromptTemplate.AlgorithmOptimal && 'Optimal solutions with complexity analysis'}
                      {selectedTemplate === PromptTemplate.AlgorithmBeginner && 'Beginner-friendly step-by-step explanations'}
                      {selectedTemplate === PromptTemplate.SystemDesign && 'Architecture and scalability discussions'}
                      {selectedTemplate === PromptTemplate.CodeReview && 'Comprehensive code review'}
                      {selectedTemplate === PromptTemplate.ExplainConcept && 'Clear technical explanations'}
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
