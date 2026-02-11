import { useState } from "react";
import { Eye, Send, Monitor, MousePointer } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { motion } from "framer-motion";
import { API_BASE_URL } from "@/lib/api";

export default function ComputerUse() {
    const [goal, setGoal] = useState("");
    const [isRunning, setIsRunning] = useState(false);
    const [logs, setLogs] = useState<string[]>([]);

    const executeGoal = async () => {
        if (!goal.trim()) return;

        setIsRunning(true);
        setLogs([`🎯 Goal: ${goal}`, "📸 Starting computer use agent..."]);

        try {
            const response = await fetch(`${API_BASE_URL}/computer-use/execute`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ goal }),
            });

            const data = await response.json();

            if (data.success) {
                setLogs(prev => [...prev, "✅ Agent started! Watch your screen."]);
            } else {
                setLogs(prev => [...prev, `❌ ${data.message}`]);
            }
        } catch (error) {
            setLogs(prev => [...prev, `❌ Error: ${error instanceof Error ? error.message : "Unknown error"}`]);
        } finally {
            setIsRunning(false);
        }
    };

    return (
        <div className="space-y-4">
            {/* Computer Use Card */}
            <Card className="border-primary/20 bg-black/40 backdrop-blur-xl">
                <CardHeader className="pb-3">
                    <CardTitle className="text-lg flex items-center gap-2">
                        <Monitor className="w-5 h-5 text-primary" />
                        Computer Use - AI가 화면을 보고 조작합니다
                    </CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                    {/* Goal Input */}
                    <div className="space-y-2">
                        <label className="text-sm text-muted-foreground">What should the AI do?</label>
                        <div className="relative">
                            <input
                                value={goal}
                                onChange={(e) => setGoal(e.target.value)}
                                onKeyDown={(e) => e.key === "Enter" && !isRunning && executeGoal()}
                                placeholder="예: 메모장을 열고 'Hello World'를 입력해줘"
                                className="w-full bg-black/20 border border-white/10 rounded-xl px-4 py-3 pr-12 text-sm focus:outline-none focus:border-primary/50 transition-colors"
                                disabled={isRunning}
                            />
                            <motion.button
                                onClick={executeGoal}
                                disabled={!goal.trim() || isRunning}
                                className="absolute right-2 top-2 bottom-2 aspect-square rounded-lg bg-gradient-to-br from-primary to-primary/80 text-primary-foreground flex items-center justify-center hover:opacity-90 disabled:opacity-30 transition-all"
                                whileHover={{ scale: 1.05 }}
                                whileTap={{ scale: 0.95 }}
                            >
                                <Send className="w-4 h-4" />
                            </motion.button>
                        </div>
                    </div>

                    {/* How it works */}
                    <div className="grid grid-cols-3 gap-3">
                        <div className="p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                            <div className="flex items-center gap-2 text-blue-400 mb-1">
                                <Eye className="w-4 h-4" />
                                <span className="text-xs font-medium">1. 화면 캡처</span>
                            </div>
                            <p className="text-xs text-blue-400/70">스크린샷 촬영</p>
                        </div>
                        <div className="p-3 bg-purple-500/10 border border-purple-500/20 rounded-lg">
                            <div className="flex items-center gap-2 text-purple-400 mb-1">
                                <span className="text-xs font-medium">2. AI 분석</span>
                            </div>
                            <p className="text-xs text-purple-400/70">GPT-4V로 이해</p>
                        </div>
                        <div className="p-3 bg-green-500/10 border border-green-500/20 rounded-lg">
                            <div className="flex items-center gap-2 text-green-400 mb-1">
                                <MousePointer className="w-4 h-4" />
                                <span className="text-xs font-medium">3. 자동 조작</span>
                            </div>
                            <p className="text-xs text-green-400/70">마우스/키보드</p>
                        </div>
                    </div>

                    {/* Examples */}
                    <div className="space-y-2">
                        <div className="text-xs text-muted-foreground">예시:</div>
                        <div className="space-y-1">
                            {[
                                "메모장을 열고 'Hello World'를 입력해줘",
                                "크롬을 열고 google.com으로 가줘",
                                "계산기를 열고 123 + 456을 계산해줘",
                            ].map((example, idx) => (
                                <button
                                    key={idx}
                                    onClick={() => setGoal(example)}
                                    disabled={isRunning}
                                    className="w-full text-left px-3 py-2 text-xs bg-white/5 hover:bg-white/10 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                                >
                                    {example}
                                </button>
                            ))}
                        </div>
                    </div>
                </CardContent>
            </Card>

            {/* Logs */}
            {logs.length > 0 && (
                <Card className="border-primary/20 bg-black/40 backdrop-blur-xl">
                    <CardHeader className="pb-3">
                        <CardTitle className="text-sm">Execution Log</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="space-y-1 font-mono text-xs max-h-60 overflow-y-auto">
                            {logs.map((log, idx) => (
                                <div key={idx} className="text-muted-foreground">
                                    {log}
                                </div>
                            ))}
                            {isRunning && (
                                <div className="flex items-center gap-2 text-primary">
                                    <span className="w-1.5 h-1.5 bg-primary rounded-full animate-pulse" />
                                    <span className="w-1.5 h-1.5 bg-primary rounded-full animate-pulse [animation-delay:0.2s]" />
                                    <span className="w-1.5 h-1.5 bg-primary rounded-full animate-pulse [animation-delay:0.4s]" />
                                    <span>실행 중...</span>
                                </div>
                            )}
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* Warning */}
            <Card className="border-yellow-500/20 bg-yellow-500/5 backdrop-blur-xl">
                <CardContent className="p-4">
                    <div className="text-xs text-yellow-400">
                        ⚠️ <strong>주의:</strong> AI가 실제로 마우스와 키보드를 제어합니다.
                        실행 중에는 컴퓨터 사용을 피해주세요.
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
