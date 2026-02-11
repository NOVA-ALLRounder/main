import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import Launcher from '@/features/launcher/Launcher'
import WidgetLayer from '@/features/widget/WidgetLayer' // Import new layer
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEffect } from 'react'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
})

function App() {
  type WindowWithTauriMeta = Window & {
    __TAURI_METADATA__?: unknown
    __TAURI__?: { metadata?: unknown }
    __TAURI_INTERNALS__?: { metadata?: unknown }
  }

  const isWidget = (() => {
    if (typeof window === 'undefined') {
      return false
    }

    const tauriMeta =
      (window as WindowWithTauriMeta).__TAURI_METADATA__ ||
      (window as WindowWithTauriMeta).__TAURI__?.metadata ||
      (window as WindowWithTauriMeta).__TAURI_INTERNALS__?.metadata

    if (!tauriMeta) {
      return false
    }

    try {
      return getCurrentWindow().label === 'widget'
    } catch {
      return false
    }
  })()

  useEffect(() => {
    // Check if this is the 'widget' window
    if (isWidget) {
      document.body.style.backgroundColor = 'transparent'; // Ensure transparency
    }
  }, [isWidget]);

  return (
    <QueryClientProvider client={queryClient}>
      {isWidget ? (
        <WidgetLayer />
      ) : (
        <div className="h-screen w-screen bg-transparent overflow-hidden relative flex items-center justify-center">
          <Launcher />
        </div>
      )}
    </QueryClientProvider>
  )
}

export default App
