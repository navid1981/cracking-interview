import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import AIResponseDisplay from './components/AIResponseDisplay';
import LiveTranscript from './components/LiveTranscript';
import TabDropdown from './components/TabDropdown';
import PromptEditor from './components/PromptEditor';
import PromptListView from './components/PromptListView';
import DocumentManager, { DocumentPlaceholder } from './components/DocumentManager';
import AuthScreen from './components/AuthScreen';
import SetupChecklist from './components/SetupChecklist';
import { useSetupStatus } from './hooks/useSetupStatus';
import AppSettingsTab from './components/settings/AppSettingsTab';
import HotkeysSettingsTab from './components/settings/HotkeysSettingsTab';
import { getMessageActions, MESSAGE_ACTION_LABELS, MessageActionKind } from './services/messageActions';
import { HotkeysConfig, formatHotkey } from './services/hotkeys';
import { buildPrompt, PromptTemplate, ProgrammingLanguage, getAllTemplates, getTemplateLabel, getConversationPrompts, StoredDocPlaceholder } from './services/prompts';
import { 
  onAuthStateChange, 
  getUserSubscription, 
  getUsageStats,
  createCheckoutSession,
  logAudioUsage,
  signOut as supabaseSignOut,
  UserSubscription,
  UsageStats,
  EDGE_FUNCTION_URL,
  SUPABASE_API_KEY,
} from './services/supabase';
import { ModelConfig, loadCachedModels, fetchRemoteModelConfig, allowsAllDomains, isFreeTierUrlAllowed } from './services/modelConfig';
import type { User, Session } from '@supabase/supabase-js';
import packageJson from '../package.json';
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

function isChromeTab(source: InputSource): source is ChromeTab {
  return 'url' in source && !isDisplay(source);
}

function decodeHtmlEntities(text: string): string {
  const div = document.createElement('div');
  div.innerHTML = text;
  return div.textContent || div.innerText || text;
}

function isAudio(source: InputSource): source is AudioSource {
  return (source as any).source_type === 'audio';
}

interface AIConfig {
  selected_model: string;
  gemini_api_key?: string;  // BYO API key for free users who exhausted tries
}

interface Announcement {
  id: string;
  title: string;
  message: string; // HTML content
}

