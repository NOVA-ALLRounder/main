import { useState } from "react";
import { Bot, Play, Pause, Activity, Eye, Brain, Hammer, MessageSquare } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import { motion } from "framer-motion";

interface AgentStatus {
    state: "idle" | "observing" | "reasoning" | "planning" | "executing" | "reflecting";
    currentTask: string | null;
    observationsCount: number;
    plansCompleted: number;
    isRunning: boolean;
}

const stateIcons = {
    idle: Activity,
    observing: Eye,
    reasoning: Brain,
    planning: MessageSquare,
    executing: Hammer,
    reflecting: Brain,
};

const stateColors = {
    idle: "text-gray-400",
    observing: "text-blue-500",
    reasoning: "text-purple-500",
    planning: "text-yellow-500",
    executing: "text-green-500",
    reflecting: "text-indigo-500",
};

const stateLabels = {
    idle: "대기 중",
    observing: "관찰 중",
    reasoning: "사고 중",
    planning: "계획 중",
    executing: "실행 중",
    reflecting: "반성 중",
};

const reactLoopStates = ["observing", "reasoning", "planning", "executing", "reflecting"] as const;

export default function AutonomousAgent() {
    const [status, setStatus] = useState<AgentStatus>({
        state: "idle",
        currentTask: null,
        observationsCount: 0,
        plansCompleted: 0,
        isRunning: false,
    });

    const StateIcon = stateIcons[status.state];

    const toggleAgent = () => {
        setStatus(prev => ({
            ...prev,
            isRunning: !prev.isRunning,
            state: !prev.isRunning ? "observing" : "idle",
        }));
    };

    return (
        <div className="space-y-4">
            {/* Agent Status Card */}
            <Card className="border-primary/20 bg-black/40 backdrop-blur-xl">
                <CardHeader className="pb-3">
                    <div className="flex items-center justify-between">
                        <CardTitle className="text-lg flex items-center gap-2">
                            <Bot className="w-5 h-5 text-primary" />
                            자율 AI 어시스턴트
                        </CardTitle>
                        <motion.button
                            onClick={toggleAgent}
                            className={cn(
                                "px-4 py-2 rounded-lg flex items-center gap-2 text-sm font-medium transition-colors",
                                status.isRunning
                                    ? "bg-red-500/20 text-red-400 hover:bg-red-500/30"
                                    : "bg-primary/20 text-primary hover:bg-primary/30"
                            )}
                            whileHover={{ scale: 1.05 }}
                            whileTap={{ scale: 0.95 }}
                        >
                            {status.isRunning ? (
                                <>
                                    <Pause className="w-4 h-4" />
                                    일시정지
                                </>
                            ) : (
                                <>
                                    <Play className="w-4 h-4" />
                                    시작
                                </>
                            )}
                        </motion.button>
                    </div>
                </CardHeader>
                <CardContent className="space-y-4">
                    {/* Current State */}
                    <div className="flex items-center gap-3 p-4 bg-white/5 rounded-xl border border-white/10">
                        <div className={cn("p-2 rounded-lg bg-white/10", stateColors[status.state])}>
                            <StateIcon className="w-5 h-5" />
                        </div>
                        <div className="flex-1">
                            <div className="text-sm font-medium">{stateLabels[status.state]}</div>
                            {status.currentTask && (
                                <div className="text-xs text-muted-foreground mt-1">{status.currentTask}</div>
                            )}
                        </div>
                        {status.isRunning && (
                            <div className="flex gap-1">
                                <span className="w-2 h-2 bg-primary rounded-full animate-pulse" />
                                <span className="w-2 h-2 bg-primary rounded-full animate-pulse [animation-delay:0.2s]" />
                                <span className="w-2 h-2 bg-primary rounded-full animate-pulse [animation-delay:0.4s]" />
                            </div>
                        )}
                    </div>

                    {/* Stats */}
                    <div className="grid grid-cols-2 gap-3">
                        <div className="p-3 bg-white/5 rounded-lg border border-white/10">
                            <div className="text-2xl font-bold text-primary">{status.observationsCount}</div>
                            <div className="text-xs text-muted-foreground">관찰한 이벤트</div>
                        </div>
                        <div className="p-3 bg-white/5 rounded-lg border border-white/10">
                            <div className="text-2xl font-bold text-green-500">{status.plansCompleted}</div>
                            <div className="text-xs text-muted-foreground">완료한 작업</div>
                        </div>
                    </div>

                    {/* Info */}
                    <div className="p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                        <div className="text-xs text-blue-400">
                            💡 <strong>자율 모드:</strong> AI가 이메일, 파일, 시스템 이벤트를 관찰하고 스스로 판단하여 작업을 수행합니다.
                        </div>
                    </div>
                </CardContent>
            </Card>

            {/* ReAct Loop Visualization */}
            <Card className="border-primary/20 bg-black/40 backdrop-blur-xl">
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm">ReAct 루프</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="space-y-2">
                        {reactLoopStates.map((state, idx) => {
                            const Icon = stateIcons[state];
                            const isActive = status.state === state && status.isRunning;
                            const currentIdx = reactLoopStates.indexOf(status.state as typeof reactLoopStates[number]);
                            const isPast = currentIdx >= 0 && idx < currentIdx;

                            return (
                                <div
                                    key={state}
                                    className={cn(
                                        "flex items-center gap-3 p-2 rounded-lg transition-colors",
                                        isActive && "bg-white/10 border border-primary/30"
                                    )}
                                >
                                    <div className={cn(
                                        "w-8 h-8 rounded-full flex items-center justify-center",
                                        isActive ? "bg-primary/20" : isPast ? "bg-white/10" : "bg-white/5"
                                    )}>
                                        <Icon className={cn(
                                            "w-4 h-4",
                                            isActive ? stateColors[state] : isPast ? "text-white/50" : "text-white/30"
                                        )} />
                                    </div>
                                    <span className={cn(
                                        "text-sm",
                                        isActive ? "text-white font-medium" : isPast ? "text-white/50" : "text-white/30"
                                    )}>
                                        {stateLabels[state]}
                                    </span>
                                </div>
                            );
                        })}
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
