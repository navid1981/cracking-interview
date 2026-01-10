import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './App.css';

interface ChromeTab {
  id: string;
  url: string;
  title: string;
  tab_type: string;
}

function App() {
  const [cdpStatus, setCdpStatus] = useState('🔴 CDP Not Running');
  const [cdpReady, setCdpReady] = useState(false);
  const [tabs, setTabs] = useState<ChromeTab[]>([]);
  const [selectedTab, setSelectedTab] = useState<ChromeTab | null>(null);
  const [extractedContent, setExtractedContent] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [isOpeningChrome, setIsOpeningChrome] = useState(false);
  const [message, setMessage] = useState('');

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
    // Prevent double-clicks
    if (isOpeningChrome || isLoading) {
      console.log('⚠️  Already opening Chrome, ignoring click');
      return;
    }

    setIsOpeningChrome(true);
    setIsLoading(true);
    setMessage('🚀 Opening Chrome CDP...');
    
    try {
      const result = await invoke<string>('open_chrome_cdp');
      setMessage(`✅ ${result}`);
      
      // Wait a moment then check status
      setTimeout(() => checkCdpStatus(), 1000);
    } catch (error) {
      setMessage(`❌ ${error}`);
    } finally {
      setIsLoading(false);
      // Keep button disabled for 2 seconds to prevent rapid clicks
      setTimeout(() => setIsOpeningChrome(false), 2000);
    }
  };

  const getTabs = async () => {
    if (!cdpReady) {
      setMessage('❌ CDP not ready');
      return;
    }

    setIsLoading(true);
    setMessage('📡 Getting tabs...');
    
    try {
      const chromeTabs = await invoke<ChromeTab[]>('get_chrome_tabs');
      setTabs(chromeTabs);
      
      if (chromeTabs.length === 0) {
        setMessage('⚠️  No tabs found');
      } else {
        setMessage(`✅ Found ${chromeTabs.length} tabs`);
      }
    } catch (error) {
      setMessage(`❌ ${error}`);
      setTabs([]);
    } finally {
      setIsLoading(false);
    }
  };

  const extractText = async () => {
    if (!selectedTab) {
      setMessage('❌ Select a tab first');
      return;
    }

    setIsLoading(true);
    setMessage('📝 Extracting...');
    
    try {
      await invoke('activate_tab', { tabId: selectedTab.id });
      await new Promise(resolve => setTimeout(resolve, 300));
      
      const text = await invoke<string>('extract_tab_text', { tabId: selectedTab.id });
      setExtractedContent(text);
      setMessage(`✅ Extracted ${text.length} chars`);
    } catch (error) {
      setMessage(`❌ ${error}`);
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
          <span className="status-text">{cdpStatus}</span>
        </div>
      </header>

      <div className="content">
        <div className="main-section">
          
          <div className="chrome-controls">
            <button 
              onClick={openChromeCdp}
              disabled={isLoading || isOpeningChrome}
              className="action-btn primary"
            >
              🚀 {isOpeningChrome ? 'Opening...' : 'Open Chrome CDP'}
            </button>
            
            <button 
              onClick={getTabs}
              disabled={isLoading || !cdpReady}
              className="action-btn success"
              title={!cdpReady ? "Open Chrome CDP first" : ""}
            >
              📑 Get Tabs
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

          {tabs.length > 0 && (
            <div className="tabs-list">
              <h3>Tabs ({tabs.length}):</h3>
              {tabs.map((tab) => (
                <div 
                  key={tab.id}
                  className={`tab-item ${selectedTab?.id === tab.id ? 'selected' : ''}`}
                  onClick={() => {
                    setSelectedTab(tab);
                    setMessage(`Selected: ${tab.title}`);
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

          {selectedTab && cdpReady && (
            <div className="extract-section">
              <h4>Selected: {selectedTab.title}</h4>
              <button 
                onClick={extractText}
                disabled={isLoading}
                className="action-btn success"
              >
                📝 Extract Text
              </button>
            </div>
          )}

          {extractedContent && (
            <div className="content-display">
              <div className="section-header">
                <h3>📄 Extracted Content</h3>
                <button 
                  onClick={() => navigator.clipboard.writeText(extractedContent)}
                  className="copy-btn"
                >
                  📋 Copy
                </button>
              </div>
              <pre className="content-text">{extractedContent}</pre>
            </div>
          )}

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
            <li>🌐 Navigate to LeetCode</li>
            <li>📑 Click "Get Tabs"</li>
            <li>✅ Select & extract</li>
          </ul>
        </div>
      </div>
    </div>
  );
}

export default App;
