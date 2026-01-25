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

interface AudioSource {
  id: string;
  name: string;
  source_type: 'audio';
  thumbnail?: string;
}

type InputSource = ChromeTab | DisplayInfo | AudioSource;

function isDisplay(source: InputSource): source is DisplayInfo {
  return 'width' in source && 'height' in source;
}

function isAudio(source: InputSource): source is AudioSource {
  return (source as any).source_type === 'audio';
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
  const allSourcesRef = useRef<InputSource[]>([]);
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

  // Persist AI model + API keys so selections survive app restarts.
  useEffect(() => {
    try {
      localStorage.setItem('ai_model', aiConfig.selected_model);
      localStorage.setItem('gemini_key', aiConfig.gemini_api_key || '');
      localStorage.setItem('claude_key', aiConfig.claude_api_key || '');
    } catch (e) {
      console.warn('Failed to persist AI config:', e);
    }
  }, [aiConfig.selected_model, aiConfig.gemini_api_key, aiConfig.claude_api_key]);
  
  const [showSettings, setShowSettings] = useState(false);
  const [settingsTab, setSettingsTab] = useState<'models' | 'prompts' | 'input' | 'hotkeys'>('models');
  const [runtimePlatform, setRuntimePlatform] = useState<'macos' | 'windows' | 'linux' | 'unknown'>('unknown');
  const [useScreenshot, setUseScreenshot] = useState(
    localStorage.getItem('use_screenshot') === 'true'
  );

  // Persist input mode (text vs screenshot) so it survives restarts.
  useEffect(() => {
    try {
      localStorage.setItem('use_screenshot', useScreenshot ? 'true' : 'false');
    } catch (e) {
      console.warn('Failed to persist input mode:', e);
    }
  }, [useScreenshot]);
  const [googleTokenExists, setGoogleTokenExists] = useState(false);
  const [isAuthenticating, setIsAuthenticating] = useState(false);
  const [previousWindowSize, setPreviousWindowSize] = useState<{width: number, height: number} | null>(null);
  const [isRecordingAudio, setIsRecordingAudio] = useState(false);
  const [audioSeconds, setAudioSeconds] = useState(0);
  const audioTimerRef = useRef<number | null>(null);
  const isRecordingAudioRef = useRef(false);
  const audioToggleInFlightRef = useRef(false);
  const lastAudioToggleAtRef = useRef(0);
  const [hotkeysDraft, setHotkeysDraft] = useState<{ text: string; screenshot: string; audio_toggle: string; scroll_up: string; scroll_down: string; move_up: string; move_down: string; move_left: string; move_right: string; toggle_visibility: string; quit_app: string }>({ text: '', screenshot: '', audio_toggle: '', scroll_up: '', scroll_down: '', move_up: '', move_down: '', move_left: '', move_right: '', toggle_visibility: '', quit_app: '' });
  const [hotkeysStatus, setHotkeysStatus] = useState<string>('');
  
  // Prompt editing state
  const [promptEditMode, setPromptEditMode] = useState<'list' | 'edit'>('list');
  const [editingPromptId, setEditingPromptId] = useState<string | null>(null);
  const solveWithAIRef = useRef<((mode?: 'auto' | 'text' | 'screenshot') => Promise<void>) | null>(null);
  const contentScrollRef = useRef<HTMLDivElement | null>(null);

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

  useEffect(() => {
    allSourcesRef.current = allSources;
  }, [allSources]);

  // Cleanup audio timer on unmount.
  useEffect(() => {
    return () => {
      if (audioTimerRef.current) {
        window.clearInterval(audioTimerRef.current);
        audioTimerRef.current = null;
      }
    };
  }, []);

  // Keep a ref in sync so hotkey-driven toggles don't depend on render timing.
  useEffect(() => {
    isRecordingAudioRef.current = isRecordingAudio;
  }, [isRecordingAudio]);

  const toggleAudioRecording = async () => {
    // Guard against duplicate hotkey events (React StrictMode/dev can double-register listeners)
    // and against rapid double presses.
    const now = Date.now();
    if (audioToggleInFlightRef.current) return;
    if (now - lastAudioToggleAtRef.current < 350) return;
    lastAudioToggleAtRef.current = now;

    if (!aiConfig.selected_model.startsWith('gemini')) {
      setMessage('⚠️ Audio input requires Gemini. Please select Gemini in Settings → AI Models.');
      return;
    }

    // Start recording
    if (!isRecordingAudioRef.current) {
      audioToggleInFlightRef.current = true;
      try {
        // Avoid duplicating the recording banner; the UI shows a dedicated timer banner while recording.
        setMessage('');
        try { await invoke('frontend_log', { message: 'audio: start recording' }); } catch {}
        // Flip ref immediately to avoid races on repeated triggers.
        isRecordingAudioRef.current = true;
        setIsRecordingAudio(true);
        setAudioSeconds(0);
        if (audioTimerRef.current) window.clearInterval(audioTimerRef.current);
        audioTimerRef.current = window.setInterval(() => setAudioSeconds((s) => s + 1), 1000);

        await invoke('start_audio_recording');
      } catch (e) {
        // Roll back state on failure.
        isRecordingAudioRef.current = false;
        setIsRecordingAudio(false);
        if (audioTimerRef.current) {
          window.clearInterval(audioTimerRef.current);
          audioTimerRef.current = null;
        }
        setMessage(`❌ Error: ${String(e)}`);
      } finally {
        audioToggleInFlightRef.current = false;
      }
      return;
    }

    // Stop recording and solve
    audioToggleInFlightRef.current = true;
    setIsLoading(true);
    setAiResponse('');
    setMessage('⏹️ Stopping recording...');
    try { await invoke('frontend_log', { message: 'audio: stop recording' }); } catch {}
    try {
      const audioPath = await invoke<string>('stop_audio_recording');
      try { await invoke('frontend_log', { message: `audio: saved to ${audioPath}` }); } catch {}

      isRecordingAudioRef.current = false;
      setIsRecordingAudio(false);
      if (audioTimerRef.current) {
        window.clearInterval(audioTimerRef.current);
        audioTimerRef.current = null;
      }

      setMessage('📝 Transcribing + solving with Gemini...');
      const audioInstructions =
        'You will receive an AUDIO recording of an interview question. First transcribe the question clearly. Then solve it. Provide a clear Explanation and a final Solution.';
      const prompt = buildPrompt(selectedTemplate, selectedLanguage, audioInstructions);
      const response = await invoke<string>('query_ai_with_audio', {
        prompt,
        audioPath,
        config: aiConfig,
      });
      setAiResponse(response);
      setMessage('✅ Solution ready!');
    } catch (e) {
      try { await invoke('frontend_log', { message: `audio: stop failed: ${String(e)}` }); } catch {}
      setMessage(`❌ Error: ${String(e)}`);
    } finally {
      setIsLoading(false);
      audioToggleInFlightRef.current = false;

      // Even if stopping failed, we likely already terminated the helper or cleared backend state.
      // Ensure the UI doesn't remain stuck in "recording".
      isRecordingAudioRef.current = false;
      setIsRecordingAudio(false);
      if (audioTimerRef.current) {
        window.clearInterval(audioTimerRef.current);
        audioTimerRef.current = null;
      }
    }
  };

  useEffect(() => {
    // Determine OS so Settings can show platform-specific hotkeys.
    (async () => {
      try {
        const os = await invoke<string>('get_os');
        const normalized = os.toLowerCase();
        if (normalized === 'macos') setRuntimePlatform('macos');
        else if (normalized === 'windows') setRuntimePlatform('windows');
        else if (normalized === 'linux') setRuntimePlatform('linux');
        else setRuntimePlatform('unknown');
      } catch {
        // Fallback for browser-only testing (http://127.0.0.1:1420/)
        const ua = (typeof navigator !== 'undefined' ? navigator.userAgent : '').toLowerCase();
        if (ua.includes('mac os') || ua.includes('macintosh')) setRuntimePlatform('macos');
        else if (ua.includes('windows')) setRuntimePlatform('windows');
        else if (ua.includes('linux')) setRuntimePlatform('linux');
        else setRuntimePlatform('unknown');
      }
    })();
  }, []);

  // Global hotkey support (registered on Rust side). This lets user stay in Chrome and trigger Solve.
  useEffect(() => {
    // React StrictMode (dev) mounts/unmounts components to detect side effects.
    // `listen()` is async; if we unmount before it resolves, we may leak a listener.
    // Use a cancellation-safe pattern: store unsubs and if cancelled, immediately unlisten.
    let cancelled = false;
    const unsubs: Array<() => void> = [];

    (async () => {
      try {
        const uText = await listen('hotkey-solve-text', async () => {
          try { await invoke('frontend_log', { message: 'FE received hotkey-solve-text' }); } catch {}
          if (solveWithAIRef.current) await solveWithAIRef.current('text');
        });
        if (cancelled) { uText(); return; }
        unsubs.push(uText);

        const uShot = await listen('hotkey-solve-screenshot', async () => {
          try { await invoke('frontend_log', { message: 'FE received hotkey-solve-screenshot' }); } catch {}
          if (solveWithAIRef.current) await solveWithAIRef.current('screenshot');
        });
        if (cancelled) { uShot(); return; }
        unsubs.push(uShot);

        const uAudio = await listen('hotkey-audio-toggle', async () => {
          try { await invoke('frontend_log', { message: 'FE received hotkey-audio-toggle' }); } catch {}
          const audio = (allSourcesRef.current || []).find((s) => isAudio(s) && s.id === 'audio') as any;
          setSelectedTab(audio || ({ id: 'audio', name: 'Audio (System)', source_type: 'audio' } as any));
          await toggleAudioRecording();
        });
        if (cancelled) { uAudio(); return; }
        unsubs.push(uAudio);

        const uScrollUp = await listen('hotkey-scroll-up', async () => {
          try { await invoke('frontend_log', { message: 'FE received hotkey-scroll-up' }); } catch {}
          const el = contentScrollRef.current;
          if (el) el.scrollBy({ top: -260, behavior: 'smooth' });
        });
        if (cancelled) { uScrollUp(); return; }
        unsubs.push(uScrollUp);

        const uScrollDown = await listen('hotkey-scroll-down', async () => {
          try { await invoke('frontend_log', { message: 'FE received hotkey-scroll-down' }); } catch {}
          const el = contentScrollRef.current;
          if (el) el.scrollBy({ top: 260, behavior: 'smooth' });
        });
        if (cancelled) { uScrollDown(); return; }
        unsubs.push(uScrollDown);

        const uMoveUp = await listen('hotkey-move-up', async () => {
          try { await invoke('frontend_log', { message: 'FE received hotkey-move-up' }); } catch {}
          await invoke('move_window_by', { dx: 0, dy: -80 });
        });
        if (cancelled) { uMoveUp(); return; }
        unsubs.push(uMoveUp);

        const uMoveDown = await listen('hotkey-move-down', async () => {
          try { await invoke('frontend_log', { message: 'FE received hotkey-move-down' }); } catch {}
          await invoke('move_window_by', { dx: 0, dy: 80 });
        });
        if (cancelled) { uMoveDown(); return; }
        unsubs.push(uMoveDown);

        const uMoveLeft = await listen('hotkey-move-left', async () => {
          try { await invoke('frontend_log', { message: 'FE received hotkey-move-left' }); } catch {}
          await invoke('move_window_by', { dx: -80, dy: 0 });
        });
        if (cancelled) { uMoveLeft(); return; }
        unsubs.push(uMoveLeft);

        const uMoveRight = await listen('hotkey-move-right', async () => {
          try { await invoke('frontend_log', { message: 'FE received hotkey-move-right' }); } catch {}
          await invoke('move_window_by', { dx: 80, dy: 0 });
        });
        if (cancelled) { uMoveRight(); return; }
        unsubs.push(uMoveRight);
      } catch (e) {
        console.warn('Failed to listen for hotkey solve events:', e);
        try { await invoke('frontend_log', { message: `FE failed to listen hotkey events: ${String(e)}` }); } catch {}
      }
    })();

    return () => {
      cancelled = true;
      for (const u of unsubs) {
        try { u(); } catch {}
      }
    };
  }, []);
  
  // Keep templates registry initialized once (used by PromptEditor / PromptListView).
  // PromptListView reads templates internally on mount; no extra state needed here.
  useEffect(() => {
    getAllTemplates();
  }, []);

  useEffect(() => {
    if (showSettings && settingsTab === 'hotkeys') {
      loadHotkeys();
    }
  }, [showSettings, settingsTab]);

  const checkGoogleTokens = async () => {
    try {
      const exists = await invoke<boolean>('get_google_token_status');
      setGoogleTokenExists(exists);
    } catch (error) {
      setGoogleTokenExists(false);
    }
  };

  const loadHotkeys = async () => {
    try {
      const cfg = await invoke<{ text: string; screenshot: string; audio_toggle: string; scroll_up: string; scroll_down: string; move_up: string; move_down: string; move_left: string; move_right: string; toggle_visibility: string; quit_app: string }>('get_hotkeys');
      setHotkeysDraft(cfg);
      setHotkeysStatus('');
    } catch (e) {
      console.warn('Failed to load hotkeys:', e);
      setHotkeysStatus(`Failed to load hotkeys: ${String(e)}`);
    }
  };

  const saveHotkeys = async () => {
    try {
      setHotkeysStatus('Saving...');
      const updated = await invoke<{ text: string; screenshot: string; audio_toggle: string; scroll_up: string; scroll_down: string; move_up: string; move_down: string; move_left: string; move_right: string; toggle_visibility: string; quit_app: string }>('set_hotkeys', {
        textHotkey: hotkeysDraft.text,
        screenshotHotkey: hotkeysDraft.screenshot,
        audioToggleHotkey: hotkeysDraft.audio_toggle,
        scrollUpHotkey: hotkeysDraft.scroll_up,
        scrollDownHotkey: hotkeysDraft.scroll_down,
        moveUpHotkey: hotkeysDraft.move_up,
        moveDownHotkey: hotkeysDraft.move_down,
        moveLeftHotkey: hotkeysDraft.move_left,
        moveRightHotkey: hotkeysDraft.move_right,
        toggleVisibilityHotkey: hotkeysDraft.toggle_visibility,
        quitAppHotkey: hotkeysDraft.quit_app,
      });
      setHotkeysDraft(updated);
      setHotkeysStatus('Saved.');
    } catch (e) {
      setHotkeysStatus(`❌ ${String(e)}`);
    }
  };

  const resetHotkeys = async () => {
    try {
      setHotkeysStatus('Resetting...');
      const updated = await invoke<{ text: string; screenshot: string; audio_toggle: string; scroll_up: string; scroll_down: string; move_up: string; move_down: string; move_left: string; move_right: string; toggle_visibility: string; quit_app: string }>('reset_hotkeys_to_default');
      setHotkeysDraft(updated);
      setHotkeysStatus('Reset to defaults.');
    } catch (e) {
      setHotkeysStatus(`❌ ${String(e)}`);
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
      
      // Add Audio source (always available) + combine all sources (Chrome tabs, displays, then Audio)
      const audioSource: AudioSource = {
        id: 'audio',
        name: 'Audio (System)',
        source_type: 'audio',
      };
      const combined: InputSource[] = [...chromeTabs, ...displaysWithThumbnails, audioSource];
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

  const solveWithAI = async (mode: 'auto' | 'text' | 'screenshot' = 'auto') => {
    const sourceToUse = selectedTab ?? (allSources.length > 0 ? allSources[0] : null);
    if (!selectedTab && sourceToUse) {
      // Keep UI selection in sync so the user can see what was used.
      setSelectedTab(sourceToUse);
    }

    try {
      const selected = sourceToUse
        ? `${isAudio(sourceToUse) ? 'audio' : isDisplay(sourceToUse) ? 'display' : 'tab'}:${sourceToUse.id}`
        : 'none';
      await invoke('frontend_log', { message: `solveWithAI start mode=${mode} selected=${selected}` });
    } catch {}

    if (!sourceToUse) {
      setMessage('❌ No input source available');
      try { await invoke('frontend_log', { message: 'solveWithAI abort: no selectedTab' }); } catch {}
      return;
    }

    if (!aiConfig.gemini_api_key && !aiConfig.claude_api_key && !googleTokenExists) {
      setMessage('❌ Configure API keys in Settings');
      setShowSettings(true);
      try { await invoke('frontend_log', { message: 'solveWithAI abort: no API keys and no OAuth' }); } catch {}
      return;
    }

    // Audio source: toggle recording or stop+solve (Gemini-only).
    if (isAudio(sourceToUse)) {
      await toggleAudioRecording();
      return;
    }

    setIsLoading(true);
    setAiResponse('');
    
    try {
      let response: string;
      
      if (isDisplay(sourceToUse)) {
        // Display/Screen capture - always uses screenshot
        setMessage('📸 Capturing display...');
        const screenshotPath = await invoke<string>('capture_display_screenshot', { 
          displayId: sourceToUse.id 
        });
        
        setMessage('🤖 Analyzing screenshot with AI...');
        const prompt = buildPrompt(selectedTemplate, selectedLanguage);
        
        response = await invoke<string>('query_ai_with_image', {
          prompt,
          imagePath: screenshotPath,
          config: aiConfig,
        });
      } else if (mode === 'screenshot' || (mode === 'auto' && useScreenshot)) {
        // Chrome tab - screenshot mode
        setMessage('📸 Taking screenshot...');
        await invoke('activate_tab', { tabId: sourceToUse.id });
        await new Promise(resolve => setTimeout(resolve, 500));
        
        const screenshotPath = await invoke<string>('capture_tab_screenshot', { 
          tabId: sourceToUse.id 
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
        await invoke('activate_tab', { tabId: sourceToUse.id });
        await new Promise(resolve => setTimeout(resolve, 300));
        
        const text = await invoke<string>('extract_tab_text', { tabId: sourceToUse.id });
        
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
                const title = isAudio(source) ? source.name : isDisplay(source) ? source.name : source.title;
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
          onClick={() => solveWithAI('auto')}
          disabled={isLoading || !selectedTab}
          className="solve-button"
        >
          {selectedTab && isAudio(selectedTab)
            ? (isRecordingAudio ? `⏹️ Stop (${audioSeconds}s)` : '🎙️ Record')
            : '🚀 Solve'}
        </button>
      </div>

      {isRecordingAudio && (
        <div className="message-box" style={{ margin: '0 20px 12px 20px' }}>
          🎙️ Recording system audio… <strong>{audioSeconds}s</strong> (press Stop / Audio hotkey to send)
        </div>
      )}

      <div className="content" ref={contentScrollRef}>
        <div className="main-section">
          
          {!cdpReady && (
            <div className="info-banner warning">
              <p>⚠️  Chrome CDP not running</p>
              <button onClick={openChromeCdp} className="action-btn primary" style={{marginTop: '8px'}}>
                🚀 Open Chrome CDP
              </button>
            </div>
          )}

          {message && !isRecordingAudio && (
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
              <button 
                className={`tab-btn ${settingsTab === 'hotkeys' ? 'active' : ''}`}
                onClick={() => setSettingsTab('hotkeys')}
              >
                ⌨️ HotKeys
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
                        placeholder={googleTokenExists ? "Optional: paste your own API key to use your own quota (otherwise OAuth will be used)" : "Enter API key or sign in with Google"}
                        className="input-field"
                        disabled={false}
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
                          ✓ OAuth token is active. If you hit quota limits, paste your own Gemini API key above to use your own project/quota.
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
                      <strong>📸 Screenshot:</strong> Captures visual content including diagrams, tables, and formatting. Best for problems with images or complex layouts. If you chose Displays, the app will automatically use the Screenshot approach even when you press the Extract hotkey.
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

              {settingsTab === 'hotkeys' && (
                <>
                  <div className="form-group">
                    <label>Global Hotkeys:</label>

                    <div className="hotkeys-two-col">
                      <div className="hotkeys-col">
                        <div className="hotkey-field">
                          <div className="hotkey-label">Extract text → Solve</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.text}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, text: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + E' : runtimePlatform === 'windows' ? 'Alt + E' : 'Ctrl + E'}
                          />
                        </div>
                        <div className="hotkey-field">
                          <div className="hotkey-label">Scroll up (Explanation)</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.scroll_up}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, scroll_up: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + Up' : 'Ctrl + Up'}
                          />
                        </div>
                        <div className="hotkey-field">
                          <div className="hotkey-label">Move window up</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.move_up}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, move_up: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + Shift + Up' : runtimePlatform === 'windows' ? 'Alt + Shift + Up' : 'Ctrl + Shift + Up'}
                          />
                        </div>
                        <div className="hotkey-field">
                          <div className="hotkey-label">Move window left</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.move_left}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, move_left: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + Shift + Left' : runtimePlatform === 'windows' ? 'Alt + Shift + Left' : 'Ctrl + Shift + Left'}
                          />
                        </div>
                      </div>

                      <div className="hotkeys-col">
                        <div className="hotkey-field">
                          <div className="hotkey-label">Screenshot → Solve</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.screenshot}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, screenshot: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + S' : runtimePlatform === 'windows' ? 'Alt + S' : 'Ctrl + S'}
                          />
                        </div>
                        <div className="hotkey-field">
                          <div className="hotkey-label">Scroll down (Explanation)</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.scroll_down}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, scroll_down: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + Down' : 'Ctrl + Down'}
                          />
                        </div>
                        <div className="hotkey-field">
                          <div className="hotkey-label">Move window down</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.move_down}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, move_down: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + Shift + Down' : runtimePlatform === 'windows' ? 'Alt + Shift + Down' : 'Ctrl + Shift + Down'}
                          />
                        </div>
                        <div className="hotkey-field">
                          <div className="hotkey-label">Move window right</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.move_right}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, move_right: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + Shift + Right' : runtimePlatform === 'windows' ? 'Alt + Shift + Right' : 'Ctrl + Shift + Right'}
                          />
                        </div>
                      </div>
                    </div>

                    <div className="hotkeys-two-col hotkeys-two-col-single">
                      <div className="hotkeys-col">
                        <div className="hotkey-field">
                          <div className="hotkey-label">Audio toggle (System)</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.audio_toggle}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, audio_toggle: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + A' : runtimePlatform === 'windows' ? 'Alt + A' : 'Ctrl + A'}
                          />
                        </div>
                      </div>
                      <div className="hotkeys-col hotkeys-col-spacer" />
                    </div>

                    <div className="hotkeys-two-col hotkeys-two-col-single">
                      <div className="hotkeys-col">
                        <div className="hotkey-field">
                          <div className="hotkey-label">Quit app</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.quit_app}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, quit_app: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + Shift + Q' : runtimePlatform === 'windows' ? 'Alt + Shift + Q' : 'Ctrl + Shift + Q'}
                          />
                        </div>
                      </div>
                      <div className="hotkeys-col">
                        <div className="hotkey-field">
                          <div className="hotkey-label">Show/Hide app window</div>
                          <input
                            className="input-field"
                            value={hotkeysDraft.toggle_visibility}
                            onChange={(e) => setHotkeysDraft({ ...hotkeysDraft, toggle_visibility: e.target.value })}
                            placeholder={runtimePlatform === 'macos' ? 'Command + Shift + H' : runtimePlatform === 'windows' ? 'Alt + Shift + H' : 'Ctrl + Shift + H'}
                          />
                        </div>
                      </div>
                    </div>

                    <div style={{display: 'flex', gap: '8px', marginTop: '12px'}}>
                      <button className="action-btn primary" style={{flex: 1}} onClick={saveHotkeys}>
                        Save
                      </button>
                      <button className="action-btn secondary" style={{flex: 1}} onClick={resetHotkeys}>
                        Reset to defaults
                      </button>
                    </div>

                    {hotkeysStatus && (
                      <div style={{marginTop: '10px', fontSize: '12px', color: hotkeysStatus.startsWith('❌') ? '#c62828' : '#666'}}>
                        {hotkeysStatus}
                      </div>
                    )}

                    <p style={{fontSize: '12px', color: '#666', marginTop: '10px'}}>
                      If your selected Input Source is a Display, the app will automatically use the Screenshot approach even when you press the Extract hotkey.
                    </p>

                    <p style={{fontSize: '12px', color: '#666', marginTop: '12px'}}>
                      Tip: avoid Shift-only shortcuts (e.g. Shift+L). Use Cmd/Ctrl/Alt (and optionally Shift).
                    </p>
                  </div>
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
