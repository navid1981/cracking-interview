import { invoke } from '@tauri-apps/api/core';
import { PromptTemplate } from './prompts';
import type { CustomPrompt } from './prompts';

/**
 * Cross-process prompt synchronization.
 *
 * Prompt selection and custom prompt definitions live in WebView
 * `localStorage`, which is scoped to the WebView2 profile. The desktop
 * worker runs an isolated profile (`WebView2_DeskWorker`), so it cannot
 * see the main GUI's prompts. The Rust backend owns a shared
 * `prompt-state.json` in the app config dir; this service keeps the local
 * `localStorage` view and the shared file in sync in both directions:
 *
 * - local edits (selection, custom prompt CRUD, built-in overrides) are
 *   detected by polling the serialized local bundle and pushed;
 * - changes made by the other process are detected via the backend
 *   revision counter, applied to `localStorage`, and reported through
 *   `onRemoteChange` so React state can refresh.
 *
 * Conflict model: last-writer-wins, coordinated by the backend revision.
 */

export interface BuiltinPromptOverride {
  system: string | null;
  user: string | null;
}

export interface PromptBundle {
  selectedTemplateId: string | null;
  customPrompts: CustomPrompt[];
  builtinOverrides: Record<string, BuiltinPromptOverride>;
}

interface SharedPromptState extends PromptBundle {
  schemaVersion: number;
  revision: number;
  updatedAtUnixMs: number;
}

const SELECTED_KEY = 'prompt_template';
const CUSTOM_PROMPTS_KEY = 'custom_prompts';

function collectLocalPromptBundle(): PromptBundle {
  let customPrompts: CustomPrompt[] = [];
  try {
    const parsed = JSON.parse(localStorage.getItem(CUSTOM_PROMPTS_KEY) || '[]');
    if (Array.isArray(parsed)) {
      customPrompts = parsed;
    }
  } catch {
    customPrompts = [];
  }

  const builtinOverrides: Record<string, BuiltinPromptOverride> = {};
  for (const id of Object.values(PromptTemplate)) {
    const system = localStorage.getItem(`custom_${id}_system`);
    const user = localStorage.getItem(`custom_${id}_user`);
    if (system !== null || user !== null) {
      builtinOverrides[id] = { system, user };
    }
  }

  return {
    selectedTemplateId: localStorage.getItem(SELECTED_KEY),
    customPrompts,
    builtinOverrides,
  };
}

function applySharedPromptBundle(bundle: PromptBundle): void {
  localStorage.setItem(CUSTOM_PROMPTS_KEY, JSON.stringify(bundle.customPrompts ?? []));
  if (bundle.selectedTemplateId) {
    localStorage.setItem(SELECTED_KEY, bundle.selectedTemplateId);
  } else {
    localStorage.removeItem(SELECTED_KEY);
  }
  for (const id of Object.values(PromptTemplate)) {
    const override = bundle.builtinOverrides?.[id];
    if (override?.system != null) {
      localStorage.setItem(`custom_${id}_system`, override.system);
    } else {
      localStorage.removeItem(`custom_${id}_system`);
    }
    if (override?.user != null) {
      localStorage.setItem(`custom_${id}_user`, override.user);
    } else {
      localStorage.removeItem(`custom_${id}_user`);
    }
  }
}

function isEmptyBundle(bundle: PromptBundle): boolean {
  return (
    !bundle.selectedTemplateId &&
    bundle.customPrompts.length === 0 &&
    Object.keys(bundle.builtinOverrides).length === 0
  );
}

function bundleOf(state: SharedPromptState): PromptBundle {
  return {
    selectedTemplateId: state.selectedTemplateId ?? null,
    customPrompts: state.customPrompts ?? [],
    builtinOverrides: state.builtinOverrides ?? {},
  };
}

export function startPromptStateSync(
  handlers: { onRemoteChange?: (bundle: PromptBundle) => void } = {},
): () => void {
  let stopped = false;
  let initialized = false;
  let applyingRemote = false;
  let knownRevision = 0;
  let lastLocalKey = '';

  const keyOf = (bundle: PromptBundle) => JSON.stringify(bundle);

  async function push(bundle: PromptBundle): Promise<void> {
    const saved = await invoke<SharedPromptState>('save_prompt_state', {
      snapshot: {
        schemaVersion: 1,
        revision: 0,
        updatedAtUnixMs: Date.now(),
        selectedTemplateId: bundle.selectedTemplateId,
        customPrompts: bundle.customPrompts,
        builtinOverrides: bundle.builtinOverrides,
      },
    });
    knownRevision = saved.revision;
  }

  async function init(): Promise<void> {
    try {
      const remote = await invoke<SharedPromptState | null>('get_prompt_state');
      if (remote) {
        applyingRemote = true;
        applySharedPromptBundle(bundleOf(remote));
        applyingRemote = false;
        knownRevision = remote.revision;
      } else {
        // No shared state yet: seed it from this profile, but only if this
        // profile actually has prompt data — an empty desktop-worker
        // profile must not overwrite the main GUI's prompts with nothing.
        const local = collectLocalPromptBundle();
        if (!isEmptyBundle(local)) {
          await push(local);
        }
      }
    } catch {
      // Backend unavailable (e.g. plain browser dev): localStorage-only mode.
    }
    lastLocalKey = keyOf(collectLocalPromptBundle());
    initialized = true;
  }

  async function tick(): Promise<void> {
    if (stopped || !initialized || applyingRemote) {
      return;
    }
    try {
      const local = collectLocalPromptBundle();
      if (keyOf(local) !== lastLocalKey) {
        lastLocalKey = keyOf(local);
        await push(local);
        return;
      }
      const remote = await invoke<SharedPromptState | null>('get_prompt_state');
      if (remote && remote.revision > knownRevision) {
        knownRevision = remote.revision;
        applyingRemote = true;
        const bundle = bundleOf(remote);
        applySharedPromptBundle(bundle);
        applyingRemote = false;
        lastLocalKey = keyOf(collectLocalPromptBundle());
        handlers.onRemoteChange?.(bundle);
      }
    } catch {
      // Transient IPC errors are fine; the next tick retries.
    }
  }

  void init();
  const timer = setInterval(() => {
    void tick();
  }, 1000);

  return () => {
    stopped = true;
    clearInterval(timer);
  };
}
