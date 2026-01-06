import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './App.css';

interface ChromeTab {
  id: string;
  url: string;
  title: string;
  tab_type: string;
}

function App() {
  const [tabs, setTabs] = useState<ChromeTab[]>([]);
  const [selectedTab, setSelectedTab] = useState<ChromeTab | null>(null);
  const [message, setMessage] = useState('');
  const [extractedText, setExtractedText] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [chromeStatus, setChromeStatus] = useState<'checking' | 'ready' | 'error'>('checking');

  // Listen for Chrome auto-launch events
  useEffect(() => {
    const setupListeners = async () => {
      await listen('chrome-ready', () => {
        setChromeStatus('ready');
        setMessage('✅ Chrome CDP automatically started!');
      });
      
      await listen('chrome-error', (event) => {
        setChromeStatus('error');
        setMessage(`⚠️ Chrome startup issue: ${event.payload}`);
      });
    };
    
    setupListeners();
  }, []);

  const testCDP = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<string>('test_chrome_cdp');
      setMessage(result);
      setChromeStatus('ready');
    } catch (error) {
      setMessage(`❌ Error: ${error}`);
      setChromeStatus('error');
    } finally {
      setIsLoading(false);
    }
  };

  const startChromeCDP = async () => {
    setIsLoading(true);
    setChromeStatus('checking');
    setMessage('🔄 Restarting Chrome with CDP...');
    
    try {
      const result = await invoke<string>('start_chrome_cdp');
      setMessage(`✅ ${result}`);
      setChromeStatus('ready');
    } catch (error) {
      setMessage(`❌ Error: ${error}`);
      setChromeStatus('error');
    } finally {
      setIsLoading(false);
    }
  };

  const fetchTabs = async () => {
    setIsLoading(true);
    try {
      const chromeTabs = await invoke<ChromeTab[]>('get_chrome_tabs');
      setTabs(chromeTabs);
      setMessage(`✅ Found ${chromeTabs.length} Chrome tabs!`);
    } catch (error) {
      setMessage(`❌ Error: ${error}`);
      setTabs([]);
    } finally {
      setIsLoading(false);
    }
  };

  const extractText = async () => {
    if (!selectedTab) {
      setMessage('❌ Please select a tab first!');
      return;
    }

    setIsLoading(true);
    setExtractedText('');
    
    try {
      await invoke('activate_tab', { tabId: selectedTab.id });
      await new Promise(resolve => setTimeout(resolve, 300));
      
      const text = await invoke<string>('extract_tab_text', { tabId: selectedTab.id });
      setExtractedText(text);
      setMessage(`✅ Extracted ${text.length} characters!`);
    } catch (error) {
      setMessage(`❌ Extract error: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const extractLeetCode = async () => {
    if (!selectedTab) {
      setMessage('❌ Please select a tab first!');
      return;
    }

    setIsLoading(true);
    
    try {
      await invoke('activate_tab', { tabId: selectedTab.id });
      await new Promise(resolve => setTimeout(resolve, 300));
      
      const structuredData = await invoke<string>('extract_leetcode_problem', { tabId: selectedTab.id });
      setExtractedText(structuredData);
      setMessage('✅ Extracted structured LeetCode data!');
    } catch (error) {
      setMessage(`❌ Error: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="app-container">
      <header className="app-header">
        <div className="header-left">
          <span className="app-icon">💻</span>
          <h1>CrackingInterview</h1>
        </div>
        <div className="header-right">
          <span className={`status-indicator ${chromeStatus}`}>
            {chromeStatus === 'checking' && '🔄 Starting Chrome...'}
            {chromeStatus === 'ready' && '✅ Chrome Ready'}
            {chromeStatus === 'error' && '⚠️ Chrome Error'}
          </span>
        </div>
      </header>

      <div className="content">
        <div className="test-section">
          <h2>🚀 Auto-Launch Chrome CDP</h2>
          
          <div className="chrome-controls">
            <button 
              onClick={startChromeCDP} 
              disabled={isLoading}
              className="test-btn primary"
            >
              🔄 Restart Chrome with CDP
            </button>
            
            <button 
              onClick={testCDP} 
              disabled={isLoading}
              className="test-btn"
            >
              🔌 Check Status
            </button>
            
            <button 
              onClick={fetchTabs} 
              disabled={isLoading || chromeStatus !== 'ready'}
              className="test-btn success"
            >
              📑 Get Tabs
            </button>
          </div>

          {chromeStatus === 'checking' && (
            <div className="info-banner checking">
              <p>🔄 Chrome is starting with remote debugging...</p>
              <p className="small">This happens automatically when you launch the app</p>
            </div>
          )}

          {chromeStatus === 'error' && (
            <div className="info-banner error">
              <p>⚠️ Chrome CDP not available</p>
              <p className="small">Click "Restart Chrome with CDP" above</p>
            </div>
          )}

          {selectedTab && (
            <div className="extract-section">
              <h3>Selected: {selectedTab.title}</h3>
              <div className="test-buttons">
                <button 
                  onClick={extractText} 
                  disabled={isLoading}
                  className="test-btn success"
                >
                  📝 Extract Text
                </button>
                
                <button 
                  onClick={extractLeetCode} 
                  disabled={isLoading}
                  className="test-btn success"
                >
                  🎯 Extract LeetCode
                </button>
              </div>
            </div>
          )}

          {isLoading && (
            <div className="loading">
              <div className="spinner"></div>
              <p>Processing...</p>
            </div>
          )}

          {message && (
            <div className="message-box">
              <pre>{message}</pre>
            </div>
          )}

          {tabs.length > 0 && (
            <div className="tabs-list">
              <h3>Chrome Tabs ({tabs.length}):</h3>
              {tabs.map((tab) => (
                <div 
                  key={tab.id} 
                  className={`tab-item ${selectedTab?.id === tab.id ? 'selected' : ''}`}
                  onClick={() => {
                    setSelectedTab(tab);
                    setMessage(`✅ Selected: ${tab.title}`);
                  }}
                >
                  <span className="tab-icon">🌐</span>
                  <div className="tab-details">
                    <div className="tab-title">{tab.title}</div>
                    <div className="tab-url">{tab.url}</div>
                  </div>
                  {selectedTab?.id === tab.id && <span className="checkmark">✓</span>}
                </div>
              ))}
            </div>
          )}

          {extractedText && (
            <div className="extracted-text">
              <div className="section-header">
                <h3>📄 Extracted Content</h3>
                <button 
                  onClick={() => navigator.clipboard.writeText(extractedText)}
                  className="copy-btn"
                >
                  📋 Copy
                </button>
              </div>
              <pre className="text-content">{extractedText}</pre>
            </div>
          )}
        </div>

        <div className="info-box">
          <h3>✅ Features</h3>
          <ul>
            <li className="done">✅ Auto-launches Chrome with CDP on startup</li>
            <li className="done">✅ Kills existing Chrome automatically</li>
            <li className="done">✅ Manual restart button if needed</li>
            <li className="done">✅ Tab listing and selection</li>
            <li className="done">✅ Text extraction via CDP</li>
            <li className="done">✅ Structured LeetCode extraction</li>
          </ul>
          
          <div className="help-note">
            <strong>💡 How it works:</strong>
            <p>When you launch this app, it automatically closes your regular Chrome and starts a new Chrome with remote debugging enabled.</p>
            <p className="small">Navigate to LeetCode in the Chrome window that opens.</p>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
