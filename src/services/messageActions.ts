// Maps status/error messages to the next step the user can take, shown as buttons under the message.

export type MessageActionKind =
  | 'add_api_key'
  | 'fix_api_key'
  | 'upgrade'
  | 'view_usage'
  | 'open_chrome'
  | 'download_chrome'
  | 'sign_in'
  | 'screen_permission';

export const MESSAGE_ACTION_LABELS: Record<MessageActionKind, string> = {
  add_api_key: 'Add your API key',
  fix_api_key: 'Update API key',
  upgrade: 'Upgrade to Pro',
  view_usage: 'View usage',
  open_chrome: 'Open Chrome',
  download_chrome: 'Download Chrome',
  sign_in: 'Sign in again',
  screen_permission: 'Open Screen Recording settings',
};

interface Context {
  isPro: boolean;
  isMac: boolean;
  cdpReady: boolean;
}

const RULES: Array<{ test: RegExp; actions: (ctx: Context) => MessageActionKind[] }> = [
  { test: /free trial expired/i, actions: () => ['add_api_key', 'upgrade'] },
  { test: /requires pro subscription|free tier only works on/i, actions: () => ['upgrade'] },
  { test: /invalid api key|api key not valid|api_key_invalid/i, actions: () => ['fix_api_key'] },
  { test: /monthly (quota|audio limit)/i, actions: () => ['view_usage'] },
  { test: /session error|sign in again|please sign in/i, actions: () => ['sign_in'] },
  { test: /chrome not found/i, actions: () => ['download_chrome'] },
  {
    test: /no input source|no page targets found in chrome|ws connection closed|chrome.*not (connected|running)/i,
    actions: ({ cdpReady }) => (cdpReady ? [] : ['open_chrome']),
  },
  { test: /screen recording/i, actions: ({ isMac }) => (isMac ? ['screen_permission'] : []) },
];

export function getMessageActions(message: string, ctx: Context): MessageActionKind[] {
  if (!message.startsWith('❌') && !message.startsWith('⚠️')) return [];
  const rule = RULES.find(r => r.test.test(message));
  const actions = rule ? rule.actions(ctx) : [];
  return ctx.isPro ? actions.filter(a => a !== 'upgrade' && a !== 'add_api_key') : actions;
}
