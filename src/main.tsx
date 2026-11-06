import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { invoke } from '@tauri-apps/api/core'
import App from './App.tsx'
import './App.css'

// [webview-diag] Boot/error probes -> resilience.log (see main.rs report_frontend_diagnostic)
const reportDiag = (message: string) => {
  invoke('report_frontend_diagnostic', { message }).catch(() => {})
}
window.addEventListener('error', (e) => reportDiag(`window.onerror: ${e.message}`))
window.addEventListener('unhandledrejection', (e) =>
  reportDiag(`unhandledrejection: ${String((e as PromiseRejectionEvent).reason)}`),
)
reportDiag('main.tsx evaluated: JS bundle loaded and executing')

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
