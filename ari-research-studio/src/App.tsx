import React, { useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { 
  Plus, 
  Search, 
  Database, 
  BookOpen, 
  Sparkles, 
  Wand2, 
  FlaskConical, 
  BarChart3, 
  Settings,
  Bell,
  User,
  ChevronRight,
  Monitor
} from 'lucide-react'
import { Button } from './components/ui-system/Button'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from './components/ui-system/Card'
import { Badge } from './components/ui-system/Badge'
import { Input } from './components/ui-system/Input'

const menuItems = [
  { id: 'studio', label: 'Workflow Studio', icon: Monitor },
  { id: 'library', label: 'Research Library', icon: Database },
  { id: 'publication', label: 'Publication Center', icon: BookOpen },
]

function App() {
  const [activeTab, setActiveTab ] = useState('studio')

  return (
    <div className="flex h-screen bg-[#0c0c0e] text-white overflow-hidden">
      {/* Sidebar */}
      <aside className="w-64 border-r border-white/5 flex flex-col pt-6 pb-4">
        <div className="px-6 mb-10 flex items-center gap-2">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-primary-400 to-primary-600 flex items-center justify-center font-bold text-black text-sm">
            A
          </div>
          <span className="font-bold tracking-tight text-lg">ARI Studio</span>
        </div>

        <nav className="flex-1 space-y-1 px-3">
          {menuItems.map((item) => {
            const Icon = item.icon
            const isActive = activeTab === item.id
            return (
              <button
                key={item.id}
                onClick={() => setActiveTab(item.id)}
                className={`w-full flex items-center gap-3 px-4 py-3 rounded-xl text-sm font-medium transition-all ${
                  isActive 
                    ? 'bg-white/10 text-white shadow-lg' 
                    : 'text-white/40 hover:text-white hover:bg-white/5'
                }`}
              >
                <Icon className={`w-4 h-4 ${isActive ? 'text-primary-400' : ''}`} />
                {item.label}
              </button>
            )
          })}
        </nav>

        <div className="px-3 pt-6 border-t border-white/5 space-y-1">
          <button className="w-full flex items-center gap-3 px-4 py-3 rounded-xl text-sm font-medium text-white/40 hover:text-white hover:bg-white/5">
            <Settings className="w-4 h-4" />
            Settings
          </button>
        </div>
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col relative overflow-y-auto">
        {/* Header */}
        <header className="h-16 border-b border-white/5 flex items-center justify-between px-8 sticky top-0 bg-[#0c0c0e]/80 backdrop-blur-md z-10">
          <div className="flex items-center gap-2 text-sm text-white/40">
            <span>Project</span>
            <ChevronRight className="w-4 h-4" />
            <span className="text-white/90 font-medium">Untitled Research</span>
          </div>

          <div className="flex items-center gap-4">
            <div className="flex items-center gap-1">
              <Button variant="ghost" size="sm" className="w-9 h-9 p-0">
                <Search className="w-4 h-4" />
              </Button>
              <Button variant="ghost" size="sm" className="w-9 h-9 p-0">
                <Bell className="w-4 h-4" />
              </Button>
            </div>
            <div className="h-6 w-[1px] bg-white/10" />
            <Button variant="secondary" size="sm" className="gap-2">
              <User className="w-4 h-4" />
              Jiwon Han
            </Button>
          </div>
        </header>

        {/* Studio Workspace */}
        <div className="flex-1 p-8 max-w-7xl mx-auto w-full space-y-8">
          <section className="flex items-end justify-between">
            <div className="space-y-2">
              <Badge variant="default" className="text-[10px] tracking-widest uppercase py-0 px-2 font-bold bg-primary-500/5 text-primary-400 border-primary-500/20">
                Research Studio
              </Badge>
              <h1 className="text-4xl font-bold tracking-tight bg-gradient-to-r from-white to-white/50 bg-clip-text text-transparent">
                Launch New Experiment
              </h1>
              <p className="text-white/40 max-w-xl">
                Define your research seed and let the autonomous engine suggest the best methodological approach.
              </p>
            </div>
            <Button className="rounded-full px-8 py-6 text-base font-bold dark-glow hover:scale-[1.02] transition-transform">
              Start Research
            </Button>
          </section>

          {/* Seed Input Card */}
          <Card className="p-8 group relative overflow-hidden">
            <div className="absolute top-0 right-0 w-64 h-64 bg-primary-500/5 rounded-full blur-3xl -mr-32 -mt-32 pointer-events-none" />
            
            <div className="grid grid-cols-1 md:grid-cols-2 gap-12">
              <div className="space-y-6">
                <div>
                  <label className="text-xs font-bold text-white/20 uppercase tracking-wider mb-2 block">Research Domain</label>
                  <Input placeholder="e.g. Fintech, Cybersecurity, DevOps..." className="bg-white/5" />
                </div>
                <div>
                  <label className="text-xs font-bold text-white/20 uppercase tracking-wider mb-2 block">Seed Hypothesis / Question</label>
                  <textarea 
                    placeholder="Enter your research idea or question here..."
                    className="w-full min-h-[160px] bg-white/5 rounded-2xl border border-white/10 p-4 text-sm text-white focus:outline-none focus:ring-2 focus:ring-primary-500/50 transition-all resize-none"
                  />
                </div>
                <div className="flex gap-2">
                  <Badge variant="secondary">Directional</Badge>
                  <Badge variant="secondary">Comparative</Badge>
                  <Badge variant="secondary">Observational</Badge>
                </div>
              </div>

              <div className="bg-white/[0.02] border border-white/5 rounded-2xl p-6 flex flex-col">
                <div className="flex items-center gap-2 mb-4">
                  <Sparkles className="w-4 h-4 text-primary-400" />
                  <span className="text-sm font-semibold">Engine Suggestions</span>
                </div>
                <div className="flex-1 flex flex-col items-center justify-center text-center p-8 space-y-4">
                  <div className="w-12 h-12 rounded-xl bg-white/5 flex items-center justify-center">
                    <Wand2 className="w-6 h-6 text-white/20" />
                  </div>
                  <div className="space-y-1">
                    <p className="text-sm font-medium text-white/60">Ready to analyze</p>
                    <p className="text-xs text-white/30 px-6">Submit your seed to receive methodological candidates.</p>
                  </div>
                </div>
                <Button variant="secondary" className="w-full">
                  Improve Seed with AI
                </Button>
              </div>
            </div>
          </Card>

          {/* Workflow Steps Preview */}
          <section className="grid grid-cols-1 md:grid-cols-4 gap-4">
            {[
              { id: 'approach', title: 'Approach Recommendation', desc: 'Select optimized analysis methodology.', icon: Wand2 },
              { id: 'execution', title: 'Autonomous Execution', desc: 'Agent performs data processing.', icon: FlaskConical },
              { id: 'report', title: 'Insight Generation', desc: 'Automated synthesis & visualization.', icon: BarChart3 },
              { id: 'paper', title: 'Publication Output', desc: 'Generate IMRaD structured papers.', icon: BookOpen },
            ].map((step, idx) => {
              const Icon = step.icon
              return (
                <div key={step.id} className="p-6 rounded-2xl bg-white/[0.02] border border-white/5 hover:border-white/10 transition-colors">
                  <div className="w-10 h-10 rounded-xl bg-white/5 flex items-center justify-center mb-4">
                    <Icon className="w-5 h-5 text-white/40" />
                  </div>
                  <h4 className="text-sm font-semibold mb-1">{step.title}</h4>
                  <p className="text-xs text-white/30 leading-relaxed">{step.desc}</p>
                </div>
              )
            })}
          </section>
        </div>
      </main>
    </div>
  )
}

export default App
