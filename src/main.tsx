import { Component, StrictMode, type ReactNode } from 'react'
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
// React render errors surface through console.error, not window.onerror, and
// otherwise silently unmount the root leaving a white window. Forward them.
const originalConsoleError = console.error.bind(console)
console.error = (...args: unknown[]) => {
  originalConsoleError(...args)
  const text = args
    .map((a) => {
      try {
        if (a instanceof Error) return a.stack || a.message
        return typeof a === 'string' ? a : JSON.stringify(a)
      } catch {
        return String(a)
      }
    })
    .join(' ')
  reportDiag(`console.error: ${text.slice(0, 450)}`)
}
reportDiag('main.tsx evaluated: JS bundle loaded and executing')

// [webview-diag] 3s heartbeat: if the renderer/JS dies (e.g. WebView2 child
// processes killed externally), the Rust self-healer logs the exact moment
// the beats stop instead of the window silently turning white.
setInterval(() => {
  invoke('frontend_heartbeat').catch(() => {})
}, 3000)

// [webview-diag] Last-resort boundary: report the render crash and show the
// message in the window instead of leaving a blank white screen.
class DiagErrorBoundary extends Component<{ children: ReactNode }, { error: string | null }> {
  state: { error: string | null } = { error: null }

  static getDerivedStateFromError(error: Error) {
    return { error: error.message }
  }

  componentDidCatch(error: Error, info: { componentStack?: string | null }) {
    reportDiag(
      `React render crash: ${error.stack || error.message} | componentStack: ${(info.componentStack || '').slice(0, 250)}`,
    )
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 16, fontFamily: 'monospace', fontSize: 12, color: '#b00020' }}>
          UI crashed: {this.state.error}
        </div>
      )
    }
    return this.props.children
  }
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <DiagErrorBoundary>
      <App />
    </DiagErrorBoundary>
  </StrictMode>,
)
