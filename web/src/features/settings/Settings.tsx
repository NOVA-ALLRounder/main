import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Check, ShieldCheck, Database, Server, RefreshCw, Power } from "lucide-react";
import { motion } from "framer-motion";
import axios from "axios";
import { LaunchReadinessOpsCard } from "@/features/settings/components/LaunchReadinessOpsCard";
import { MemorySafetyCard } from "@/features/settings/components/MemorySafetyCard";
import {
    API_BASE_URL,
    deleteExecutionMemory,
    deleteRequestMemory,
    fetchLaunchEvalCandidates,
    fetchLaunchEvalCandidateSnapshotInfo,
    fetchLaunchOps,
    fetchMemoryRecords,
    fetchRecommendationReviewEvents,
    getHealth,
    restoreExecutionMemory,
    restoreRequestMemory,
    snapshotLaunchEvalCandidates,
    suppressExecutionMemory,
    suppressRequestMemory,
} from "@/lib/api";
import { useMutation, useQuery } from "@tanstack/react-query";
import type {
    ExecutionMemoryRecord,
    LaunchEvalCandidate,
    LaunchEvalCandidateSnapshotInfo,
    LaunchOpsResponse,
    RequestMemoryRecord,
} from "@/lib/types";

type LaunchEvalCandidateMode = "real" | "synthetic";

type SystemHealth = {
    missing_deps?: { name?: string; install_cmd?: string }[];
};