function App() {
  // ========== AUTH STATE ==========
  const [authUser, setAuthUser] = useState<User | null>(null);
  const [_authSession, setAuthSession] = useState<Session | null>(null);
  const [authLoading, setAuthLoading] = useState(true);
  const [subscription, setSubscription] = useState<UserSubscription | null>(null);
  const subscriptionRef = useRef<UserSubscription | null>(null);
  const [usageStats, setUsageStats] = useState<UsageStats | null>(null);
  const [isSubscribing, setIsSubscribing] = useState(false);
  const [announcement, setAnnouncement] = useState<Announcement | null>(null);
  const [showAnnouncement, setShowAnnouncement] = useState(true);
  const announcementDismissedRef = useRef(false);
  const [showAudioPromptWarning, setShowAudioPromptWarning] = useState(false);
  const [modelConfig, setModelConfig] = useState<ModelConfig | null>(loadCachedModels);
  const modelConfigRef = useRef<ModelConfig | null>(modelConfig);
  useEffect(() => { modelConfigRef.current = modelConfig; }, [modelConfig]);

  // ========== APP STATE ==========
  const [cdpStatus, setCdpStatus] = useState('🔴 Chrome Not Running');
  const [cdpReady, setCdpReady] = useState(false);
  // "user" | "app" | "none"
  const [chromeConnectionMode, setChromeConnectionMode] = useState<'user' | 'app' | 'none'>('none');
  const [allSources, setAllSources] = useState<InputSource[]>([]);
  const allSourcesRef = useRef<InputSource[]>([]);
  const [selectedTab, setSelectedTab] = useState<InputSource | null>(null);
  const [aiResponse, setAiResponse] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [isOpeningChrome, setIsOpeningChrome] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [message, setMessage] = useState('');
  // Solve progress stepper: tracks which phase we're in
  const [solvePhase, setSolvePhase] = useState<'idle' | 'extract' | 'screenshot' | 'capture' | 'audio' | 'asking' | 'error'>('idle');
  const [solveFlowType, setSolveFlowType] = useState<'text' | 'screenshot' | 'audio' | null>(null);
  
  const [selectedTemplate, setSelectedTemplate] = useState<string>(
    localStorage.getItem('prompt_template') || PromptTemplate.AlgorithmOptimal
  );
  const [selectedLanguage, setSelectedLanguage] = useState<ProgrammingLanguage>(
    (localStorage.getItem('language') as ProgrammingLanguage) || ProgrammingLanguage.Java
  );
  
  const [aiConfig, setAiConfig] = useState<AIConfig>(() => {
    const storedModel = localStorage.getItem('ai_model');
    const cached = loadCachedModels();
    
    // Reset if stored model is no longer in the available list
    if (storedModel && (!cached || !cached.pro_models.some(m => m.id === storedModel))) {
      localStorage.removeItem('ai_model');
    }
    
    return {
      selected_model: localStorage.getItem('ai_model') || cached?.default_pro_model || '',
      gemini_api_key: localStorage.getItem('gemini_api_key') || undefined,
    };
  });

  // Persist AI config so it survives app restarts.
  useEffect(() => {
    try {
      localStorage.setItem('ai_model', aiConfig.selected_model);
      if (aiConfig.gemini_api_key) {
        localStorage.setItem('gemini_api_key', aiConfig.gemini_api_key);
      } else {
        localStorage.removeItem('gemini_api_key');
      }
    } catch (e) {
      // Silent fail - non-critical
    }
  }, [aiConfig.selected_model, aiConfig.gemini_api_key]);

  // The BYO key is only stored in aiConfig (and shown as Active) after Google accepts it.
  const [byoKeyDraft, setByoKeyDraft] = useState('');
  const [byoKeyVerifying, setByoKeyVerifying] = useState(false);
  const [byoKeyError, setByoKeyError] = useState('');

  const handleSaveByoKey = async () => {
    const key = byoKeyDraft.trim();
    if (!key || byoKeyVerifying) return;
    setByoKeyError('');
    const models = await ensureModelConfig();
    if (!models) {
      setByoKeyError('❌ Could not load AI models from the server. Check your connection and try again.');
      return;
    }
    setByoKeyVerifying(true);
    try {
      await invoke('validate_gemini_key', { apiKey: key, model: models.byo_model.id });
      setAiConfig(prev => ({ ...prev, gemini_api_key: key }));
      setByoKeyDraft('');
    } catch (e) {
      setByoKeyError(String(e));
    } finally {
      setByoKeyVerifying(false);
    }
  };

  const handleRemoveByoKey = () => {
    setAiConfig(prev => ({ ...prev, gemini_api_key: undefined }));
    setByoKeyDraft('');
    setByoKeyError('');
  };
  
  const [showSettings, setShowSettings] = useState(false);
  const [settingsTab, setSettingsTab] = useState<'account' | 'models' | 'prompts' | 'hotkeys' | 'app'>('models');
  const [runtimePlatform, setRuntimePlatform] = useState<'macos' | 'windows' | 'linux' | 'unknown'>('unknown');
  const [useScreenshot, setUseScreenshot] = useState(
    localStorage.getItem('use_screenshot') === 'true'
  );

  // Persist input mode (text vs screenshot) so it survives restarts.
  useEffect(() => {
    try {
      localStorage.setItem('use_screenshot', useScreenshot ? 'true' : 'false');
    } catch (e) {
      // Silent fail - non-critical
    }
  }, [useScreenshot]);

  // ========== DOCUMENT PLACEHOLDERS STATE ==========
  const [showDocumentManager, setShowDocumentManager] = useState(false);
  const [documentPlaceholders, setDocumentPlaceholders] = useState<DocumentPlaceholder[]>([]);

  // ========== APP SETTINGS STATE ==========
  const [windowOpacity, setWindowOpacity] = useState<number>(() => {
    const saved = localStorage.getItem('window_opacity');
    return saved ? parseFloat(saved) : 1.0;
  });
  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    return (localStorage.getItem('app_theme') as 'light' | 'dark') || 'light';
  });
  const [stealthMode, setStealthMode] = useState<boolean>(() => {
    const saved = localStorage.getItem('stealth_mode');
    return saved !== null ? saved === 'true' : true;
  });

  // Apply saved app preferences on startup
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    invoke('set_window_opacity', { opacity: windowOpacity }).catch(() => {});
    // Stealth mode is applied by Rust at startup from stealth.json config file.
    // No need to call set_stealth_mode here — it would conflict with the startup
    // activation policy on macOS.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const isPro = subscription?.subscription_status === 'active' || subscription?.subscription_status === 'cancelling';

  // Ensure Stealth Mode is strictly disabled and unswitchable for non-Pro users.
  // For Pro users, default Stealth Mode to ON.
  useEffect(() => {
    if (authLoading) return;

    if (!isPro) {
      if (stealthMode) {
        setStealthMode(false);
        localStorage.setItem('stealth_mode', 'false');
        invoke('set_stealth_mode', { enabled: false }).catch(() => {});
      }
      localStorage.removeItem('pro_stealth_default_applied');
    } else {
      // Pro user: default Stealth Mode switch to ON
      const proDefaultApplied = localStorage.getItem('pro_stealth_default_applied');
      if (!proDefaultApplied) {
        localStorage.setItem('pro_stealth_default_applied', 'true');
        setStealthMode(true);
        localStorage.setItem('stealth_mode', 'true');
        invoke('set_stealth_mode', { enabled: true })
          .then(() => {
            invoke('get_stealth_status')
              .then(status => setStealthStatus(status as any))
              .catch(() => {});
          })
          .catch(() => {});
      }
    }
  }, [authLoading, isPro]);

  const [previousWindowSize, setPreviousWindowSize] = useState<{width: number, height: number} | null>(null);
  const [isRecordingAudio, setIsRecordingAudio] = useState(false);
  const [audioSeconds, setAudioSeconds] = useState(0);
  const audioTimerRef = useRef<number | null>(null);
  const isRecordingAudioRef = useRef(false);
  const audioToggleInFlightRef = useRef(false);
  const lastAudioToggleAtRef = useRef(0);
  // ========== LIVE TRANSCRIPTION STATE ==========
  const [isLiveTranscribing, setIsLiveTranscribing] = useState(false);
  const [liveTranscriptFinal, setLiveTranscriptFinal] = useState('');
  const [liveTranscriptInterim, setLiveTranscriptInterim] = useState('');
  const [conversationHistory, setConversationHistory] = useState<Array<{role: string; content: string}>>([]);
  const conversationHistoryRef = useRef<Array<{role: string; content: string}>>([]);
  const [displayTranscripts, setDisplayTranscripts] = useState<string[]>([]);
  const [silenceCountdown, setSilenceCountdown] = useState<number | null>(null);
  const [interviewLanguage, setInterviewLanguage] = useState(() => localStorage.getItem('interview_language') || 'multi');
  const silenceTimerRef = useRef<number | null>(null);
  const lastTranscriptTimeRef = useRef<number>(0);
  const isLiveTranscribingRef = useRef(false);
  const liveTranscriptFinalRef = useRef('');
  // Set from server config (`audio_session_max_seconds`) and remaining quota when a session starts
  const sessionMaxSecondsRef = useRef(Infinity);
  const audioSecondsRef = useRef(0);

  const [hotkeysDraft, setHotkeysDraft] = useState<HotkeysConfig>({ text: '', screenshot: '', audio_toggle: '', scroll_up: '', scroll_down: '', move_up: '', move_down: '', move_left: '', move_right: '', toggle_visibility: '', quit_app: '' });
  // Saved hotkeys (the draft above may hold unsaved edits from the HotKeys tab).
  const [activeHotkeys, setActiveHotkeys] = useState<HotkeysConfig | null>(null);
  const [hotkeysStatus, setHotkeysStatus] = useState<string>('');
  const [stealthStatus, setStealthStatus] = useState<{ swallowing_active: boolean; blur_prevention_active: boolean; macos_accessibility_granted: boolean } | null>(null);
  
  // Prompt editing state
  const [promptEditMode, setPromptEditMode] = useState<'list' | 'edit'>('list');
  const [editingPromptId, setEditingPromptId] = useState<string | null>(null);
  const solveWithAIRef = useRef<((mode?: 'auto' | 'text' | 'screenshot') => Promise<void>) | null>(null);
  const contentScrollRef = useRef<HTMLDivElement | null>(null);
  const previousTemplateRef = useRef<string | null>(null); // Stores template before auto-switching to Audio

  // ========== AUTH INITIALIZATION ==========
  useEffect(() => {
    // On app start: always clear the session so user lands on Sign In page.
    // "Remember me" only pre-fills credentials — it does NOT auto-login.
    const handleSessionOnStart = () => {
      const SUPABASE_URL = 'https://uudwpcjxbwtszhhcgybj.supabase.co';
      const storageKey = `sb-${SUPABASE_URL.split('//')[1].split('.')[0]}-auth-token`;
      localStorage.removeItem(storageKey);
    };

    handleSessionOnStart();
    setAuthLoading(false);

    // Listen for auth state changes
    // Note: We primarily handle auth in handleAuthSuccess (called from SignInForm)
    // This listener is a backup for token refresh events
    let lastProcessedUserId: string | null = null;
    
    const { data: { subscription: authSubscription } } = onAuthStateChange(async (event, session) => {
      // Only log and process meaningful events, skip duplicates
      if (event === 'SIGNED_IN' && session?.user?.id === lastProcessedUserId) {
        return; // Skip duplicate SIGNED_IN for same user
      }
      
      setAuthSession(session);
      setAuthUser(session?.user || null);
      
      if (session?.user) {
        lastProcessedUserId = session.user.id;
        const sub = await getUserSubscription(session.user.id);
        setSubscription(sub);
        setUsageStats(await loadUsageStats(session.user.id, sub, await ensureModelConfig()));
        
        // Fetch announcement
        await fetchAnnouncement(session.user.email, sub);
      } else {
        lastProcessedUserId = null;
        setSubscription(null);
        setUsageStats(null);
      }
    });

    return () => {
      authSubscription.unsubscribe();
    };
  }, []);

  // Usage stats are fetched:
  // 1. On sign in (in handleAuthSuccess and onAuthStateChange)
  // 2. After each AI request (from ai-proxy response)
  // No need for periodic polling - saves Supabase calls

  // Fetch announcement when subscription is loaded (handles both auth paths)
  useEffect(() => {
    if (authUser?.email && subscription) {
      fetchAnnouncement(authUser.email, subscription);
    }
  }, [authUser, subscription]);

  useEffect(() => {
    checkCdpStatus();
    const interval = setInterval(checkCdpStatus, 3000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    // Fetch displays and tabs on mount
    fetchTabs();
  }, []);

  useEffect(() => {
    allSourcesRef.current = allSources;
  }, [allSources]);

  // Refs let the background tab refresh (timer/focus listeners) read current state.
  const selectedTabRef = useRef<InputSource | null>(null);
  const cdpReadyRef = useRef(false);
  const isLoadingRef = useRef(false);
  const quietRefreshInFlightRef = useRef(false);
  // Background tabs often can't be screenshotted; try each tab+URL once instead of every cycle.
  const thumbnailAttemptedRef = useRef<Set<string>>(new Set());
  useEffect(() => { selectedTabRef.current = selectedTab; }, [selectedTab]);
  useEffect(() => { cdpReadyRef.current = cdpReady; }, [cdpReady]);
  useEffect(() => { isLoadingRef.current = isLoading; }, [isLoading]);

  // Background refresh of Chrome tabs: no spinner, keeps the selection (updated in place so its
  // URL/title stay current), and only re-captures thumbnails for new tabs or tabs that navigated.
  const refreshChromeTabsQuietly = async () => {
    if (!cdpReadyRef.current || quietRefreshInFlightRef.current || isLoadingRef.current) return;
    quietRefreshInFlightRef.current = true;
    try {
      const tabs = await Promise.race([
        invoke<ChromeTab[]>('get_chrome_tabs'),
        new Promise<ChromeTab[]>((_, reject) => setTimeout(() => reject('timeout'), 5000)),
      ]);
      const previousTabs = new Map(
        allSourcesRef.current.filter(isChromeTab).map(t => [t.id, t] as const)
      );
      const merged = await Promise.all(tabs.map(async (tab): Promise<ChromeTab> => {
        const previous = previousTabs.get(tab.id);
        if (previous?.thumbnail && previous.url === tab.url) return { ...tab, thumbnail: previous.thumbnail };
        const attemptKey = `${tab.id}|${tab.url}`;
        if (thumbnailAttemptedRef.current.has(attemptKey)) return { ...tab, thumbnail: previous?.thumbnail };
        thumbnailAttemptedRef.current.add(attemptKey);
        try {
          const thumbnail = await Promise.race([
            invoke<string>('get_tab_thumbnail', { tabId: tab.id }),
            new Promise<string>((_, reject) => setTimeout(() => reject('timeout'), 6000)),
          ]);
          return { ...tab, thumbnail };
        } catch {
          return { ...tab, thumbnail: previous?.thumbnail };
        }
      }));

      const changed = merged.length !== previousTabs.size || merged.some(t => {
        const p = previousTabs.get(t.id);
        return !p || p.url !== t.url || p.title !== t.title || p.thumbnail !== t.thumbnail;
      });
      if (!changed || !cdpReadyRef.current) return;

      const combined: InputSource[] = [...merged, ...allSourcesRef.current.filter(s => !isChromeTab(s))];
      allSourcesRef.current = combined;
      setAllSources(combined);

      const selected = selectedTabRef.current;
      if (selected && isChromeTab(selected)) {
        const updated = merged.find(t => t.id === selected.id);
        if (updated) {
          setSelectedTab(updated);
          if (updated.title !== selected.title) {
            const title = decodeHtmlEntities(updated.title?.trim() || updated.url);
            setMessage(prev => prev.startsWith('Selected: ') ? `Selected: ${title}` : prev);
          }
        } else if (combined.length > 0) {
          setSelectedTab(combined[0]);
          setMessage('Selected tab was closed, auto-selected first source');
        }
      }
    } catch {
      // Keep the current list on transient CDP errors; the next refresh will retry.
    } finally {
      quietRefreshInFlightRef.current = false;
    }
  };

  useEffect(() => {
    if (!cdpReady) return;
    const onFocus = () => { refreshChromeTabsQuietly(); };
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [cdpReady]);

  // Cleanup audio timer on unmount.
  useEffect(() => {
    return () => {
      if (audioTimerRef.current) {
        window.clearInterval(audioTimerRef.current);
        audioTimerRef.current = null;
      }
    };
  }, []);

  // Keep live transcription ref in sync
  useEffect(() => {
    isLiveTranscribingRef.current = isLiveTranscribing;
  }, [isLiveTranscribing]);

  // Listen for Deepgram transcript events from Rust backend
  useEffect(() => {
    let cancelled = false;
    const unsubs: Array<() => void> = [];

    (async () => {
      const u1 = await listen<{ text: string; is_final: boolean }>('live_transcript', (event) => {
        lastTranscriptTimeRef.current = Date.now();
        if (event.payload.is_final) {
          setLiveTranscriptFinal(prev => {
            const updated = prev + (prev ? ' ' : '') + event.payload.text;
            liveTranscriptFinalRef.current = updated;
            return updated;
          });
          setLiveTranscriptInterim('');
        } else {
          setLiveTranscriptInterim(event.payload.text);
        }
      });
      if (cancelled) { u1(); return; }
      unsubs.push(u1);

      const u2 = await listen('live_transcript_utterance_end', () => {
        // Deepgram detected end of utterance — start silence countdown
        lastTranscriptTimeRef.current = Date.now();
      });
      if (cancelled) { u2(); return; }
      unsubs.push(u2);

      const u3 = await listen<string>('live_transcript_error', (event) => {
        setMessage(`❌ Transcription error: ${event.payload}`);
        setIsLiveTranscribing(false);
      });
      if (cancelled) { u3(); return; }
      unsubs.push(u3);
    })();

    return () => {
      cancelled = true;
      unsubs.forEach(u => u());
    };
  }, []);

  // Keep a ref in sync so hotkey-driven toggles don't depend on render timing.
  useEffect(() => {
    isRecordingAudioRef.current = isRecordingAudio;
  }, [isRecordingAudio]);

  // Keep subscription ref in sync for hotkey access to latest subscription state
  useEffect(() => {
    subscriptionRef.current = subscription;
  }, [subscription]);

  // Auto-stop recording when session time limit is reached (server session cap or remaining quota)
  const sessionStopTriggeredRef = useRef(false);
  useEffect(() => {
    if (isLiveTranscribing && audioSeconds >= sessionMaxSecondsRef.current && !sessionStopTriggeredRef.current) {
      sessionStopTriggeredRef.current = true;
      const minutes = Math.round(sessionMaxSecondsRef.current / 60);
      setMessage(`⏱️ Session limit reached (${minutes} min). Stopping recording…`);
      stopLiveTranscription();
    }
  }, [audioSeconds, isLiveTranscribing]);

  const toggleAudioRecording = async () => {
    const now = Date.now();
    if (audioToggleInFlightRef.current) return;
    if (now - lastAudioToggleAtRef.current < 1000) return;

    audioToggleInFlightRef.current = true;
    lastAudioToggleAtRef.current = now;

    const isPro = subscriptionRef.current?.subscription_status === 'active' || subscriptionRef.current?.subscription_status === 'cancelling';
    if (!isPro) {
      setMessage('⚠️ Audio input requires Pro subscription.');
      audioToggleInFlightRef.current = false;
      return;
    }

    // Start live transcription
    if (!isRecordingAudioRef.current && !isLiveTranscribingRef.current) {
      try {
        await startLiveTranscription();
        setTimeout(() => { audioToggleInFlightRef.current = false; }, 500);
      } catch (e) {
        setMessage(`❌ Error: ${String(e)}`);
        audioToggleInFlightRef.current = false;
      }
      return;
    }

    // Stop live transcription and send to AI
    try {
      await stopLiveTranscription();
    } catch (e) {
      setMessage(`❌ Error: ${String(e)}`);
    } finally {
      audioToggleInFlightRef.current = false;
    }
  };

  // ========== LIVE TRANSCRIPTION ==========

  const getAccessToken = (): string => {
    const SUPABASE_URL = 'https://uudwpcjxbwtszhhcgybj.supabase.co';
    const storageKey = `sb-${SUPABASE_URL.split('//')[1].split('.')[0]}-auth-token`;
    const stored = localStorage.getItem(storageKey);
    if (!stored) return '';
    try {
      return JSON.parse(stored).access_token || '';
    } catch {
      return '';
    }
  };

  const fetchModelConfig = async (accessToken: string): Promise<ModelConfig | null> => {
    const config = await fetchRemoteModelConfig(accessToken);
    if (!config) return null;
    setModelConfig(config);
    modelConfigRef.current = config;
    const currentModel = localStorage.getItem('ai_model');
    if (!currentModel || !config.pro_models.some(m => m.id === currentModel)) {
      setAiConfig(prev => ({ ...prev, selected_model: config.default_pro_model }));
      localStorage.setItem('ai_model', config.default_pro_model);
    }
    return config;
  };

  /** Returns the server-provided model config, fetching it if it was never loaded. */
  const ensureModelConfig = async (): Promise<ModelConfig | null> => {
    if (modelConfigRef.current) return modelConfigRef.current;
    const accessToken = getAccessToken();
    if (!accessToken) return null;
    return fetchModelConfig(accessToken);
  };

  /** Usage stats need the server-provided Pro limits; without config they stay unknown. */
  const loadUsageStats = async (
    userId: string,
    sub: UserSubscription | null,
    config: ModelConfig | null,
  ): Promise<UsageStats | null> => {
    if (!config) return null;
    return getUsageStats(userId, sub, {
      requests: config.pro_request_limit,
      audioSeconds: config.pro_audio_seconds_limit,
    });
  };

  useEffect(() => {
    if (modelConfig && authUser && !usageStats) {
      void loadUsageStats(authUser.id, subscriptionRef.current, modelConfig).then(stats => {
        if (stats) setUsageStats(prev => prev ?? stats);
      });
    }
  }, [modelConfig, authUser]);

  useEffect(() => {
    if (showSettings && authUser && !modelConfigRef.current) {
      void ensureModelConfig();
    }
  }, [showSettings, authUser]);

  const fetchDeepgramKey = async (): Promise<{ key: string; model: string; remaining_seconds: number } | null> => {
    try {
      const accessToken = getAccessToken();
      if (!accessToken) return null;
      const resp = await fetch(`${EDGE_FUNCTION_URL}/deepgram-key`, {
        headers: {
          'Authorization': `Bearer ${accessToken}`,
          'Content-Type': 'application/json',
          'apikey': SUPABASE_API_KEY,
        },
      });
      if (!resp.ok) {
        const err = await resp.json();
        if (resp.status === 429 && err.audio_seconds_used !== undefined) {
          setUsageStats(prev => prev ? {
            ...prev,
            audio_seconds_used: err.audio_seconds_used,
            audio_seconds_limit: err.audio_seconds_limit,
          } : prev);
        }
        setMessage(`❌ ${err.error || 'Failed to get transcription key'}`);
        return null;
      }
      const data = await resp.json();
      if (data.audio_seconds_used !== undefined) {
        setUsageStats(prev => prev ? {
          ...prev,
          audio_seconds_used: data.audio_seconds_used,
          audio_seconds_limit: data.audio_seconds_limit,
        } : prev);
      }
      if (!data.model) {
        setMessage('❌ Transcription model not provided by the server. Please try again later.');
        return null;
      }
      return { key: data.key, model: data.model, remaining_seconds: data.remaining_seconds ?? Infinity };
    } catch (e) {
      setMessage(`❌ Error fetching transcription key: ${e}`);
      return null;
    }
  };

  const startLiveTranscription = async () => {
    const isPro = subscriptionRef.current?.subscription_status === 'active' || subscriptionRef.current?.subscription_status === 'cancelling';
    if (!isPro) {
      setMessage('⚠️ Live transcription requires Pro subscription.');
      return;
    }

    setMessage('🎙️ Starting live transcription…');
    const models = await ensureModelConfig();
    if (!models) {
      setMessage('❌ Could not load limits from the server. Check your connection and try again.');
      return;
    }
    const dgResult = await fetchDeepgramKey();
    if (!dgResult) return;

    const effectiveMax = Math.min(models.audio_session_max_seconds, dgResult.remaining_seconds);
    if (effectiveMax <= 0) {
      setMessage('❌ Monthly audio limit reached. Check your usage in Settings.');
      return;
    }
    sessionMaxSecondsRef.current = effectiveMax;
    sessionStopTriggeredRef.current = false;

    try {
      setLiveTranscriptFinal('');
      liveTranscriptFinalRef.current = '';
      setLiveTranscriptInterim('');
      setSilenceCountdown(null);
      lastTranscriptTimeRef.current = 0;

      await invoke('start_live_transcription', {
        deepgramKey: dgResult.key,
        model: dgResult.model,
        language: interviewLanguage,
      });

      setIsLiveTranscribing(true);
      setIsRecordingAudio(true);
      setAudioSeconds(0);
      audioSecondsRef.current = 0;
      if (audioTimerRef.current) window.clearInterval(audioTimerRef.current);
      audioTimerRef.current = window.setInterval(() => {
        setAudioSeconds(s => {
          const next = s + 1;
          audioSecondsRef.current = next;
          return next;
        });
      }, 1000);
      isRecordingAudioRef.current = true;
      setMessage('');

      startSilenceDetection();
    } catch (e) {
      setMessage(`❌ Failed to start transcription: ${e}`);
      setIsLiveTranscribing(false);
    }
  };

  // Manual stop: user clicks Stop or presses hotkey — closes WebSocket
  const stopLiveTranscription = async () => {
    if (silenceTimerRef.current) {
      window.clearInterval(silenceTimerRef.current);
      silenceTimerRef.current = null;
    }
    setSilenceCountdown(null);

    // Capture session duration before clearing timer
    const sessionDuration = audioSecondsRef.current;

    try {
      await invoke<string>('stop_live_transcription');
      setIsLiveTranscribing(false);
      setIsRecordingAudio(false);
      isRecordingAudioRef.current = false;
      if (audioTimerRef.current) {
        window.clearInterval(audioTimerRef.current);
        audioTimerRef.current = null;
      }

      // Log audio usage to backend
      if (sessionDuration > 0) {
        const accessToken = getAccessToken();
        if (accessToken) {
          const audioResult = await logAudioUsage(sessionDuration, accessToken);
          if (audioResult) {
            setUsageStats(prev => prev ? {
              ...prev,
              audio_seconds_used: audioResult.audio_seconds_used,
              audio_seconds_limit: audioResult.audio_seconds_limit,
            } : prev);
          }
        }
      }

      // Only send transcript that hasn't been auto-sent yet
      const unsent = liveTranscriptFinalRef.current;
      if (unsent.trim()) {
        setLiveTranscriptFinal('');
        liveTranscriptFinalRef.current = '';
        setLiveTranscriptInterim('');
        await sendTranscriptToAI(unsent);
      } else {
        setLiveTranscriptFinal('');
        liveTranscriptFinalRef.current = '';
        setLiveTranscriptInterim('');
      }
    } catch (e) {
      setIsLiveTranscribing(false);
      setIsRecordingAudio(false);
      isRecordingAudioRef.current = false;
      if (audioTimerRef.current) {
        window.clearInterval(audioTimerRef.current);
        audioTimerRef.current = null;
      }
      // Still log usage even on error
      if (sessionDuration > 0) {
        const accessToken = getAccessToken();
        if (accessToken) {
          logAudioUsage(sessionDuration, accessToken).then(audioResult => {
            if (audioResult) {
              setUsageStats(prev => prev ? {
                ...prev,
                audio_seconds_used: audioResult.audio_seconds_used,
                audio_seconds_limit: audioResult.audio_seconds_limit,
              } : prev);
            }
          });
        }
      }
      setMessage(`❌ Error stopping transcription: ${e}`);
    }
  };

  const SILENCE_THRESHOLD_MS = 3000;
  const isSendingRef = useRef(false);
  const autoSendRef = useRef<() => Promise<void>>();

  // Auto-send: silence detected — send transcript to AI but keep recording
  // Updated every render so it always has fresh closures
  autoSendRef.current = async () => {
    if (isSendingRef.current) return;
    const transcript = liveTranscriptFinalRef.current;
    if (!transcript.trim()) {
      lastTranscriptTimeRef.current = 0;
      setSilenceCountdown(null);
      return;
    }

    isSendingRef.current = true;
    setSilenceCountdown(null);

    setLiveTranscriptFinal('');
    liveTranscriptFinalRef.current = '';
    setLiveTranscriptInterim('');
    lastTranscriptTimeRef.current = 0;

    try {
      await sendTranscriptToAI(transcript);
    } finally {
      isSendingRef.current = false;
    }
  };

  const startSilenceDetection = () => {
    if (silenceTimerRef.current) window.clearInterval(silenceTimerRef.current);

    silenceTimerRef.current = window.setInterval(() => {
      if (!isLiveTranscribingRef.current) {
        if (silenceTimerRef.current) window.clearInterval(silenceTimerRef.current);
        return;
      }

      if (isSendingRef.current) return;

      const elapsed = Date.now() - lastTranscriptTimeRef.current;
      if (elapsed >= SILENCE_THRESHOLD_MS && lastTranscriptTimeRef.current > 0) {
        const remaining = Math.max(0, Math.ceil((SILENCE_THRESHOLD_MS + 2000 - elapsed) / 1000));
        if (remaining > 0) {
          setSilenceCountdown(remaining);
        } else {
          setSilenceCountdown(null);
          autoSendRef.current?.();
        }
      } else {
        setSilenceCountdown(null);
      }
    }, 500);
  };

  const estimateTokens = (text: string): number => Math.ceil(text.length / 4);

  const MAX_HISTORY_TOKENS = 100_000;
  const KEEP_RECENT_MESSAGES = 20; // last 10 Q&A pairs kept in full
  const SUMMARY_CHAR_LIMIT = 500;  // chars kept per old message in summary

  const trimConversationHistory = (history: Array<{role: string; content: string}>): Array<{role: string; content: string}> => {
    const totalTokens = history.reduce((sum, msg) => sum + estimateTokens(msg.content), 0);

    // Preserve: system prompt (index 0) + first user template message (index 1)
    const preserved: Array<{role: string; content: string}> = [];
    let trimStart = 0;
    if (history[0]?.role === 'system') {
      preserved.push(history[0]);
      trimStart = 1;
    }
    if (history[trimStart]?.role === 'user') {
      preserved.push(history[trimStart]);
      trimStart++;
    }

    const rest = history.slice(trimStart);

    if (totalTokens <= MAX_HISTORY_TOKENS && rest.length <= KEEP_RECENT_MESSAGES) return history;
    if (rest.length <= KEEP_RECENT_MESSAGES) return history;

    const oldMessages = rest.slice(0, rest.length - KEEP_RECENT_MESSAGES);
    const recentMessages = rest.slice(rest.length - KEEP_RECENT_MESSAGES);

    const summaryText = oldMessages
      .map(m => {
        const prefix = m.role === 'user' ? 'Q' : 'A';
        const truncated = m.content.length > SUMMARY_CHAR_LIMIT
          ? m.content.substring(0, SUMMARY_CHAR_LIMIT) + '…'
          : m.content;
        return `${prefix}: ${truncated}`;
      })
      .join('\n');

    const summaryMsg = {
      role: 'system' as const,
      content: `Previous conversation summary (earlier exchanges):\n${summaryText}`,
    };

    let result = [...preserved, summaryMsg, ...recentMessages];

    // Safety: if still over token limit, progressively drop oldest summaries
    let resultTokens = result.reduce((sum, msg) => sum + estimateTokens(msg.content), 0);
    while (resultTokens > MAX_HISTORY_TOKENS && result.length > KEEP_RECENT_MESSAGES + preserved.length) {
      const dropIdx = preserved.length;
      result.splice(dropIdx, 1);
      resultTokens = result.reduce((sum, msg) => sum + estimateTokens(msg.content), 0);
    }

    return result;
  };

  const sendTranscriptToAI = async (transcript: string) => {
    setIsLoading(true);
    setAiResponse('');
    setSolveFlowType('audio');
    setSolvePhase('asking');
    setMessage('🤖 Sending to AI…');

    try {
      const accessToken = getAccessToken();
      if (!accessToken) {
        setMessage('❌ Session error. Please sign in again.');
        setIsLoading(false);
        return;
      }

      // An empty/unknown model is replaced server-side by DEFAULT_PRO_MODEL.
      const model = aiConfig.selected_model || modelConfigRef.current?.default_pro_model || '';

      // Build messages array with conversation history (use ref to avoid stale closure)
      const currentHistory = conversationHistoryRef.current;
      let messages: Array<{role: string; content: string}> = [];

      if (currentHistory.length === 0) {
        // First turn — use the editable Verbal Interview system + user prompts
        const { systemPrompt, userMessage } = getConversationPrompts(
          selectedTemplate,
          interviewLanguage,
          transcript,
        );
        messages.push({ role: 'system', content: systemPrompt });
        messages.push({ role: 'user', content: userMessage });
      } else {
        // Follow-up turns — reuse history, add raw transcript
        messages = [...currentHistory];
        messages.push({ role: 'user', content: transcript });
      }

      // Trim if approaching token limit
      messages = trimConversationHistory(messages);

      const messagesJson = JSON.stringify(messages);

      const proxyResponse = await invoke<{
        response: string;
        usage?: { requests_used: number; requests_limit: number; is_paid?: boolean };
        error?: string;
      }>('query_ai_via_proxy_conversation', {
        messagesJson,
        model,
        accessToken,
      });

      if (proxyResponse.error) {
        throw new Error(proxyResponse.error);
      }

      // Update conversation history + display transcripts
      const newHistory = [...messages, { role: 'assistant', content: proxyResponse.response }];
      setConversationHistory(newHistory);
      conversationHistoryRef.current = newHistory;
      setDisplayTranscripts(prev => [...prev, transcript]);

      setAiResponse(proxyResponse.response);
      setSolvePhase('idle');
      setMessage('');

      if (proxyResponse.usage) {
        setUsageStats(prev => prev ? {
          ...prev,
          requests_used: proxyResponse.usage!.requests_used,
        } : prev);
        if (!proxyResponse.usage.is_paid && authUser?.id) {
          const updatedSub = await getUserSubscription(authUser.id);
          if (updatedSub) setSubscription(updatedSub);
        }
      }
    } catch (e) {
      setSolvePhase('error');
      setMessage(`❌ Error: ${String(e)}`);
    } finally {
      setIsLoading(false);
    }
  };

  const clearConversationHistory = () => {
    setConversationHistory([]);
    conversationHistoryRef.current = [];
    setDisplayTranscripts([]);
    setLiveTranscriptFinal('');
    liveTranscriptFinalRef.current = '';
    setLiveTranscriptInterim('');
    setAiResponse('');
    setMessage('Conversation cleared — starting fresh.');
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

  // Load document placeholders from disk on startup and cache in localStorage for sync access in buildPrompt
  useEffect(() => {
    (async () => {
      try {
        const placeholders = await invoke<DocumentPlaceholder[]>('get_document_placeholders');
        setDocumentPlaceholders(placeholders);
        const forPrompts: StoredDocPlaceholder[] = placeholders.map(p => ({
          name: p.name,
          extractedText: p.extracted_text,
        }));
        localStorage.setItem('document_placeholders', JSON.stringify(forPrompts));
      } catch {
        // Non-critical — placeholders just won't be available
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
          if (solveWithAIRef.current) await solveWithAIRef.current('text');
        });
        if (cancelled) { uText(); return; }
        unsubs.push(uText);

        const uShot = await listen('hotkey-solve-screenshot', async () => {
          if (solveWithAIRef.current) await solveWithAIRef.current('screenshot');
        });
        if (cancelled) { uShot(); return; }
        unsubs.push(uShot);

        const uAudio = await listen('hotkey-audio-toggle', async () => {
          announcementDismissedRef.current = true;
          setShowAnnouncement(false);
          const audio = (allSourcesRef.current || []).find((s) => isAudio(s) && s.id === 'audio') as any;
          setSelectedTab(audio || ({ id: 'audio', name: 'Audio (System)', source_type: 'audio' } as any));
          // Auto-select Audio prompt when triggered via hotkey
          if (selectedTemplate !== PromptTemplate.VerbalInterviewAudio) {
            previousTemplateRef.current = selectedTemplate;
            setSelectedTemplate(PromptTemplate.VerbalInterviewAudio);
            localStorage.setItem('prompt_template', PromptTemplate.VerbalInterviewAudio);
          }
          await toggleAudioRecording();
        });
        if (cancelled) { uAudio(); return; }
        unsubs.push(uAudio);

        const uScrollUp = await listen('hotkey-scroll-up', async () => {
          const el = contentScrollRef.current;
          if (el) el.scrollBy({ top: -260, behavior: 'smooth' });
        });
        if (cancelled) { uScrollUp(); return; }
        unsubs.push(uScrollUp);

        const uScrollDown = await listen('hotkey-scroll-down', async () => {
          const el = contentScrollRef.current;
          if (el) el.scrollBy({ top: 260, behavior: 'smooth' });
        });
        if (cancelled) { uScrollDown(); return; }
        unsubs.push(uScrollDown);

        const uMoveUp = await listen('hotkey-move-up', async () => {
          await invoke('move_window_by', { dx: 0, dy: -80 });
        });
        if (cancelled) { uMoveUp(); return; }
        unsubs.push(uMoveUp);

        const uMoveDown = await listen('hotkey-move-down', async () => {
          await invoke('move_window_by', { dx: 0, dy: 80 });
        });
        if (cancelled) { uMoveDown(); return; }
        unsubs.push(uMoveDown);

        const uMoveLeft = await listen('hotkey-move-left', async () => {
          await invoke('move_window_by', { dx: -80, dy: 0 });
        });
        if (cancelled) { uMoveLeft(); return; }
        unsubs.push(uMoveLeft);

        const uMoveRight = await listen('hotkey-move-right', async () => {
          await invoke('move_window_by', { dx: 80, dy: 0 });
        });
        if (cancelled) { uMoveRight(); return; }
        unsubs.push(uMoveRight);
      } catch (e) {
        // Silent fail - hotkey registration error
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
    if (showSettings) {
      if (settingsTab === 'hotkeys') {
        loadHotkeys();
      }
      if (settingsTab === 'hotkeys' || settingsTab === 'app') {
        invoke<{ swallowing_active: boolean; blur_prevention_active: boolean; macos_accessibility_granted: boolean }>('get_stealth_status')
          .then(setStealthStatus)
          .catch(() => {});
      }
    }
  }, [showSettings, settingsTab]);


  const loadHotkeys = async () => {
    try {
      const cfg = await invoke<HotkeysConfig>('get_hotkeys');
      setHotkeysDraft(cfg);
      setActiveHotkeys(cfg);
      setHotkeysStatus('');
      try {
        const status = await invoke<{ swallowing_active: boolean; blur_prevention_active: boolean; macos_accessibility_granted: boolean }>('get_stealth_status');
        setStealthStatus(status);
      } catch {
        // ignore
      }
    } catch (e) {
      setHotkeysStatus(`Failed to load hotkeys: ${String(e)}`);
    }
  };

  useEffect(() => {
    invoke<HotkeysConfig>('get_hotkeys').then(setActiveHotkeys).catch(() => {});
  }, []);

  const saveHotkeys = async () => {
    try {
      setHotkeysStatus('Saving...');
      const updated = await invoke<HotkeysConfig>('set_hotkeys', {
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
      setActiveHotkeys(updated);
      setHotkeysStatus('Saved.');
      try {
        const status = await invoke<{ swallowing_active: boolean; blur_prevention_active: boolean; macos_accessibility_granted: boolean }>('get_stealth_status');
        setStealthStatus(status);
      } catch {
        // ignore
      }
    } catch (e) {
      setHotkeysStatus(`❌ ${String(e)}`);
    }
  };

  const resetHotkeys = async () => {
    try {
      setHotkeysStatus('Resetting...');
      const updated = await invoke<HotkeysConfig>('reset_hotkeys_to_default');
      setHotkeysDraft(updated);
      setActiveHotkeys(updated);
      setHotkeysStatus('Reset to defaults.');
      try {
        const status = await invoke<{ swallowing_active: boolean; blur_prevention_active: boolean; macos_accessibility_granted: boolean }>('get_stealth_status');
        setStealthStatus(status);
      } catch {
        // ignore
      }
    } catch (e) {
      setHotkeysStatus(`❌ ${String(e)}`);
    }
  };

  const handleRequestAccessibility = async () => {
    try {
      await invoke('request_accessibility');
      // Wait a moment for System Settings or user toggle, then refresh status
      setTimeout(async () => {
        try {
          const status = await invoke<{ swallowing_active: boolean; blur_prevention_active: boolean; macos_accessibility_granted: boolean }>('refresh_stealth_status');
          setStealthStatus(status);
        } catch {
          // ignore
        }
      }, 1000);
    } catch (e) {
      console.error('Failed to request accessibility:', e);
    }
  };

  const setup = useSetupStatus(cdpReady, isPro, () => { handleRefreshStealthStatus(); });

  const handleOpacityChange = (opacity: number) => {
    setWindowOpacity(opacity);
    localStorage.setItem('window_opacity', opacity.toString());
    invoke('set_window_opacity', { opacity }).catch(console.error);
  };

  const handleThemeChange = (next: 'light' | 'dark') => {
    setTheme(next);
    localStorage.setItem('app_theme', next);
    document.documentElement.setAttribute('data-theme', next);
  };

  const handleStealthModeChange = (enabled: boolean) => {
    setStealthMode(enabled);
    localStorage.setItem('stealth_mode', enabled.toString());
    invoke('set_stealth_mode', { enabled })
      .then(() => invoke<typeof stealthStatus>('get_stealth_status'))
      .then(status => setStealthStatus(status))
      .catch(console.error);
  };

  const handleShowSetup = () => {
    setup.show();
    if (showSettings) handleCloseSettings();
  };

  const setupChecklistProps = {
    steps: setup.steps,
    requested: setup.requested,
    isOpeningChrome,
    onOpenChrome: () => { openChromeCdp(); },
    onDownloadChrome: () => { invoke('open_url', { url: 'https://www.google.com/chrome/' }); },
    onGrant: setup.grant,
    onRestart: () => { invoke('restart_app'); },
    onRecheck: setup.recheck,
  };

  const handleRefreshStealthStatus = async () => {
    try {
      const status = await invoke<{ swallowing_active: boolean; blur_prevention_active: boolean; macos_accessibility_granted: boolean }>('refresh_stealth_status');
      setStealthStatus(status);
    } catch (e) {
      console.error('Failed to refresh stealth status:', e);
    }
  };

  const handleOpenSettings = () => openSettingsAt('models');

  const openSettingsAt = async (tab: typeof settingsTab) => {
    
    try {
      const currentSize = await invoke<{ width: number; height: number }>('get_window_inner_size');
      
      // Match the app's default window size from `src-tauri/tauri.conf.json`
      // so Settings content is always fully visible after a user resizes.
      const DEFAULT_WIDTH = 550;
      const DEFAULT_HEIGHT = 900;

      // Always save the user's current size on Settings open so we can restore it on close.
      setPreviousWindowSize({ width: currentSize.width, height: currentSize.height });

      // Force resize BEFORE opening the modal (more reliable).
      // Prefer backend resize (native context), fall back to JS API.
      try {
        await invoke('resize_window', { width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT });
      } catch (e) {
        // Silent fail - resize error
      }

      // Give the OS a moment to apply resize before showing modal.
      await new Promise((r) => setTimeout(r, 150));
    } catch (error) {
      console.error('❌ Window resize error:', error);
    }

    // Open modal AFTER resize attempt.
    setSettingsTab(tab);
    setShowSettings(true);
  };

  const handleCloseSettings = async () => {
    setShowSettings(false);
    
    setTimeout(async () => {
      if (previousWindowSize) {
        try {
          
          await invoke('resize_window', { width: previousWindowSize.width, height: previousWindowSize.height });
          setPreviousWindowSize(null);
        } catch (error) {
          console.error('❌ Restore error:', error);
          setPreviousWindowSize(null);
        }
      } else {
      }
    }, 100);
  };


  // ========== SUPABASE AUTH HANDLERS ==========
  const fetchAnnouncement = async (email: string | undefined, sub: UserSubscription | null) => {
    if (!email) return;
    
    try {
      const userType = (sub?.subscription_status === 'active' || sub?.subscription_status === 'cancelling') ? 'pro' : 'free';
      const appVersion = packageJson.version;
      
      
      // Get access token from localStorage
      const SUPABASE_URL = 'https://uudwpcjxbwtszhhcgybj.supabase.co';
      const storageKey = `sb-${SUPABASE_URL.split('//')[1].split('.')[0]}-auth-token`;
      const storedSession = localStorage.getItem(storageKey);
      
      let accessToken = '';
      if (storedSession) {
        try {
          const session = JSON.parse(storedSession);
          accessToken = session.access_token || '';
        } catch (e) {
          console.error('[Announcement] Failed to parse session:', e);
        }
      }
      
      const response = await fetch(`${EDGE_FUNCTION_URL}/notification`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${accessToken}`,
          'apikey': SUPABASE_API_KEY,
        },
        body: JSON.stringify({
          email,
          user_type: userType,
          app_version: appVersion,
        }),
      });
      
      if (!response.ok) {
        console.error('[Announcement] Fetch failed:', response.status);
        return;
      }
      
      const data = await response.json();
      
      if (data.announcement) {
        setAnnouncement(data.announcement);
        if (!announcementDismissedRef.current) {
          setShowAnnouncement(true);
        }
      } else {
        setAnnouncement(null);
        setShowAnnouncement(false);
      }
    } catch (error) {
      console.error('[Announcement] Error fetching:', error);
    }
  };

  const handleAuthSuccess = async () => {
    
    // Close settings modal if open (e.g., user signed out from Account tab)
    setShowSettings(false);
    
    // Clear any previous status messages and AI response from previous session
    setMessage('');
    setAiResponse('');
    
    // Since we bypass Supabase JS client, onAuthStateChange doesn't fire
    // We need to manually get the session from localStorage and update state
    const SUPABASE_URL = 'https://uudwpcjxbwtszhhcgybj.supabase.co';
    const storageKey = `sb-${SUPABASE_URL.split('//')[1].split('.')[0]}-auth-token`;
    const storedSession = localStorage.getItem(storageKey);
    
    if (storedSession) {
      try {
        const session = JSON.parse(storedSession);
        
        setAuthSession(session);
        setAuthUser(session.user);
        
        // Fetch subscription and usage data
        if (session.user?.id) {
          const configPromise = fetchModelConfig(session.access_token);
          const sub = await getUserSubscription(session.user.id);
          setSubscription(sub);
          setUsageStats(await loadUsageStats(session.user.id, sub, await configPromise));

          // Fetch announcement
          await fetchAnnouncement(session.user.email, sub);
        }
      } catch (e) {
        console.error('[App] Failed to parse stored session:', e);
      }
    } else {
    }
  };

  const handleSignOut = async () => {
    try {
      await supabaseSignOut();
      // Note: Don't clear cracking_interview_remember_me or saved credentials here.
      // "Remember me" means credentials persist across sign-out/sign-in cycles.
      setAuthUser(null);
      setAuthSession(null);
      setSubscription(null);
      setUsageStats(null);
      setAnnouncement(null);
      setShowAnnouncement(false);
      announcementDismissedRef.current = false;
      setMessage('✅ Signed out successfully');
    } catch (error) {
      setMessage(`❌ Sign out failed: ${error}`);
    }
  };

  const handleSubscribe = async () => {
    if (!authUser || !authUser.email) {
      setMessage('❌ Please sign in first');
      return;
    }

    setIsSubscribing(true);
    setMessage('🔄 Opening Stripe checkout...');

    try {
      const checkoutUrl = await createCheckoutSession(authUser.id, authUser.email);
      
      if (checkoutUrl) {
        // Open Stripe checkout in system browser using Tauri command
        try {
          await invoke('open_external_url', { url: checkoutUrl });
          setMessage('✅ Checkout opened in browser. Complete payment there.');
        } catch (e) {
          // Fallback to window.open
          window.open(checkoutUrl, '_blank');
          setMessage('✅ Checkout opened. Complete payment to activate subscription.');
        }
      } else {
        setMessage('❌ Failed to create checkout session');
      }
    } catch (error) {
      setMessage(`❌ Subscription error: ${error}`);
    } finally {
      setIsSubscribing(false);
    }
  };

  // Helper to check if user can use AI proxy (has quota remaining)
  // Returns { allowed, reason, useBYOKey } - useBYOKey indicates free user should use their own Gemini key
  const canUseAIProxy = (freeCallLimit: number): { allowed: boolean; reason?: string; useBYOKey?: boolean } => {
    if (!subscription) {
      return { allowed: false, reason: 'Not signed in' };
    }

    const isPaid = subscription.subscription_status === 'active' || subscription.subscription_status === 'cancelling';

    if (isPaid) {
      // Paid user - check monthly quota
      if (usageStats && usageStats.requests_used >= usageStats.requests_limit) {
        return { 
          allowed: false, 
          reason: `Monthly quota exceeded (${usageStats.requests_limit} requests). Resets on ${usageStats.period_end.toLocaleDateString()}`
        };
      }
      return { allowed: true };
    } else {
      const lifetimeUsed = subscription.lifetime_ai_calls || 0;
      if (lifetimeUsed >= freeCallLimit) {
        // Check if user has their own Gemini API key
        if (aiConfig.gemini_api_key) {
          return { allowed: true, useBYOKey: true };
        }
        return { 
          allowed: false, 
          reason: `Free trial expired (${freeCallLimit} lifetime calls used). Add your own Gemini API key or subscribe to continue.`
        };
      }
      return { allowed: true };
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
          } else {
            setSelectedTab(null);
          }
        }
        
        return displaysOnly;
      });
      
      // Clear any "CDP opened" message when CDP closes
      setMessage(prev => prev.includes('Chrome CDP opened') ? '' : prev);
    }
  }, [cdpReady]);

  const checkCdpStatus = async () => {
    try {
      const [status, mode] = await Promise.all([
        invoke<string>('get_cdp_status'),
        invoke<string>('get_chrome_connection_mode'),
      ]);
      setCdpStatus(status);
      setCdpReady(status.includes('🟢'));
      setChromeConnectionMode(mode as 'user' | 'app' | 'none');
    } catch (error) {
      setCdpStatus('🔴 Error');
      setCdpReady(false);
      setChromeConnectionMode('none');
    }
  };

  const openChromeCdp = async () => {
    if (isOpeningChrome || isLoading) return;

    setIsOpeningChrome(true);
    setIsLoading(true);
    setMessage('🔍 Detecting Chrome… (if Chrome asks for permission, click Allow)');

    try {
      const result = await invoke<string>('open_chrome_cdp');
      setMessage(`✅ ${result}`);
      setTimeout(() => checkCdpStatus(), 1000);
    } catch (error: any) {
      const msg = String(error);
      // Surface the "check for permission dialog" error with extra visibility
      if (msg.includes('permission dialog') || msg.includes('Allow')) {
        setMessage(`⚠️ ${msg}`);
      } else {
        setMessage(`❌ ${msg}`);
      }
    } finally {
      setIsLoading(false);
      setTimeout(() => setIsOpeningChrome(false), 2000);
    }
  };

  const fetchTabs = async () => {
    setIsRefreshing(true);
    try {
      // Fetch displays (always available)
      const displaysData = await invoke<DisplayInfo[]>('get_displays');
      
      // Fetch thumbnails for displays (5s timeout each)
      const displaysWithThumbnails = await Promise.all(
        displaysData.map(async (display) => {
          try {
            const thumbnail = await Promise.race([
              invoke<string>('get_display_thumbnail', { displayId: display.id }),
              new Promise<string>((_, reject) => setTimeout(() => reject('timeout'), 5000)),
            ]);
            return { ...display, thumbnail };
          } catch {
            return display;
          }
        })
      );
      
      // Fetch Chrome tabs if CDP ready (8s timeout for tab list)
      let chromeTabs: ChromeTab[] = [];
      if (cdpReady) {
        try {
          chromeTabs = await Promise.race([
            invoke<ChromeTab[]>('get_chrome_tabs'),
            new Promise<ChromeTab[]>((_, reject) => setTimeout(() => reject('timeout'), 8000)),
          ]);
        } catch {
          chromeTabs = [];
        }
        
        // Fetch thumbnails for Chrome tabs (6s timeout each)
        chromeTabs = await Promise.all(
          chromeTabs.map(async (tab) => {
            try {
              const thumbnail = await Promise.race([
                invoke<string>('get_tab_thumbnail', { tabId: tab.id }),
                new Promise<string>((_, reject) => setTimeout(() => reject('timeout'), 6000)),
              ]);
              return { ...tab, thumbnail };
            } catch {
              return tab;
            }
          })
        );
      }
      
      // Add Audio source (always available) + combine all sources (Chrome tabs, displays, then Audio)
      // Audio records BOTH system audio (Zoom/Teams) AND microphone for interview capture
      const audioSource: AudioSource = {
        id: 'audio',
        name: 'Audio (System)',
        source_type: 'audio',
      };
      const combined: InputSource[] = [...chromeTabs, ...displaysWithThumbnails, audioSource];
      setAllSources(combined);
      
      const currentSelection = selectedTab ? combined.find(s => s.id === selectedTab.id) : undefined;
      if (currentSelection) {
        setSelectedTab(currentSelection);
      }
      // If selected tab is no longer in the list, select first available source
      else if (selectedTab) {
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
    } finally {
      setIsRefreshing(false);
    }
  };

  const solveWithAI = async (mode: 'auto' | 'text' | 'screenshot' = 'auto') => {
    // Hide announcement on any AI query (unconditional to avoid stale closure issues with hotkeys)
    announcementDismissedRef.current = true;
    setShowAnnouncement(false);
    
    let sourceToUse = selectedTab ?? (allSources.length > 0 ? allSources[0] : null);

    // The tab may have navigated since the list was last refreshed; use its current URL/title.
    if (sourceToUse && isChromeTab(sourceToUse) && cdpReady) {
      try {
        const tabs = await Promise.race([
          invoke<ChromeTab[]>('get_chrome_tabs'),
          new Promise<ChromeTab[]>((_, reject) => setTimeout(() => reject('timeout'), 2000)),
        ]);
        const current = tabs.find(t => t.id === sourceToUse!.id);
        if (current) {
          sourceToUse = { ...current, thumbnail: sourceToUse.thumbnail };
          setSelectedTab(sourceToUse);
        }
      } catch {
        // Fall back to the cached tab info.
      }
    }

    if (!selectedTab && sourceToUse) {
      // Keep UI selection in sync so the user can see what was used.
      setSelectedTab(sourceToUse);
    }

    if (!sourceToUse) {
      setMessage('❌ No input source available');
      return;
    }

    // WARNING: If audio prompt is selected but source is not audio, show warning
    if (selectedTemplate === PromptTemplate.VerbalInterviewAudio && !isAudio(sourceToUse)) {
      setShowAudioPromptWarning(true);
      return;
    }

    // Check if user is authenticated
    if (!authUser) {
      setMessage('❌ Please sign in first');
      return;
    }

    // Determine user tier
    const isPro = subscription?.subscription_status === 'active' || subscription?.subscription_status === 'cancelling';

    const models = await ensureModelConfig();
    if (!models) {
      setMessage('❌ Could not load AI models from the server. Check your connection and try again.');
      return;
    }

    // Free user restrictions
    if (!isPro) {
      // Free users can only use Chrome tabs (no display capture)
      if (isDisplay(sourceToUse)) {
        setMessage('⚠️ Display capture requires Pro subscription. Please select a Chrome tab.');
        return;
      }

      // Free users can only use allowed domains
      if (!isAudio(sourceToUse)) {
        const tab = sourceToUse as ChromeTab;
        const url = tab.url || '';
        if (!isFreeTierUrlAllowed(models, url)) {
          setMessage(`⚠️ Free tier only works on: ${models.free_allowed_domains.join(', ')}. Upgrade to Pro for unlimited access.`);
          return;
        }
      }
    }

    // Audio source: toggle recording or stop+solve
    if (isAudio(sourceToUse)) {
      await toggleAudioRecording();
      return;
    }

    // Check quota before making request
    const quotaCheck = canUseAIProxy(models.free_call_limit);
    if (!quotaCheck.allowed) {
      setMessage(`⚠️ ${quotaCheck.reason}`);
      return;
    }

    // Check if using BYO Gemini API key (free user with exhausted quota but has own key)
    const useBYOKey = quotaCheck.useBYOKey && aiConfig.gemini_api_key;

    // Get access token for proxy calls (not needed for BYO key)
    let accessToken = '';
    if (!useBYOKey) {
      const SUPABASE_URL = 'https://uudwpcjxbwtszhhcgybj.supabase.co';
      const storageKey = `sb-${SUPABASE_URL.split('//')[1].split('.')[0]}-auth-token`;
      const storedSession = localStorage.getItem(storageKey);
      
      if (storedSession) {
        try {
          const session = JSON.parse(storedSession);
          accessToken = session.access_token || '';
        } catch {
          setMessage('❌ Session error. Please sign in again.');
          return;
        }
      }
      
      if (!accessToken) {
        setMessage('❌ Please sign in to use AI features.');
        return;
      }
    }

    const modelToUse = useBYOKey
      ? models.byo_model.id
      : (isPro ? (aiConfig.selected_model || models.default_pro_model) : models.free_model.id);

    setIsLoading(true);
    setAiResponse('');
    setSolvePhase('idle');
    setSolveFlowType(null);
    
    try {
      let responseText: string;
      
      if (isDisplay(sourceToUse)) {
        // Display/Screen capture - always uses screenshot (Pro only)
        setSolveFlowType('screenshot');
        setSolvePhase('capture');
        setMessage('📸 Capturing display...');
        const screenshotPath = await invoke<string>('capture_display_screenshot', { 
          displayId: sourceToUse.id 
        });
        
        setSolvePhase('asking');
        setMessage('🤖 Analyzing screenshot with AI...');
        const prompt = buildPrompt(selectedTemplate, selectedLanguage);
        
        // Display capture is Pro-only, never uses BYO key
        const proxyResponse = await invoke<{ response: string; usage?: { requests_used: number; requests_limit: number; is_paid?: boolean }; error?: string }>('query_ai_via_proxy_with_image', {
          prompt,
          imagePath: screenshotPath,
          model: modelToUse,
          accessToken,
        });
        
        if (proxyResponse.error) {
          throw new Error(proxyResponse.error);
        }
        responseText = proxyResponse.response;
        
        // Update usage stats if returned
        if (proxyResponse.usage) {
          setUsageStats(prev => prev ? {
            ...prev,
            requests_used: proxyResponse.usage!.requests_used,
          } : prev);
          // Refresh subscription for free users (lifetime_ai_calls updated)
          if (!proxyResponse.usage.is_paid && authUser?.id) {
            const updatedSub = await getUserSubscription(authUser.id);
            if (updatedSub) setSubscription(updatedSub);
          }
        }
      } else if (mode === 'screenshot' || (mode === 'auto' && useScreenshot)) {
        // Chrome tab - screenshot mode
        setSolveFlowType('screenshot');
        setSolvePhase('screenshot');
        setMessage('📸 Taking screenshot...');
        await invoke('activate_tab', { tabId: sourceToUse.id });
        await new Promise(resolve => setTimeout(resolve, 500));
        
        const screenshotPath = await invoke<string>('capture_tab_screenshot', { 
          tabId: sourceToUse.id 
        });
        
        setSolvePhase('asking');
        setMessage('🤖 Analyzing screenshot with AI...');
        const prompt = buildPrompt(selectedTemplate, selectedLanguage);
        
        if (useBYOKey) {
          // Use direct Gemini API with user's own key
          // Pass source_url for domain validation in Rust
          const tabUrl = !isDisplay(sourceToUse) && !isAudio(sourceToUse) ? (sourceToUse as ChromeTab).url : undefined;
          responseText = await invoke<string>('query_ai_with_image', {
            prompt,
            imagePath: screenshotPath,
            config: { 
              selected_model: modelToUse, 
              gemini_api_key: aiConfig.gemini_api_key || '', 
              max_output_tokens: models.max_output_tokens,
            },
            sourceUrl: tabUrl,
          });
        } else {
          // Pass source_url for server-side domain validation
          const tabUrl = !isDisplay(sourceToUse) && !isAudio(sourceToUse) ? (sourceToUse as ChromeTab).url : undefined;
          const proxyResponse = await invoke<{ response: string; usage?: { requests_used: number; requests_limit: number; is_paid?: boolean }; error?: string }>('query_ai_via_proxy_with_image', {
            prompt,
            imagePath: screenshotPath,
            model: modelToUse,
            accessToken,
            sourceUrl: tabUrl,
          });
          
          if (proxyResponse.error) {
            throw new Error(proxyResponse.error);
          }
          responseText = proxyResponse.response;
          
          if (proxyResponse.usage) {
            setUsageStats(prev => prev ? {
              ...prev,
              requests_used: proxyResponse.usage!.requests_used,
            } : prev);
            // Refresh subscription for free users (lifetime_ai_calls updated)
            if (!proxyResponse.usage.is_paid && authUser?.id) {
              const updatedSub = await getUserSubscription(authUser.id);
              if (updatedSub) setSubscription(updatedSub);
            }
          }
        }
      } else {
        // Chrome tab - text mode
        setSolvePhase('extract');
        setMessage('📝 Extracting text...');
        await invoke('activate_tab', { tabId: sourceToUse.id });
        await new Promise(resolve => setTimeout(resolve, 300));
        
        const text = await invoke<string>('extract_tab_text', { tabId: sourceToUse.id });
        
        setSolvePhase('asking');
        setMessage('🤖 Asking AI...');
        const prompt = buildPrompt(selectedTemplate, selectedLanguage, text);
        
        if (useBYOKey) {
          // Use direct Gemini API with user's own key
          // Pass source_url for domain validation in Rust
          const tabUrl = !isDisplay(sourceToUse) && !isAudio(sourceToUse) ? (sourceToUse as ChromeTab).url : undefined;
          responseText = await invoke<string>('query_ai', {
            prompt,
            config: { 
              selected_model: modelToUse, 
              gemini_api_key: aiConfig.gemini_api_key || '', 
              max_output_tokens: models.max_output_tokens,
            },
            sourceUrl: tabUrl,
          });
        } else {
          // Pass source_url for server-side domain validation
          const tabUrlForProxy = !isDisplay(sourceToUse) && !isAudio(sourceToUse) ? (sourceToUse as ChromeTab).url : undefined;
          const proxyResponse = await invoke<{ response: string; usage?: { requests_used: number; requests_limit: number; is_paid?: boolean }; error?: string }>('query_ai_via_proxy', {
            prompt,
            model: modelToUse,
            accessToken,
            sourceUrl: tabUrlForProxy,
          });
          
          if (proxyResponse.error) {
            throw new Error(proxyResponse.error);
          }
          responseText = proxyResponse.response;
          
          if (proxyResponse.usage) {
            setUsageStats(prev => prev ? {
              ...prev,
              requests_used: proxyResponse.usage!.requests_used,
            } : prev);
            // Refresh subscription for free users (lifetime_ai_calls updated)
            if (!proxyResponse.usage.is_paid && authUser?.id) {
              const updatedSub = await getUserSubscription(authUser.id);
              if (updatedSub) setSubscription(updatedSub);
            }
          }
        }
      }
      
      setAiResponse(responseText);
      setSolvePhase('idle');
      setMessage('');
    } catch (error) {
      setSolvePhase('error');
      setMessage(`❌ Error: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  // Keep a stable reference for the hotkey listener.
  useEffect(() => {
    solveWithAIRef.current = solveWithAI;
  });

  const runMessageAction = (kind: MessageActionKind) => {
    switch (kind) {
      case 'add_api_key':
      case 'fix_api_key':
        openSettingsAt('models');
        break;
      case 'upgrade':
      case 'view_usage':
        openSettingsAt('account');
        break;
      case 'open_chrome':
        openChromeCdp();
        break;
      case 'download_chrome':
        invoke('open_url', { url: 'https://www.google.com/chrome/' });
        break;
      case 'sign_in':
        handleSignOut();
        break;
      case 'screen_permission':
        setup.grant('screen_recording');
        break;
    }
  };

  // ========== AUTH LOADING STATE ==========
  if (authLoading) {
    return (
      <div className="app-container" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: '100vh' }}>
        <div className="loading">
          <div className="spinner"></div>
          <p>Loading...</p>
        </div>
      </div>
    );
  }

  // ========== AUTH SCREEN (NOT LOGGED IN) ==========
  if (!authUser) {
    return <AuthScreen onAuthSuccess={handleAuthSuccess} />;
  }

  // ========== MAIN APP (LOGGED IN) ==========
  // Pro users include 'active' and 'cancelling' (still have access until period end)
  const isPaidUser = subscription?.subscription_status === 'active' || subscription?.subscription_status === 'cancelling';
  const freeCallLimit = modelConfig?.free_call_limit;
  const freeCallLimitLabel = freeCallLimit ?? '…';
  const freeCallsUsed = subscription?.lifetime_ai_calls || 0;
  const freeCallsExhausted = freeCallLimit !== undefined && freeCallsUsed >= freeCallLimit;
  const freeSitesRestricted = !modelConfig || !allowsAllDomains(modelConfig);
  const usingByoKey = !isPaidUser && freeCallsExhausted && !!aiConfig.gemini_api_key;
  const proRequestLimitLabel = usageStats?.requests_limit ?? modelConfig?.pro_request_limit ?? '…';
  const proAudioHoursLabel = modelConfig ? +(modelConfig.pro_audio_seconds_limit / 3600).toFixed(1) : '…';
  const proModelNames = modelConfig?.pro_models.map(m => m.name).join(', ') ?? '';
  const solveKind = !selectedTab ? null : isAudio(selectedTab) ? 'audio' : (isDisplay(selectedTab) || useScreenshot) ? 'screenshot' : 'text';
  const solveHotkeyRaw = !activeHotkeys || !solveKind ? '' : solveKind === 'audio' ? activeHotkeys.audio_toggle : activeHotkeys[solveKind];
  const solveHotkey = formatHotkey(solveHotkeyRaw, runtimePlatform === 'macos');
  const withHotkey = (label: string, hotkey?: string) =>
    hotkey ? `${label} · solve now with ${formatHotkey(hotkey, runtimePlatform === 'macos')}` : label;
  const solveActionLabel = solveKind === 'audio' ? 'Start / stop recording' : solveKind === 'screenshot' ? 'Solve from screenshot' : 'Solve from text';
  const messageActions = getMessageActions(message, { isPro: isPaidUser, isMac: runtimePlatform === 'macos', cdpReady });

  return (
    <div className="app-container">
      <header className="app-header">
        <div className="header-left">
          <img src="/icon.png" alt="CrackingInterview" className="app-icon-img" />
        </div>
        <div className="header-right">
          {/* Quota Display */}
          {subscription && (
            <span
              className="quota-badge"
              title={
                isPaidUser
                  ? `${usageStats?.requests_used ?? '?'} of ${proRequestLimitLabel} AI calls used this month${usageStats?.period_end ? ` · Resets ${usageStats.period_end.toLocaleDateString()}` : ''}`
                  : usingByoKey
                    ? 'Using your own Gemini API key (unlimited) · Click to manage it in AI Models'
                    : `${freeCallsUsed} of ${freeCallLimitLabel} lifetime free calls used`
              }
              {...(!isPaidUser && {
                role: 'button',
                tabIndex: 0,
                style: { cursor: 'pointer' },
                onClick: handleOpenSettings,
                onKeyDown: (e: React.KeyboardEvent) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    handleOpenSettings();
                  }
                },
              })}
            >
              {isPaidUser ? (
                <>📊 {usageStats ? `${usageStats.requests_used}/${usageStats.requests_limit} calls` : '...'}</>
              ) : usingByoKey ? (
                <>🔑 BYO Key</>
              ) : (
                <>🎁 {freeCallsUsed}/{freeCallLimitLabel} calls</>
              )}
            </span>
          )}
          {isPaidUser && usageStats && (
            <span className="quota-badge" title={`${(usageStats.audio_seconds_used / 3600).toFixed(1)} of ${usageStats.audio_seconds_limit / 3600} audio hours used this month`}>
              🎙️ {(usageStats.audio_seconds_used / 3600).toFixed(1)}/{usageStats.audio_seconds_limit / 3600}h
            </span>
          )}
          {setup.oneTimeMissing > 0 && (
            <button
              className="setup-badge"
              onClick={handleShowSetup}
              title="Some setup steps are missing. Click to see them."
            >
              ⚠ Setup {setup.steps.filter(s => s.done).length}/{setup.steps.length}
            </button>
          )}
          {cdpReady ? (
            <span
              className={`status-indicator${chromeConnectionMode === 'user' ? ' chrome-user-mode' : ''}`}
              title={
                chromeConnectionMode === 'user'
                  ? 'Connected to your existing Chrome browser'
                  : 'Connected to a Chrome window opened by this app'
              }
            >
              {cdpStatus}
            </span>
          ) : (
            <button
              className="status-indicator chrome-open-btn"
              onClick={openChromeCdp}
              disabled={isOpeningChrome}
              title="Open or connect to Chrome"
            >
              {isOpeningChrome ? (
                <>🔍 Check Chrome for a dialog…</>
              ) : (
                <>
                  <svg className="chrome-logo" viewBox="0 0 48 48" width="14" height="14">
                    <circle cx="24" cy="24" r="12" fill="#fff"/>
                    <path d="M24,12H44.78a24,24,0,0,0-41.56.003L13.61,30l.01-.002A12,12,0,0,1,24,12Z" fill="#EA4335"/>
                    <circle cx="24" cy="24" r="9.5" fill="#1a73e8"/>
                    <path d="M34.39,30L24,48A24,24,0,0,0,44.78,12H24l-.003.009A12,12,0,0,1,34.39,30Z" fill="#FBBC04"/>
                    <path d="M13.61,30L3.22,12.01A24,24,0,0,0,24,48L34.39,30l-.007-.007A12,12,0,0,1,13.61,30Z" fill="#34A853"/>
                  </svg>
                  {' '}Open Chrome
                </>
              )}
            </button>
          )}
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
                const wasAudio = selectedTab && isAudio(selectedTab);
                const nowAudio = isAudio(source);
                
                setSelectedTab(source);
                const rawTitle = isAudio(source) ? source.name : isDisplay(source) ? source.name : (source.title?.trim() || (source as any).url || 'Unknown');
                const titleDiv = document.createElement('div');
                titleDiv.innerHTML = rawTitle;
                const title = titleDiv.textContent || titleDiv.innerText || rawTitle;
                setMessage(`Selected: ${title}`);
                
                // Auto-select Verbal Interview (Audio) prompt when switching to Audio source
                if (nowAudio && !wasAudio) {
                  previousTemplateRef.current = selectedTemplate;
                  setSelectedTemplate(PromptTemplate.VerbalInterviewAudio);
                  localStorage.setItem('prompt_template', PromptTemplate.VerbalInterviewAudio);
                  invoke('warm_audio_capture')
                    .then(() => {})
                    .catch(() => {});
                }
                // Restore previous prompt when switching away from Audio
                else if (wasAudio && !nowAudio) {
                  if (previousTemplateRef.current && previousTemplateRef.current !== PromptTemplate.VerbalInterviewAudio) {
                    setSelectedTemplate(previousTemplateRef.current);
                    localStorage.setItem('prompt_template', previousTemplateRef.current);
                  }
                  previousTemplateRef.current = null;
                  invoke('cooldown_audio_capture').catch(() => {});
                }
                // Warning: if selecting non-audio source while Audio prompt is active
                else if (!nowAudio && selectedTemplate === PromptTemplate.VerbalInterviewAudio) {
                  setShowAudioPromptWarning(true);
                }
              }}
              disabled={false}
              onOpen={refreshChromeTabsQuietly}
            />
            <button
              onClick={fetchTabs}
              className={`refresh-btn ${isRefreshing ? 'refreshing' : ''}`}
              title="Refresh Input Sources"
              disabled={isRefreshing}
            >
              🔄
            </button>
          </div>
        </div>

        <div className="solve-group">
          {selectedTab && !isAudio(selectedTab) && (() => {
            const displayLocked = isDisplay(selectedTab);
            const screenshotActive = displayLocked || useScreenshot;
            const lockedHint = 'Displays always use Screenshot Capture';
            // Same `useScreenshot` setting as Settings → AI Models → Input Mode, so both stay in sync.
            return (
              <div
                className={`mode-segment ${screenshotActive ? 'is-screenshot' : ''} ${displayLocked ? 'is-locked' : ''}`}
                role="radiogroup"
                aria-label="Input mode"
              >
                <span className="mode-segment-thumb" aria-hidden="true" />
                <button
                  type="button"
                  role="radio"
                  aria-checked={!screenshotActive}
                  aria-label="Text Extraction"
                  className={`mode-segment-btn ${!screenshotActive ? 'active' : ''}`}
                  onClick={() => setUseScreenshot(false)}
                  disabled={isLoading || displayLocked}
                  title={displayLocked ? lockedHint : withHotkey('Text Extraction', activeHotkeys?.text)}
                >
                  <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
                    <path d="M14 3v5h5M9 13h6M9 17h4" />
                  </svg>
                </button>
                <button
                  type="button"
                  role="radio"
                  aria-checked={screenshotActive}
                  aria-label="Screenshot Capture"
                  className={`mode-segment-btn ${screenshotActive ? 'active' : ''}`}
                  onClick={() => setUseScreenshot(true)}
                  disabled={isLoading || displayLocked}
                  title={displayLocked ? lockedHint : withHotkey('Screenshot Capture', activeHotkeys?.screenshot)}
                >
                  <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <path d="M4 8a2 2 0 0 1 2-2h1.5l1.5-2h6l1.5 2H18a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z" />
                    <circle cx="12" cy="12.5" r="3.5" />
                  </svg>
                </button>
              </div>
            );
          })()}
          <button 
            onClick={() => solveWithAI('auto')}
            disabled={isLoading || !selectedTab}
            className="solve-button"
            title={solveHotkey ? `${solveActionLabel} (${solveHotkey})` : undefined}
            aria-keyshortcuts={solveHotkeyRaw || undefined}
          >
            {selectedTab && isAudio(selectedTab)
              ? (isRecordingAudio ? `⏹️ Stop (${audioSeconds}s)` : '🎙️ Record')
              : `${selectedTab && (isDisplay(selectedTab) || useScreenshot) ? '📸' : '📝'} Solve`}
          </button>
        </div>
      </div>

      {isRecordingAudio && !isLiveTranscribing && (
        <div className="message-box" style={{ margin: '0 20px 12px 20px' }}>
          🎙️ Recording system audio… <strong>{audioSeconds}s</strong> (press Stop / Audio hotkey to send)
        </div>
      )}

      {isLiveTranscribing && (
        <div style={{ margin: '0 20px 12px 20px' }}>
          <LiveTranscript
            interimText={liveTranscriptInterim}
            finalText={liveTranscriptFinal}
            isTranscribing={isLiveTranscribing}
            silenceCountdown={silenceCountdown}
          />
        </div>
      )}

      {conversationHistory.length > 0 && (
        <div className="conversation-controls" style={{ margin: '0 20px 8px 20px', display: 'flex', gap: '8px', alignItems: 'center' }}>
          <span style={{ fontSize: '12px', color: '#888' }}>
            {Math.floor(conversationHistory.filter(m => m.role === 'user').length)} exchange{conversationHistory.filter(m => m.role === 'user').length !== 1 ? 's' : ''} in this session
          </span>
          <button
            onClick={clearConversationHistory}
            className="new-session-btn"
            title="Clear conversation and start fresh"
          >
            New Session
          </button>
        </div>
      )}

      <div className="content" ref={contentScrollRef}>
        <div className="main-section">
          
          {setup.showCard && (
            <SetupChecklist
              variant="card"
              {...setupChecklistProps}
              onHide={setup.hide}
            />
          )}

          {announcement && showAnnouncement && (
            <div className="info-banner announcement">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                <div style={{ flex: 1 }}>
                  <h3 style={{ margin: '0 0 8px 0', fontSize: '16px', fontWeight: '600' }}>
                    {announcement.title}
                  </h3>
                  <div 
                    style={{ fontSize: '14px', lineHeight: '1.5' }}
                    dangerouslySetInnerHTML={{ __html: announcement.message }}
                    onClick={async (e) => {
                      // Intercept clicks on links to open in system browser
                      const target = e.target as HTMLElement;
                      if (target.tagName === 'A') {
                        e.preventDefault();
                        const href = target.getAttribute('href');
                        if (href) {
                          try {
                            await invoke('open_external_url', { url: href });
                          } catch (err) {
                            console.error('[Announcement] Failed to open link:', err);
                            // Fallback to window.open
                            window.open(href, '_blank');
                          }
                        }
                      }
                    }}
                  />
                </div>
                <button 
                  onClick={() => { announcementDismissedRef.current = true; setShowAnnouncement(false); }}
                  style={{
                    background: 'transparent',
                    border: 'none',
                    fontSize: '20px',
                    cursor: 'pointer',
                    padding: '0 0 0 12px',
                    opacity: 0.6,
                    lineHeight: '1',
                  }}
                  title="Dismiss"
                >
                  ×
                </button>
              </div>
            </div>
          )}

          {/* Progress stepper during solve flow (2 steps only — disappears when AI responds) */}
          {solvePhase !== 'idle' && solvePhase !== 'error' && isLoading && !isRecordingAudio && (
            <div className="solve-stepper">
              {(() => {
                type Step = { label: string; icon: string; key: string };
                let steps: Step[];
                if (solveFlowType === 'audio') {
                  steps = [
                    { label: 'Record', icon: '🎙️', key: 'audio' },
                    { label: 'Asking AI', icon: '🤖', key: 'asking' },
                  ];
                } else if (solveFlowType === 'screenshot') {
                  steps = [
                    { label: 'Screenshot', icon: '📸', key: 'screenshot' },
                    { label: 'Asking AI', icon: '🤖', key: 'asking' },
                  ];
                } else {
                  steps = [
                    { label: 'Extract', icon: '📝', key: 'extract' },
                    { label: 'Asking AI', icon: '🤖', key: 'asking' },
                  ];
                }

                const phaseOrder = ['extract', 'screenshot', 'capture', 'audio', 'asking'];
                const currentIdx = phaseOrder.indexOf(solvePhase);

                const modelName = (() => {
                  if (!modelConfig) return '';
                  if (!isPaidUser) {
                    return usingByoKey ? modelConfig.byo_model.name : modelConfig.free_model.name;
                  }
                  const found = modelConfig.pro_models.find(m => m.id === aiConfig.selected_model);
                  return found ? found.name : aiConfig.selected_model;
                })();
                const promptLabel = getTemplateLabel(selectedTemplate);

                return (
                  <>
                    <div className="stepper-track">
                      {steps.map((step, i) => {
                        const stepIdx = phaseOrder.indexOf(step.key);
                        const isActive = step.key === solvePhase || 
                          (step.key === 'screenshot' && (solvePhase === 'capture' || solvePhase === 'screenshot'));
                        const isCompleted = stepIdx < currentIdx && stepIdx >= 0;
                        return (
                          <div key={step.key} className="stepper-item-wrapper">
                            {i > 0 && (
                              <div className={`stepper-connector ${isCompleted || isActive ? 'active' : ''}`} />
                            )}
                            <div className={`stepper-step ${isActive ? 'active' : ''} ${isCompleted ? 'completed' : ''}`}>
                              <span className="stepper-icon">{step.icon}</span>
                              <span className="stepper-label">{step.label}</span>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                    {solvePhase === 'asking' && (
                      <div className="stepper-info">
                        {modelName && (
                          <>
                            <span className="stepper-info-model">🧠 {modelName}</span>
                            <span className="stepper-info-divider">·</span>
                          </>
                        )}
                        <span className="stepper-info-prompt">📋 {promptLabel}</span>
                      </div>
                    )}
                  </>
                );
              })()}
            </div>
          )}

          {/* Non-stepper messages (errors, status messages outside solve flow) */}
          {message && !isRecordingAudio && (solvePhase === 'idle' || solvePhase === 'error') && (
            <div className="message-box">
              {message}
              {messageActions.length > 0 && (
                <div className="message-actions">
                  {messageActions.map(kind => (
                    <button key={kind} className="message-action-btn" onClick={() => runMessageAction(kind)}>
                      {MESSAGE_ACTION_LABELS[kind]}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}

          {conversationHistory.length > 1 && selectedTab && isAudio(selectedTab) ? (
            <div className="conversation-view">
              {(() => {
                const visible = conversationHistory.filter(msg => msg.role !== 'system');
                // Group into Q&A pairs (user + assistant), reverse so newest is on top
                // pairIndex maps to displayTranscripts index (0-based)
                const pairs: Array<{ messages: typeof visible; transcriptIdx: number }> = [];
                let userCount = 0;
                for (let i = 0; i < visible.length; i += 2) {
                  pairs.push({ messages: visible.slice(i, i + 2), transcriptIdx: userCount });
                  userCount++;
                }
                pairs.reverse();
                return pairs.map((pair, pairIdx) => (
                  <div key={pairIdx} className="conversation-pair">
                    {pairIdx === 0 && <div className="conversation-latest-badge">Latest</div>}
                    {pair.messages.map((msg, msgIdx) => (
                      <div key={msgIdx} className={`conversation-msg conversation-msg-${msg.role}`}>
                        <div className="conversation-msg-label">
                          {msg.role === 'user' ? '🎤 Interviewer' : '🤖 AI'}
                        </div>
                        {msg.role === 'assistant' ? (
                          <AIResponseDisplay response={msg.content} language={selectedLanguage.toLowerCase()} />
                        ) : (
                          <div className="conversation-msg-text">
                            {displayTranscripts[pair.transcriptIdx] || msg.content}
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                ));
              })()}
            </div>
          ) : (
            <AIResponseDisplay 
              response={aiResponse}
              language={selectedLanguage.toLowerCase()}
            />
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
                className={`tab-btn ${settingsTab === 'account' ? 'active' : ''}`}
                onClick={() => setSettingsTab('account')}
              >
                👤 Account
              </button>
              <button 
                className={`tab-btn ${settingsTab === 'models' ? 'active' : ''}`}
                onClick={() => setSettingsTab('models')}
              >
                🤖 AI Models
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
              <button 
                className={`tab-btn ${settingsTab === 'app' ? 'active' : ''}`}
                onClick={() => setSettingsTab('app')}
              >
                🖥️ App
              </button>
            </div>
            
            <div className="modal-body">
              {settingsTab === 'account' && (
                <>
                  <div className="account-section">
                    <div className="account-info">
                      <div className="account-avatar">
                        {authUser?.email?.charAt(0).toUpperCase() || '?'}
                      </div>
                      <div className="account-details">
                        <div className="account-email">{authUser?.email}</div>
                        <div className="account-tier">
                          {isPaidUser ? (
                            <span className="tier-badge pro">✨ Pro Subscriber</span>
                          ) : (
                            <span className="tier-badge free">🎁 Free Tier</span>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>

                  <div className="form-group">
                    <label>AI Calls {isPaidUser ? 'This Month' : '(Lifetime)'}:</label>
                    <div className="usage-bar-container">
                      {(() => {
                        const pct = isPaidUser && usageStats
                          ? Math.min(100, (usageStats.requests_used / usageStats.requests_limit) * 100)
                          : freeCallLimit ? Math.min(100, (freeCallsUsed / freeCallLimit) * 100) : 0;
                        const barColor = pct < 50
                          ? `linear-gradient(90deg, #4caf50, #66bb6a)`
                          : pct < 80
                            ? `linear-gradient(90deg, #66bb6a, #ffc107)`
                            : `linear-gradient(90deg, #ff9800, #f44336)`;
                        return (
                          <div
                            className="usage-bar"
                            style={{ width: `${pct}%`, background: barColor }}
                          />
                        );
                      })()}
                    </div>
                    <div className="usage-text">
                      {isPaidUser ? (
                        <>
                          <span>{usageStats?.requests_used || 0} / {proRequestLimitLabel} requests</span>
                          <span className="usage-reset">Resets {usageStats?.period_end?.toLocaleDateString()}</span>
                        </>
                      ) : (
                        <>
                          <span>{freeCallsUsed} / {freeCallLimitLabel} lifetime free calls used</span>
                        </>
                      )}
                    </div>
                  </div>

                  {isPaidUser && usageStats && (
                    <div className="form-group">
                      <label>Audio Recording This Month:</label>
                      <div className="usage-bar-container">
                        {(() => {
                          const pct = Math.min(100, (usageStats.audio_seconds_used / usageStats.audio_seconds_limit) * 100);
                          const barColor = pct < 50
                            ? `linear-gradient(90deg, #4caf50, #66bb6a)`
                            : pct < 80
                              ? `linear-gradient(90deg, #66bb6a, #ffc107)`
                              : `linear-gradient(90deg, #ff9800, #f44336)`;
                          return (
                            <div
                              className="usage-bar"
                              style={{ width: `${pct}%`, background: barColor }}
                            />
                          );
                        })()}
                      </div>
                      <div className="usage-text">
                        <span>{(usageStats.audio_seconds_used / 3600).toFixed(1)} / {usageStats.audio_seconds_limit / 3600} hours</span>
                        <span className="usage-reset">Resets {usageStats.period_end?.toLocaleDateString()}</span>
                      </div>
                    </div>
                  )}

                  {!isPaidUser && (
                    <div className="upgrade-section">
                      <h4>🚀 Upgrade to Pro</h4>
                      <ul className="upgrade-benefits">
                        <li>✓ Stealth mode: Undetectable and fully private plus hidden hotkeys</li>
                        <li>✓ {proRequestLimitLabel} AI requests per month</li>
                        <li>✓ {proAudioHoursLabel} hours audio recording per month</li>
                        {proModelNames && <li>✓ {proModelNames}</li>}
                        <li>✓ Screen capture for display</li>
                        <li>✓ Audio input with transcription</li>
                      </ul>
                      <button 
                        className="action-btn primary upgrade-btn"
                        onClick={handleSubscribe}
                        disabled={isSubscribing}
                      >
                        {isSubscribing ? '⏳ Loading...' : '💳 Subscribe to Pro'}
                      </button>
                    </div>
                  )}

                  {isPaidUser && subscription?.stripe_customer_id && (
                    <div className="manage-subscription">
                      <button 
                        className="action-btn secondary"
                        style={{ width: '100%' }}
                        onClick={async () => {
                          try {
                            setMessage('Opening billing portal...');
                            const portalUrl = await invoke<string>('create_billing_portal_session', {
                              customerId: subscription.stripe_customer_id
                            });
                            // Open in external browser
                            await invoke('open_external_url', { url: portalUrl });
                            setMessage('✅ Billing portal opened in browser');
                          } catch (e) {
                            setMessage(`❌ Failed to open billing portal: ${e}`);
                          }
                        }}
                      >
                        📋 Manage Subscription
                      </button>
                    </div>
                  )}

                  <div className="sign-out-section">
                    <button 
                      className="action-btn secondary"
                      onClick={handleSignOut}
                      style={{ width: '100%', marginTop: '20px' }}
                    >
                      🚪 Sign Out
                    </button>
                  </div>
                </>
              )}

              {settingsTab === 'models' && (
                <>
                  <div className="form-group">
                    <label>AI Model:</label>
                    {isPaidUser ? (
                      <select 
                        value={aiConfig.selected_model}
                        onChange={(e) => setAiConfig({...aiConfig, selected_model: e.target.value})}
                        className="input-field"
                        disabled={!modelConfig}
                      >
                        {!modelConfig && <option value="">Loading models…</option>}
                        {modelConfig?.pro_models.map(model => (
                          <option key={model.id} value={model.id}>
                            {model.name} ({model.provider})
                          </option>
                        ))}
                      </select>
                    ) : (
                      <div className="input-field" style={{ backgroundColor: 'var(--input-disabled-bg)', cursor: 'not-allowed' }}>
                        {!modelConfig
                          ? 'Loading models…'
                          : usingByoKey
                            ? `${modelConfig.byo_model.name} (${modelConfig.byo_model.provider}) - Your API Key`
                            : `${modelConfig.free_model.name} (${modelConfig.free_model.provider}) - Free Tier`}
                      </div>
                    )}
                  </div>

                  {/* BYO API Key section - shown when free user exhausted their free calls */}
                  {!isPaidUser && freeCallsExhausted && (
                    <div className="form-group" style={{ marginTop: '16px', padding: '16px', backgroundColor: '#f0f9ff', borderRadius: '8px', border: '1px solid #0ea5e9' }}>
                      <label style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px', color: '#0369a1', fontWeight: 600 }}>
                        🔑 Bring Your Own API Key
                        {aiConfig.gemini_api_key && <span style={{ color: '#16a34a', fontSize: '12px' }}>✓ Active</span>}
                      </label>
                      <p style={{ fontSize: '12px', color: '#64748b', marginBottom: '12px' }}>
                        Your {freeCallLimitLabel} free tries are used. Add your own Gemini API key to continue using AI{freeSitesRestricted ? ' (with same domain restrictions)' : ''}.
                      </p>
                      <div style={{ display: 'flex', gap: '8px', marginBottom: '8px' }}>
                        <input
                          type="password"
                          value={byoKeyDraft}
                          onChange={(e) => { setByoKeyDraft(e.target.value); setByoKeyError(''); }}
                          onKeyDown={(e) => { if (e.key === 'Enter') { e.preventDefault(); handleSaveByoKey(); } }}
                          placeholder={aiConfig.gemini_api_key
                            ? `Saved key ••••${aiConfig.gemini_api_key.slice(-4)} · paste a new key to replace`
                            : 'Paste your Gemini API key'}
                          className="input-field"
                          style={{ flex: 1, margin: 0 }}
                          disabled={byoKeyVerifying}
                        />
                        <button
                          className="action-btn primary"
                          onClick={handleSaveByoKey}
                          disabled={!byoKeyDraft.trim() || byoKeyVerifying}
                          style={{ whiteSpace: 'nowrap' }}
                        >
                          {byoKeyVerifying ? '⏳ Verifying…' : 'Save & Verify'}
                        </button>
                        {aiConfig.gemini_api_key && !byoKeyDraft && (
                          <button
                            className="action-btn"
                            onClick={handleRemoveByoKey}
                            disabled={byoKeyVerifying}
                            style={{ whiteSpace: 'nowrap' }}
                          >
                            Remove
                          </button>
                        )}
                      </div>
                      {byoKeyError && (
                        <p style={{ fontSize: '12px', color: '#dc2626', margin: '0 0 8px 0', whiteSpace: 'pre-wrap' }}>
                          {byoKeyError}
                        </p>
                      )}
                      <button 
                        onClick={() => invoke('open_url', { url: 'https://aistudio.google.com/app/apikey' })}
                        style={{ 
                          background: 'none', 
                          border: 'none', 
                          padding: 0, 
                          fontSize: '12px', 
                          color: '#0ea5e9', 
                          cursor: 'pointer',
                          textDecoration: 'underline'
                        }}
                      >
                        🔗 Get a free API key from Google AI Studio
                      </button>
                      {aiConfig.gemini_api_key && (
                        <p style={{ fontSize: '11px', color: '#16a34a', marginTop: '8px' }}>
                          ✓ Using {modelConfig?.byo_model.name ?? 'the free-tier model'} with your own key.{freeSitesRestricted ? ' Domain restrictions still apply.' : ''}
                        </p>
                      )}
                    </div>
                  )}

                  {!isPaidUser && (
                    <div className="info-note" style={{ marginTop: '16px' }}>
                      <h4 style={{ marginBottom: '8px' }}>🔒 Free Tier Limitations</h4>
                      <ul style={{ fontSize: '13px', margin: 0, paddingLeft: '20px' }}>
                        {modelConfig && (
                          <li>{usingByoKey
                            ? `${modelConfig.byo_model.name} (with your API key)`
                            : `${modelConfig.free_model.name} model only`}</li>
                        )}
                        {freeCallLimit !== undefined && (
                          <li>{freeCallsExhausted
                            ? (aiConfig.gemini_api_key ? 'Unlimited with your API key' : `${freeCallLimit} lifetime AI requests (used)`)
                            : `${freeCallLimit - freeCallsUsed} of ${freeCallLimit} free requests remaining`}</li>
                        )}
                        <li>Chrome tabs only (no display screen capture)</li>
                        {modelConfig && !allowsAllDomains(modelConfig) && (
                          <li>Only works on: {modelConfig.free_allowed_domains.join(', ')}</li>
                        )}
                      </ul>
                      <button 
                        className="action-btn primary"
                        style={{ marginTop: '12px', width: '100%' }}
                        onClick={handleSubscribe}
                        disabled={isSubscribing}
                      >
                        {isSubscribing ? '⏳ Loading...' : '🚀 Upgrade to Pro'}
                      </button>
                    </div>
                  )}

                  {isPaidUser && (
                    <div className="info-note" style={{ marginTop: '16px' }}>
                      <h4 style={{ marginBottom: '8px' }}>✨ Pro Features</h4>
                      <ul style={{ fontSize: '13px', margin: 0, paddingLeft: '20px' }}>
                        <li>Stealth Mode with Anti-Detection Protection</li>
                        <li>{modelConfig ? `All ${modelConfig.pro_models.length} premium AI models` : 'All premium AI models'}</li>
                        <li>{proRequestLimitLabel} requests per month</li>
                        <li>Display/screen capture</li>
                        <li>Audio input with transcription</li>
                      </ul>
                    </div>
                  )}

                  {/* Input Mode (merged from old tab) */}
                  <div className="form-group" style={{ marginTop: '20px', paddingTop: '16px', borderTop: `1px solid var(--border)` }}>
                    <label>Input Mode:</label>
                    <div className="toggle-group">
                      <button
                        onClick={() => setUseScreenshot(false)}
                        className={`toggle-btn mode-btn ${!useScreenshot ? 'active' : ''}`}
                      >
                        <span className="mode-icon">📝</span>
                        <span className="mode-label">Text Extraction</span>
                        <span className="mode-desc">Chrome tabs only</span>
                      </button>
                      <button
                        onClick={() => setUseScreenshot(true)}
                        className={`toggle-btn mode-btn ${useScreenshot ? 'active' : ''}`}
                      >
                        <span className="mode-icon">📸</span>
                        <span className="mode-label">Screenshot Capture</span>
                        <span className="mode-desc">Visual · Images & diagrams</span>
                      </button>
                    </div>
                  </div>

                  <div className="info-note">
                    <h4 style={{marginBottom: '8px'}}>Mode Comparison:</h4>
                    <p className="small" style={{marginBottom: '12px'}}>
                      <strong>📝 Text:</strong> Extracts text from Chrome tabs only. Fast and efficient for text-based problems.
                    </p>
                    <p className="small">
                      <strong>📸 Screenshot:</strong> Captures visual content including diagrams, tables, and formatting. Works with any source. Display source always uses screenshot mode.
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
                      isPro={subscription?.subscription_status === 'active' || subscription?.subscription_status === 'cancelling'}
                      interviewLanguage={interviewLanguage}
                      onInterviewLanguageChange={(langCode: string) => {
                        setInterviewLanguage(langCode);
                        localStorage.setItem('interview_language', langCode);
                      }}
                      onOpenDocuments={() => setShowDocumentManager(true)}
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
                <HotkeysSettingsTab
                  draft={hotkeysDraft}
                  onDraftChange={setHotkeysDraft}
                  platform={runtimePlatform}
                  status={hotkeysStatus}
                  onSave={saveHotkeys}
                  onReset={resetHotkeys}
                />
              )}

              {settingsTab === 'app' && (
                <AppSettingsTab
                  windowOpacity={windowOpacity}
                  onOpacityChange={handleOpacityChange}
                  theme={theme}
                  onThemeChange={handleThemeChange}
                  isPro={isPro}
                  stealthMode={stealthMode}
                  onStealthModeChange={handleStealthModeChange}
                  hotkeySwallowingActive={!!stealthStatus?.swallowing_active}
                  isMac={runtimePlatform === 'macos'}
                  onRequestAccessibility={handleRequestAccessibility}
                  onRefreshStealthStatus={handleRefreshStealthStatus}
                  setupChecklist={setupChecklistProps}
                />
              )}
            </div>
          </div>
        </div>
      )}

      {/* Audio Prompt Warning Dialog */}
      {showDocumentManager && (
        <DocumentManager
          onClose={() => setShowDocumentManager(false)}
          existingPlaceholders={documentPlaceholders}
          onSaved={(saved) => {
            setDocumentPlaceholders(saved);
            const forPrompts: StoredDocPlaceholder[] = saved.map(p => ({
              name: p.name,
              extractedText: p.extracted_text,
            }));
            localStorage.setItem('document_placeholders', JSON.stringify(forPrompts));
          }}
        />
      )}

      {showAudioPromptWarning && (
        <div className="dialog-overlay" onClick={() => setShowAudioPromptWarning(false)}>
          <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
            <h3>⚠️ Audio Prompt Selected</h3>
            <p>
              You have selected a non-audio input source, but the "Verbal Interview (Audio)" prompt is currently active.
            </p>
            <p>
              Please change your prompt in the <strong>Settings → Prompts</strong> tab to match your input source.
            </p>
            <div className="dialog-actions">
              <button onClick={() => setShowAudioPromptWarning(false)} className="dialog-btn confirm">
                OK
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
