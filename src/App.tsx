import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import AIResponseDisplay from './components/AIResponseDisplay';
import TabDropdown from './components/TabDropdown';
import PromptEditor from './components/PromptEditor';
import PromptListView from './components/PromptListView';
import { buildPrompt, PromptTemplate, ProgrammingLanguage, getAllTemplates } from './services/prompts';
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
  
  const [selectedTemplate, setSelectedTemplate] = useState<string>(
    localStorage.getItem('prompt_template') || PromptTemplate.AlgorithmOptimal
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
  const [previousWindowSize, setPreviousWindowSize] = useState<{width: number, height: number} | null>(null);
  
  // Prompt editing state
  const [promptEditMode, setPromptEditMode] = useState<'list' | 'edit'>('list');
  const [editingPromptId, setEditingPromptId] = useState<string | null>(null);
  const solveWithAIRef = useRef<(() => Promise<void>) | null>(null);

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

  // Global hotkey support (registered on Rust side). This lets user stay in Chrome and trigger Solve.
  useEffect(() => {
    let unlistenFn: (() => void) | null = null;
    (async () => {
      try {
        unlistenFn = await listen('hotkey-solve', async () => {
          if (solveWithAIRef.current) {
            await solveWithAIRef.current();
          }
        });
      } catch (e) {
        console.warn('Failed to listen for hotkey-solve:', e);
      }
    })();
    return () => {
      try {
        unlistenFn?.();
      } catch {
        // ignore
      }
    };
  }, []);
  
  // Keep templates registry initialized once (used by PromptEditor / PromptListView).
  // PromptListView reads templates internally on mount; no extra state needed here.
  useEffect(() => {
    getAllTemplates();
  }, []);

  const checkGoogleTokens = async () => {
    try {
      const exists = await invoke<boolean>('get_google_token_status');
      setGoogleTokenExists(exists);
    } catch (error) {
      setGoogleTokenExists(false);
    }
  };

  const handleOpenSettings = async () => {
    try { await invoke('frontend_log', { message: 'handleOpenSettings clicked' }); } catch {}
    console.log('🎯 handleOpenSettings called!');
    
    try {
      const currentSize = await invoke<{ width: number; height: number }>('get_window_inner_size');
      try { await invoke('frontend_log', { message: `currentSize=${currentSize.width}x${currentSize.height}` }); } catch {}
      console.log('📏 Current window size:', currentSize.width, 'x', currentSize.height);
      
      // Match the app's default window size from `src-tauri/tauri.conf.json`
      // so Settings content is always fully visible after a user resizes.
      const DEFAULT_WIDTH = 550;
      const DEFAULT_HEIGHT = 900;

      // Always save the user's current size on Settings open so we can restore it on close.
      setPreviousWindowSize({ width: currentSize.width, height: currentSize.height });
      console.log('💾 Saved user size:', currentSize.width, 'x', currentSize.height);

      // Force resize BEFORE opening the modal (more reliable).
      // Prefer backend resize (native context), fall back to JS API.
      try {
        await invoke('resize_window', { width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT });
        console.log('✅ resize_window invoked');
        try { await invoke('frontend_log', { message: `resize_window invoked ${DEFAULT_WIDTH}x${DEFAULT_HEIGHT}` }); } catch {}
      } catch (e) {
        console.warn('❌ resize_window command failed:', e);
        try { await invoke('frontend_log', { message: `resize_window failed: ${String(e)}` }); } catch {}
      }

      // Read back size so we can confirm resize actually happened.
      try {
        const after = await invoke<{ width: number; height: number }>('get_window_inner_size');
        console.log('📏 After resize size:', after.width, 'x', after.height);
        try { await invoke('frontend_log', { message: `afterSize=${after.width}x${after.height}` }); } catch {}
      } catch (e) {
        console.warn('Failed to read size after resize:', e);
      }

      console.log('⚙️ Resized to default:', DEFAULT_WIDTH, 'x', DEFAULT_HEIGHT);
      // Give the OS a moment to apply resize before showing modal.
      await new Promise((r) => setTimeout(r, 150));
    } catch (error) {
      console.error('❌ Window resize error:', error);
    }

    // Open modal AFTER resize attempt.
    setSettingsTab('models');
    setShowSettings(true);
  };

  const handleCloseSettings = async () => {
    console.log('🎯 handleCloseSettings called!');
    try { await invoke('frontend_log', { message: 'handleCloseSettings called' }); } catch {}
    setShowSettings(false);
    
    setTimeout(async () => {
      if (previousWindowSize) {
        try {
          console.log('🔄 Restoring to:', previousWindowSize.width, 'x', previousWindowSize.height);
          
          await invoke('resize_window', { width: previousWindowSize.width, height: previousWindowSize.height });
          console.log('✅ Restored!');
          setPreviousWindowSize(null);
        } catch (error) {
          console.error('❌ Restore error:', error);
          setPreviousWindowSize(null);
        }
      } else {
        console.log('ℹ️  No size to restore');
      }
    }, 100);
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

    if (!aiConfig.gemini_api_key && !aiConfig.claude_api_key && !googleTokenExists) {
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

  // Keep a stable reference for the hotkey listener.
  useEffect(() => {
    solveWithAIRef.current = solveWithAI;
  });


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
            onClick={handleOpenSettings}
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
        <div className="modal-overlay" onClick={handleCloseSettings}>
          <div className="modal-content settings-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h2>⚙️ Settings</h2>
              <button onClick={handleCloseSettings} className="close-btn">✕</button>
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
                onClick={() => {
                  setSettingsTab('prompts');
                  setPromptEditMode('list');
                }}
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
                  {promptEditMode === 'list' ? (
                    <PromptListView
                      selectedTemplate={selectedTemplate}
                      onSelectTemplate={(id: string) => {
                        setSelectedTemplate(id);
                        localStorage.setItem('prompt_template', id);
                      }}
                      onEditPrompt={(id: string) => {
                        setEditingPromptId(id);
                        setPromptEditMode('edit');
                      }}
                      selectedLanguage={selectedLanguage}
                      onLanguageChange={(lang: ProgrammingLanguage) => {
                        setSelectedLanguage(lang);
                        localStorage.setItem('language', lang);
                      }}
                    />
                  ) : (
                    <>
                      <PromptEditor
                        key={editingPromptId}
                        currentTemplateId={editingPromptId || PromptTemplate.AlgorithmOptimal}
                        onCancel={() => {
                          setPromptEditMode('list');
                          setEditingPromptId(null);
                        }}
                      />
                    </>
                  )}
                </>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