export default function Settings() {
    const [n8nRestarting, setN8nRestarting] = useState(false);
    const [manualStatus, setManualStatus] = useState<"error" | null>(null);
    const [telegramListenerBusy, setTelegramListenerBusy] = useState(false);
    const [telegramListenerStatus, setTelegramListenerStatus] = useState<string | null>(null);
    const [memoryStatus, setMemoryStatus] = useState<string | null>(null);
    const [launchEvalSnapshotStatus, setLaunchEvalSnapshotStatus] = useState<string | null>(null);
    const [launchEvalCandidateMode, setLaunchEvalCandidateMode] =
        useState<LaunchEvalCandidateMode>("real");
    const { data: healthData, isError: healthError, refetch: refetchHealth } = useQuery({
        queryKey: ["systemHealth"],
        queryFn: getHealth,
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
    });
    const {
        data: memoryData,
        isLoading: memoryLoading,
        refetch: refetchMemory,
    } = useQuery({
        queryKey: ["memoryRecords"],
        queryFn: () => fetchMemoryRecords(10, true),
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
    });
    const {
        data: launchOpsData,
        isLoading: launchOpsLoading,
        refetch: refetchLaunchOps,
    } = useQuery({
        queryKey: ["launchOps"],
        queryFn: () => fetchLaunchOps(200),
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
    });
    const { data: recommendationReviewEventsData } = useQuery({
        queryKey: ["recommendationReviewEvents"],
        queryFn: () => fetchRecommendationReviewEvents(10),
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
    });
    const {
        data: launchEvalCandidatesData,
        isLoading: launchEvalCandidatesLoading,
        refetch: refetchLaunchEvalCandidates,
    } = useQuery({
        queryKey: ["launchEvalCandidates", launchEvalCandidateMode],
        queryFn: () => fetchLaunchEvalCandidates(8, launchEvalCandidateMode),
        refetchInterval: 60000,
        refetchIntervalInBackground: false,
    });
    const {
        data: launchEvalSnapshotInfoData,
        refetch: refetchLaunchEvalSnapshotInfo,
    } = useQuery({
        queryKey: ["launchEvalCandidateSnapshotInfo", launchEvalCandidateMode],
        queryFn: () => fetchLaunchEvalCandidateSnapshotInfo(launchEvalCandidateMode),
        refetchInterval: 60000,
        refetchIntervalInBackground: false,
    });
    const health = healthData as SystemHealth | undefined;
    const launchOps = launchOpsData as LaunchOpsResponse | undefined;
    const launchEvalCandidates = launchEvalCandidatesData as LaunchEvalCandidate[] | undefined;
    const launchEvalSnapshotInfo = launchEvalSnapshotInfoData as LaunchEvalCandidateSnapshotInfo | undefined;
    const n8nMissing = Boolean(health?.missing_deps?.some((dep) => dep.name === "n8n"));
    const missingDeps = health?.missing_deps ?? [];
    const n8nStatus: "idle" | "success" | "error" | "failed" = manualStatus ?? (healthError || n8nMissing ? "failed" : "success");
    const memoryMetrics = memoryData?.metrics;
    const requestMemory = memoryData?.request_memory ?? [];
    const executionMemory = memoryData?.execution_memory ?? [];
    const recentMemoryAdminEvents = memoryData?.recent_admin_events ?? [];
    const recentRecommendationReviewEvents = recommendationReviewEventsData ?? [];
    const launchChatMetrics = launchOps?.chat_metrics;
    const launchNlRunMetrics = launchOps?.nl_run_metrics;
    const launchExecApprovalMetrics = launchOps?.exec_approval_metrics;
    const launchRecommendationMetrics = launchOps?.recommendation_metrics;
    const launchRecommendationReviewMetrics = launchOps?.recommendation_review_metrics;
    const launchRecentEvents = launchOps?.recent_events ?? [];
    const launchEvalCandidateList = launchEvalCandidates ?? [];
    const launchNlErrorRate = launchNlRunMetrics && launchNlRunMetrics.total > 0
        ? (launchNlRunMetrics.error / launchNlRunMetrics.total) * 100
        : 0;

    const checks = [
        { name: "Rust Core API", status: "Operational", icon: Server },
        { name: "SQLite Database", status: "Connected", icon: Database },
        { name: "n8n Integration", status: n8nStatus === "success" ? "Active" : "Failed", icon: ShieldCheck },
        { name: "System Monitor", status: "Running", icon: Check },
    ];

    const memoryActionMutation = useMutation({
        mutationFn: async (
            action:
                | { kind: "request"; op: "suppress" | "restore" | "delete"; record: RequestMemoryRecord }
                | { kind: "execution"; op: "suppress" | "restore" | "delete"; record: ExecutionMemoryRecord }
        ) => {
            if (action.kind === "request") {
                if (action.op === "suppress") {
                    return suppressRequestMemory(
                        action.record.original_request,
                        action.record.memory_scope,
                        "manual suppress from settings"
                    );
                }
                if (action.op === "restore") {
                    return restoreRequestMemory(action.record.original_request, action.record.memory_scope);
                }
                return deleteRequestMemory(action.record.original_request, action.record.memory_scope);
            }

            if (action.op === "suppress") {
                return suppressExecutionMemory(
                    action.record.intent_command,
                    action.record.params_key,
                    action.record.memory_scope,
                    "manual suppress from settings"
                );
            }
            if (action.op === "restore") {
                return restoreExecutionMemory(
                    action.record.intent_command,
                    action.record.params_key,
                    action.record.memory_scope
                );
            }
            return deleteExecutionMemory(
                action.record.intent_command,
                action.record.params_key,
                action.record.memory_scope
            );
        },
        onSuccess: async (data) => {
            setMemoryStatus(data.message);
            await refetchMemory();
        },
        onError: () => {
            setMemoryStatus("Memory action failed.");
        },
    });

    const handleN8nRestart = async () => {
        setN8nRestarting(true);
        setManualStatus(null);
        try {
            // Call backend to restart n8n
            await axios.post(`${API_BASE_URL}/chat`, {
                message: "n8n restart"
            });

            // Poll for health status
            setTimeout(async () => {
                await refetchHealth();
                setN8nRestarting(false);
            }, 3000); // Wait 3s for n8n to start

        } catch {
            setManualStatus("error");
            setN8nRestarting(false);
        }
    };

    const handleTelegramListenerStart = async () => {
        setTelegramListenerBusy(true);
        try {
            const { data } = await axios.post(`${API_BASE_URL}/chat`, {
                message: "telegram listener start",
            });
            setTelegramListenerStatus(data?.response ?? "Telegram listener start requested.");
        } catch {
            setTelegramListenerStatus("Telegram listener 시작 요청 실패");
        } finally {
            setTelegramListenerBusy(false);
        }
    };

    const handleTelegramListenerStatus = async () => {
        setTelegramListenerBusy(true);
        try {
            const { data } = await axios.post(`${API_BASE_URL}/chat`, {
                message: "telegram listener status",
            });
            setTelegramListenerStatus(data?.response ?? "상태 응답 없음");
        } catch {
            setTelegramListenerStatus("Telegram listener 상태 조회 실패");
        } finally {
            setTelegramListenerBusy(false);
        }
    };

    const handleRequestMemoryAction = (
        op: "suppress" | "restore" | "delete",
        record: RequestMemoryRecord
    ) => {
        setMemoryStatus(null);
        memoryActionMutation.mutate({ kind: "request", op, record });
    };

    const handleExecutionMemoryAction = (
        op: "suppress" | "restore" | "delete",
        record: ExecutionMemoryRecord
    ) => {
        setMemoryStatus(null);
        memoryActionMutation.mutate({ kind: "execution", op, record });
    };

    const handleLaunchEvalSnapshot = async () => {
        setLaunchEvalSnapshotStatus(null);
        try {
            const snapshot = await snapshotLaunchEvalCandidates(
                12,
                launchEvalCandidateMode
            );
            setLaunchEvalSnapshotStatus(
                `Snapshot saved (${snapshot.provenance_filter}): ${snapshot.output_path} (${snapshot.scenario_count} scenarios)`
            );
            await refetchLaunchEvalCandidates();
            await refetchLaunchEvalSnapshotInfo();
        } catch {
            setLaunchEvalSnapshotStatus("Launch eval candidate snapshot failed.");
        }
    };

    const containerVariants = {
        hidden: { opacity: 0 },
        visible: {
            opacity: 1,
            transition: { staggerChildren: 0.1 }
        }
    };

    const itemVariants = {
        hidden: { opacity: 0, y: 20 },
        visible: { opacity: 1, y: 0, transition: { duration: 0.4 } }
    };

    return (
        <motion.div
            className="space-y-6"
            initial="hidden"
            animate="visible"
            variants={containerVariants}
        >
            <motion.h2
                className="text-3xl font-bold tracking-tight text-glow"
                variants={itemVariants}
            >
                System Settings
            </motion.h2>

            <div className="grid gap-6 md:grid-cols-2">
                <motion.div variants={itemVariants}>
                    <Card>
                        <CardHeader>
                            <CardTitle>System Health</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            {checks.map((check, idx) => (
                                <motion.div
                                    key={check.name}
                                    className="flex items-center justify-between p-3 rounded-lg bg-white/5 border border-white/5"
                                    initial={{ opacity: 0, x: -20 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    transition={{ delay: idx * 0.1 }}
                                >
                                    <div className="flex items-center gap-3">
                                        <div className={`p-2 rounded-full bg-opacity-10 ${check.status === "Active" || check.status === "Operational" || check.status === "Running" || check.status === "Connected" ? "bg-green-500 text-green-500" : "bg-red-500 text-red-500"}`}>
                                            <check.icon className="w-4 h-4" />
                                        </div>
                                        <span className="font-medium">{check.name}</span>
                                    </div>
                                    <span className={`text-xs font-mono px-2 py-1 rounded-full border ${check.status === "Active" || check.status === "Operational" || check.status === "Running" || check.status === "Connected" ? "text-green-400 bg-green-400/10 border-green-400/20" : "text-red-400 bg-red-400/10 border-red-400/20"}`}>
                                        {check.status}
                                    </span>
                                </motion.div>
                            ))}
                        </CardContent>
                    </Card>
                </motion.div>

                <motion.div variants={itemVariants}>
                    <Card>
                        <CardHeader>
                            <CardTitle>Service Control</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="p-4 rounded-lg bg-white/5 border border-white/5">
                                <div className="flex items-center justify-between mb-3">
                                    <div className="flex items-center gap-2">
                                        <Power className={`w-4 h-4 ${n8nStatus === "success" ? "text-green-400" : "text-orange-400"}`} />
                                        <span className="font-medium">n8n Server</span>
                                    </div>
                                    {n8nStatus === "success" ? (
                                        <span className="text-xs text-green-400">Running</span>
                                    ) : (
                                        <span className="text-xs text-red-400">Failed</span>
                                    )}
                                </div>
                                <motion.button
                                    onClick={handleN8nRestart}
                                    disabled={n8nRestarting}
                                    className="w-full py-2 rounded-lg bg-orange-500/10 text-orange-400 hover:bg-orange-500/20 transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
                                    whileHover={{ scale: 1.02 }}
                                    whileTap={{ scale: 0.98 }}
                                >
                                    <RefreshCw className={`w-4 h-4 ${n8nRestarting ? 'animate-spin' : ''}`} />
                                    {n8nRestarting ? "Restarting..." : "Restart n8n Server"}
                                </motion.button>
                            </div>

                            <div className="p-4 rounded-lg bg-white/5 border border-white/5">
                                <div className="flex items-center justify-between mb-3">
                                    <div className="flex items-center gap-2">
                                        <Power className="w-4 h-4 text-sky-400" />
                                        <span className="font-medium">Telegram Listener</span>
                                    </div>
                                </div>
                                <div className="flex gap-2">
                                    <motion.button
                                        onClick={handleTelegramListenerStart}
                                        disabled={telegramListenerBusy}
                                        className="flex-1 py-2 rounded-lg bg-sky-500/10 text-sky-400 hover:bg-sky-500/20 transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
                                        whileHover={{ scale: 1.02 }}
                                        whileTap={{ scale: 0.98 }}
                                    >
                                        <Power className="w-4 h-4" />
                                        {telegramListenerBusy ? "Starting..." : "Start Listener"}
                                    </motion.button>
                                    <motion.button
                                        onClick={handleTelegramListenerStatus}
                                        disabled={telegramListenerBusy}
                                        className="flex-1 py-2 rounded-lg bg-white/5 text-muted-foreground hover:bg-white/10 transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
                                        whileHover={{ scale: 1.02 }}
                                        whileTap={{ scale: 0.98 }}
                                    >
                                        <RefreshCw className={`w-4 h-4 ${telegramListenerBusy ? "animate-spin" : ""}`} />
                                        Check Status
                                    </motion.button>
                                </div>
                                {telegramListenerStatus && (
                                    <div className="mt-3 text-xs text-muted-foreground whitespace-pre-wrap">
                                        {telegramListenerStatus}
                                    </div>
                                )}
                            </div>

                            <div className="pt-4 border-t border-white/10 text-sm text-muted-foreground">
                                <div className="flex justify-between mb-2">
                                    <span>Core Version</span>
                                    <span className="font-mono text-white">v0.1.0-alpha</span>
                                </div>
                                <div className="flex justify-between">
                                    <span>Frontend Version</span>
                                    <span className="font-mono text-white">v0.1.0-alpha</span>
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                </motion.div>

                <motion.div variants={itemVariants} className="md:col-span-2">
                    <LaunchReadinessOpsCard
                        chatMetrics={launchChatMetrics}
                        nlRunMetrics={launchNlRunMetrics}
                        execApprovalMetrics={launchExecApprovalMetrics}
                        recommendationMetrics={launchRecommendationMetrics}
                        recommendationReviewMetrics={launchRecommendationReviewMetrics}
                        recentEvents={launchRecentEvents}
                        nlErrorRate={launchNlErrorRate}
                        loading={launchOpsLoading}
                        onRefresh={() => {
                            void refetchLaunchOps();
                        }}
                    />
                </motion.div>

                <motion.div variants={itemVariants} className="md:col-span-2">
                    <MemorySafetyCard
                        metrics={memoryMetrics}
                        requestMemory={requestMemory}
                        executionMemory={executionMemory}
                        recentMemoryAdminEvents={recentMemoryAdminEvents}
                        recentRecommendationReviewEvents={recentRecommendationReviewEvents}
                        status={memoryStatus}
                        loading={memoryLoading}
                        actionPending={memoryActionMutation.isPending}
                        onRefresh={() => {
                            void refetchMemory();
                        }}
                        onRequestAction={handleRequestMemoryAction}
                        onExecutionAction={handleExecutionMemoryAction}
                    />
                </motion.div>

                <motion.div variants={itemVariants} className="md:col-span-2">
                    <Card>
                        <CardHeader>
                            <CardTitle>Launch Eval Candidates</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="flex gap-2">
                                {([
                                    ["real", "Real"],
                                    ["synthetic", "Dogfood"],
                                ] as const).map(([mode, label]) => (
                                    <button
                                        key={mode}
                                        onClick={() => setLaunchEvalCandidateMode(mode)}
                                        className={`rounded-full px-3 py-1 text-xs transition ${
                                            launchEvalCandidateMode === mode
                                                ? "border border-emerald-400/30 bg-emerald-400/10 text-emerald-300"
                                                : "border border-white/10 bg-white/5 text-muted-foreground hover:bg-white/10"
                                        }`}
                                    >
                                        {label}
                                    </button>
                                ))}
                            </div>
                            <div className="flex items-center justify-between gap-3">
                                <div className="text-xs text-muted-foreground">
                                    {launchEvalCandidateMode === "real"
                                        ? "실제 request/execution memory와 launch ops에서 평가 후보를 뽑아 release readiness용 YAML로 저장합니다."
                                        : "launch_eval.yaml의 curated 시나리오를 synthetic dogfood 후보로 분리해 내부 테스트 snapshot으로 저장합니다."}
                                </div>
                                <div className="flex gap-2">
                                    <motion.button
                                        onClick={handleLaunchEvalSnapshot}
                                        className="rounded-lg bg-white/5 px-3 py-2 text-xs text-muted-foreground hover:bg-white/10 disabled:opacity-50"
                                        whileHover={{ scale: 1.02 }}
                                        whileTap={{ scale: 0.98 }}
                                    >
                                        {launchEvalCandidateMode === "real"
                                            ? "Snapshot YAML"
                                            : "Dogfood Snapshot"}
                                    </motion.button>
                                    <motion.button
                                        onClick={() => refetchLaunchEvalCandidates()}
                                        disabled={launchEvalCandidatesLoading}
                                        className="rounded-lg bg-white/5 px-3 py-2 text-xs text-muted-foreground hover:bg-white/10 disabled:opacity-50"
                                        whileHover={{ scale: 1.02 }}
                                        whileTap={{ scale: 0.98 }}
                                    >
                                        {launchEvalCandidatesLoading ? "Refreshing..." : "Refresh Candidates"}
                                    </motion.button>
                                </div>
                            </div>

                            {launchEvalSnapshotStatus && (
                                <div className="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-xs text-muted-foreground">
                                    {launchEvalSnapshotStatus}
                                </div>
                            )}

                            {launchEvalSnapshotInfo && (
                                <div className="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-xs text-muted-foreground">
                                    snapshot ({launchEvalSnapshotInfo.provenance_filter}): {launchEvalSnapshotInfo.exists ? "present" : "missing"} · scenarios {launchEvalSnapshotInfo.scenario_count} · updated {launchEvalSnapshotInfo.updated_at ?? "unknown"}
                                </div>
                            )}

                            {launchEvalCandidateList.length === 0 ? (
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-muted-foreground">
                                    {launchEvalCandidateMode === "real"
                                        ? "No real launch eval candidates yet."
                                        : "No synthetic dogfood candidates configured."}
                                </div>
                            ) : (
                                <div className="space-y-3">
                                    {launchEvalCandidateList.map((candidate) => (
                                        <div key={candidate.id} className="rounded-lg border border-white/10 bg-white/5 p-3">
                                            <div className="flex items-start justify-between gap-3">
                                                <div>
                                                    <div className="text-sm font-medium">{candidate.title}</div>
                                                    <div className="text-[11px] text-muted-foreground">
                                                        {candidate.provenance} · {candidate.source_kind} · {candidate.scenario_kind} · {candidate.command ?? "unknown"}
                                                    </div>
                                                </div>
                                                <span className="rounded-full border border-emerald-400/20 bg-emerald-400/10 px-2 py-1 text-[10px] font-mono text-emerald-300">
                                                    {candidate.score.toFixed(1)}
                                                </span>
                                            </div>
                                            <div className="mt-2 text-xs text-white/90">{candidate.request_message}</div>
                                            <div className="mt-2 flex flex-wrap gap-2 text-[10px] text-muted-foreground">
                                                {candidate.rationale.map((item) => (
                                                    <span key={`${candidate.id}-${item}`}>{item}</span>
                                                ))}
                                            </div>
                                            <details className="mt-3 rounded-md border border-white/10 bg-black/20 p-3">
                                                <summary className="cursor-pointer text-[11px] text-muted-foreground">
                                                    YAML snippet
                                                </summary>
                                                <pre className="mt-3 overflow-x-auto whitespace-pre-wrap text-[11px] text-white/90">
                                                    {candidate.yaml}
                                                </pre>
                                            </details>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </CardContent>
                    </Card>
                </motion.div>

                <motion.div variants={itemVariants} className="md:col-span-2">
                    <Card>
                        <CardHeader>
                            <CardTitle>Missing Dependencies</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-3">
                            {missingDeps.length === 0 ? (
                                <div className="text-sm text-gray-500">All dependencies are installed.</div>
                            ) : (
                                missingDeps.map((dep) => (
                                    <div
                                        key={dep.name}
                                        className="flex items-center justify-between p-3 rounded-lg bg-white/5 border border-white/5"
                                    >
                                        <div className="flex items-center gap-3">
                                            <div className="p-2 rounded-full bg-red-500/10 text-red-400">
                                                <ShieldCheck className="w-4 h-4" />
                                            </div>
                                            <div>
                                                <div className="font-medium">{dep.name}</div>
                                                <div className="text-xs text-gray-500">Install: {dep.install_cmd}</div>
                                            </div>
                                        </div>
                                    </div>
                                ))
                            )}
                        </CardContent>
                    </Card>
                </motion.div>
            </div>
        </motion.div>
    );
}
