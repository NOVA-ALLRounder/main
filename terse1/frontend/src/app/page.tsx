"use client";

import { useState } from "react";

interface Method {
  id: number;
  title: string;
  type: string;
  description: string;
  pros: string[];
  cons: string[];
}

interface ResearchResult {
  session_id: string;
  status: string;
  intent: string | null;
  domain: string | null;
  novelty_score: number | null;
  proposed_methods: Method[] | null;
  final_report: string | null;
  literature_count: number | null;
}

const API_BASE = "http://localhost:8000";

export default function Home() {
  const [input, setInput] = useState("");
  const [domain, setDomain] = useState("general");
  const [loading, setLoading] = useState(false);
  const [phase, setPhase] = useState<"input" | "methods" | "result">("input");
  const [result, setResult] = useState<ResearchResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const domains = [
    { value: "general", label: "일반" },
    { value: "physics", label: "물리학" },
    { value: "biology", label: "생물학" },
    { value: "chemistry", label: "화학" },
    { value: "cs", label: "컴퓨터과학" },
    { value: "medicine", label: "의학" },
    { value: "data_science", label: "데이터과학" },
  ];

  const startResearch = async () => {
    if (!input.trim()) return;

    setLoading(true);
    setError(null);

    try {
      const res = await fetch(`${API_BASE}/api/research/start`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ user_input: input, domain }),
      });

      if (!res.ok) throw new Error("API 요청 실패");

      const data: ResearchResult = await res.json();
      setResult(data);

      if (data.status === "waiting_selection" && data.proposed_methods) {
        setPhase("methods");
      } else {
        setPhase("result");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "알 수 없는 오류");
    } finally {
      setLoading(false);
    }
  };

  const selectMethod = async (methodId: number) => {
    if (!result?.session_id) return;

    setLoading(true);
    setError(null);

    try {
      const res = await fetch(`${API_BASE}/api/research/select-method`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          session_id: result.session_id,
          selected_method: methodId,
        }),
      });

      if (!res.ok) throw new Error("방법론 선택 실패");

      const data: ResearchResult = await res.json();
      setResult(data);
      setPhase("result");
    } catch (err) {
      setError(err instanceof Error ? err.message : "알 수 없는 오류");
    } finally {
      setLoading(false);
    }
  };

  const resetAll = () => {
    setInput("");
    setPhase("input");
    setResult(null);
    setError(null);
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-slate-900 via-purple-900 to-slate-900">
      {/* Header */}
      <header className="border-b border-white/10 bg-black/20 backdrop-blur-xl">
        <div className="max-w-6xl mx-auto px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <span className="text-3xl">🔬</span>
            <h1 className="text-xl font-bold text-white">Virtual Science Lab</h1>
          </div>
          <span className="text-sm text-purple-300">자율 과학 발견 에이전트</span>
        </div>
      </header>

      <main className="max-w-4xl mx-auto px-6 py-12">
        {/* Phase 1: Input */}
        {phase === "input" && (
          <div className="space-y-8">
            <div className="text-center space-y-4">
              <h2 className="text-4xl font-bold text-white">
                가설 또는 질문을 입력하세요
              </h2>
              <p className="text-lg text-purple-200">
                AI가 문헌을 분석하고, 실험 방법을 설계하며, 보고서를 작성합니다
              </p>
            </div>

            <div className="bg-white/5 backdrop-blur-lg rounded-2xl p-8 border border-white/10 space-y-6">
              <div>
                <label className="block text-sm font-medium text-purple-200 mb-2">
                  연구 분야
                </label>
                <select
                  value={domain}
                  onChange={(e) => setDomain(e.target.value)}
                  className="w-full bg-white/10 border border-white/20 rounded-lg px-4 py-3 text-white focus:outline-none focus:ring-2 focus:ring-purple-500"
                >
                  {domains.map((d) => (
                    <option key={d.value} value={d.value} className="bg-slate-800">
                      {d.label}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-sm font-medium text-purple-200 mb-2">
                  가설 또는 질문
                </label>
                <textarea
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  placeholder="예: 특정 단백질 구조가 바이러스 복제를 억제할 것이다"
                  rows={4}
                  className="w-full bg-white/10 border border-white/20 rounded-lg px-4 py-3 text-white placeholder-white/40 focus:outline-none focus:ring-2 focus:ring-purple-500 resize-none"
                />
              </div>

              <button
                onClick={startResearch}
                disabled={loading || !input.trim()}
                className="w-full bg-gradient-to-r from-purple-600 to-pink-600 text-white font-semibold py-4 rounded-lg hover:from-purple-500 hover:to-pink-500 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
              >
                {loading ? (
                  <>
                    <span className="animate-spin">⏳</span>
                    분석 중...
                  </>
                ) : (
                  <>
                    🚀 연구 시작
                  </>
                )}
              </button>
            </div>
          </div>
        )}

        {/* Phase 2: Method Selection */}
        {phase === "methods" && result?.proposed_methods && (
          <div className="space-y-8">
            <div className="text-center space-y-4">
              <h2 className="text-3xl font-bold text-white">🧪 방법론을 선택하세요</h2>
              <div className="flex items-center justify-center gap-6 text-sm">
                <span className="text-purple-300">
                  📊 독창성: {((result.novelty_score || 0) * 100).toFixed(0)}%
                </span>
                <span className="text-purple-300">
                  📚 관련 논문: {result.literature_count}개
                </span>
              </div>
            </div>

            <div className="grid gap-4">
              {result.proposed_methods.map((method) => (
                <div
                  key={method.id}
                  onClick={() => !loading && selectMethod(method.id - 1)}
                  className="bg-white/5 backdrop-blur-lg rounded-xl p-6 border border-white/10 hover:border-purple-500/50 hover:bg-white/10 cursor-pointer transition-all group"
                >
                  <div className="flex items-start justify-between">
                    <div className="space-y-2">
                      <div className="flex items-center gap-3">
                        <span className="text-2xl">
                          {method.type === "analytical" ? "📈" : method.type === "simulation" ? "🔬" : "🤖"}
                        </span>
                        <h3 className="text-xl font-semibold text-white group-hover:text-purple-300 transition-colors">
                          {method.title}
                        </h3>
                      </div>
                      <p className="text-purple-200/80">{method.description}</p>
                      <div className="flex gap-4 text-sm">
                        <span className="text-green-400">✓ {method.pros?.join(", ")}</span>
                        <span className="text-orange-400">⚠ {method.cons?.join(", ")}</span>
                      </div>
                    </div>
                    <span className="text-purple-400 opacity-0 group-hover:opacity-100 transition-opacity">
                      선택 →
                    </span>
                  </div>
                </div>
              ))}
            </div>

            {loading && (
              <div className="text-center text-purple-300 animate-pulse">
                ⏳ 가상 실험 실행 중...
              </div>
            )}
          </div>
        )}

        {/* Phase 3: Results */}
        {phase === "result" && result && (
          <div className="space-y-8">
            <div className="flex items-center justify-between">
              <h2 className="text-3xl font-bold text-white">📄 연구 보고서</h2>
              <button
                onClick={resetAll}
                className="text-purple-300 hover:text-white transition-colors"
              >
                ← 새 연구 시작
              </button>
            </div>

            <div className="bg-white/5 backdrop-blur-lg rounded-2xl p-8 border border-white/10">
              <div className="prose prose-invert max-w-none">
                <pre className="whitespace-pre-wrap text-purple-100 font-sans">
                  {result.final_report || "보고서 생성 중..."}
                </pre>
              </div>
            </div>
          </div>
        )}

        {/* Error Display */}
        {error && (
          <div className="mt-6 bg-red-500/20 border border-red-500/50 rounded-lg p-4 text-red-200">
            ⚠️ {error}
          </div>
        )}
      </main>

      {/* Footer */}
      <footer className="border-t border-white/10 mt-12">
        <div className="max-w-6xl mx-auto px-6 py-4 text-center text-sm text-purple-300/60">
          Powered by LangGraph + DAACS v2 CLI
        </div>
      </footer>
    </div>
  );
}
