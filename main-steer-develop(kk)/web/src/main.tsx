import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.tsx'
import './index.css'

const showBootError = (label: string, error: unknown) => {
    const root = document.getElementById('root')
    const pre = document.createElement('pre')
    pre.style.whiteSpace = 'pre-wrap'
    pre.style.padding = '16px'
    pre.style.margin = '12px'
    pre.style.border = '1px solid #7f1d1d'
    pre.style.background = '#111827'
    pre.style.color = '#fca5a5'
    pre.style.fontFamily = 'Consolas, monospace'
    pre.textContent = `[${label}] ${String(error)}`
    if (root) {
        root.replaceChildren(pre)
    } else {
        document.body.appendChild(pre)
    }
}

window.addEventListener('error', (event) => {
    showBootError('window.error', event.error ?? event.message)
})

window.addEventListener('unhandledrejection', (event) => {
    showBootError('unhandledrejection', event.reason)
})

type RootBoundaryProps = {
    children: React.ReactNode
}

type RootBoundaryState = {
    error: unknown
}

class RootBoundary extends React.Component<RootBoundaryProps, RootBoundaryState> {
    state: RootBoundaryState = { error: null }

    static getDerivedStateFromError(error: unknown): RootBoundaryState {
        return { error }
    }

    componentDidCatch(error: unknown) {
        showBootError('react.boundary', error)
    }

    render() {
        if (this.state.error) {
            return (
                <pre
                    style={{
                        whiteSpace: 'pre-wrap',
                        padding: '16px',
                        margin: '12px',
                        border: '1px solid #7f1d1d',
                        background: '#111827',
                        color: '#fca5a5',
                        fontFamily: 'Consolas, monospace',
                    }}
                >
                    {`[react.boundary] ${String(this.state.error)}`}
                </pre>
            )
        }
        return this.props.children
    }
}

try {
    ReactDOM.createRoot(document.getElementById('root')!).render(
        <React.StrictMode>
            <RootBoundary>
                <App />
            </RootBoundary>
        </React.StrictMode>,
    )
} catch (error) {
    showBootError('bootstrap', error)
}
