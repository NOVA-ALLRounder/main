import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Check, ShieldCheck, Database, Server, RefreshCw, Power } from "lucide-react";
import { motion } from "framer-motion";
import axios from "axios";
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
    MemoryAdminEventRecord,
    LaunchOpsResponse,
    RecommendationReviewEventRecord,
    RequestMemoryRecord,
} from "@/lib/types";

type LaunchEvalCandidateMode = "real" | "synthetic";

type SystemHealth = {
    missing_deps?: { name?: string; install_cmd?: string }[];
};

function previewText(value?: string | null): string {
    if (!value) return "No cached response text";
    return value.length > 140 ? `${value.slice(0, 140)}...` : value;
}

function formatLaunchEventTime(value?: string | null): string {
    if (!value) return "—";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return date.toLocaleTimeString("ko-KR", {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
        hour12: false,
    });
}

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
                    <Card>
                        <CardHeader>
                            <CardTitle>Launch Readiness Ops</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="grid gap-3 md:grid-cols-3 xl:grid-cols-5">
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                                    <div className="text-xs text-muted-foreground">Recent Requests</div>
                                    <div className="mt-1 text-xl font-semibold">
                                        {launchChatMetrics?.total_requests ?? 0}
                                    </div>
                                    <div className="text-[11px] text-muted-foreground">
                                        window {launchChatMetrics?.window_size ?? 0}
                                    </div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                                    <div className="text-xs text-muted-foreground">Cached Response Hit Rate</div>
                                    <div className="mt-1 text-xl font-semibold">
                                        {(launchChatMetrics?.cached_response_hit_rate ?? 0).toFixed(1)}%
                                    </div>
                                    <div className="text-[11px] text-muted-foreground">
                                        req {launchChatMetrics?.request_memory_hits ?? 0} · exec {launchChatMetrics?.execution_memory_hits ?? 0}
                                    </div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                                    <div className="text-xs text-muted-foreground">Intent Acceleration</div>
                                    <div className="mt-1 text-xl font-semibold">
                                        {launchChatMetrics?.intent_memory_hits ?? 0}
                                    </div>
                                    <div className="text-[11px] text-muted-foreground">
                                        det {launchChatMetrics?.deterministic_routes ?? 0} · llm {launchChatMetrics?.llm_routes ?? 0}
                                    </div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                                    <div className="text-xs text-muted-foreground">Work Proposal Approval</div>
                                    <div className="mt-1 text-xl font-semibold">
                                        {(launchRecommendationMetrics?.approval_rate ?? 0).toFixed(1)}%
                                    </div>
                                    <div className="text-[11px] text-muted-foreground">
                                        pending {launchRecommendationMetrics?.pending ?? 0} · later {launchRecommendationMetrics?.later ?? 0}
                                    </div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                                    <div className="text-xs text-muted-foreground">Write Safety</div>
                                    <div className="mt-1 text-xl font-semibold">
                                        {(launchNlRunMetrics?.success_rate ?? 0).toFixed(1)}%
                                    </div>
                                    <div className="text-[11px] text-muted-foreground">
                                        queue {launchExecApprovalMetrics?.pending ?? 0} · expired {launchExecApprovalMetrics?.expired_pending ?? 0}
                                    </div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                                    <div className="text-xs text-muted-foreground">Review Friction</div>
                                    <div className="mt-1 text-xl font-semibold">
                                        {(launchRecommendationReviewMetrics?.non_positive_feedback_rate ?? 0).toFixed(1)}%
                                    </div>
                                    <div className="text-[11px] text-muted-foreground">
                                        fail {(launchRecommendationReviewMetrics?.action_failure_rate ?? 0).toFixed(1)}% · events {launchRecommendationReviewMetrics?.total_events ?? 0}
                                    </div>
                                </div>
                            </div>

                            <div className="grid gap-3 md:grid-cols-4 xl:grid-cols-8">
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                                    <div className="text-xs text-muted-foreground">Auto Digest</div>
                                    <div className="mt-1 font-semibold">{launchChatMetrics?.ai_digest_auto_routes ?? 0}</div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                                    <div className="text-xs text-muted-foreground">Freshness Bypass</div>
                                    <div className="mt-1 font-semibold">{launchChatMetrics?.freshness_bypasses ?? 0}</div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                                    <div className="text-xs text-muted-foreground">Blocked</div>
                                    <div className="mt-1 font-semibold">{launchChatMetrics?.blocked_requests ?? 0}</div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                                    <div className="text-xs text-muted-foreground">Low Confidence</div>
                                    <div className="mt-1 font-semibold">{launchChatMetrics?.low_confidence_routes ?? 0}</div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                                    <div className="text-xs text-muted-foreground">Errors</div>
                                    <div className="mt-1 font-semibold text-amber-300">{launchChatMetrics?.error_routes ?? 0}</div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                                    <div className="text-xs text-muted-foreground">Approval Required</div>
                                    <div className="mt-1 font-semibold">{launchNlRunMetrics?.approval_required ?? 0}</div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                                    <div className="text-xs text-muted-foreground">NL Run Errors</div>
                                    <div className="mt-1 font-semibold text-amber-300">
                                        {launchNlErrorRate.toFixed(1)}%
                                    </div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                                    <div className="text-xs text-muted-foreground">Review Failures</div>
                                    <div className="mt-1 font-semibold">
                                        {launchRecommendationReviewMetrics?.failed_actions ?? 0}
                                    </div>
                                </div>
                            </div>

                            <div className="flex items-center justify-between gap-3">
                                <div className="text-xs text-muted-foreground">
                                    request/execution cache, deterministic route, digest fallback, 추천 승인율, 추천 리뷰 friction, write-action 승인 backlog를 한 카드에서 확인합니다.
                                </div>
                                <motion.button
                                    onClick={() => refetchLaunchOps()}
                                    disabled={launchOpsLoading}
                                    className="rounded-lg bg-white/5 px-3 py-2 text-xs text-muted-foreground hover:bg-white/10 disabled:opacity-50"
                                    whileHover={{ scale: 1.02 }}
                                    whileTap={{ scale: 0.98 }}
                                >
                                    {launchOpsLoading ? "Refreshing..." : "Refresh Launch Ops"}
                                </motion.button>
                            </div>

                            <div className="grid gap-4 md:grid-cols-2">
                                <div className="space-y-3">
                                    <div className="text-sm font-semibold">Route Breakdown</div>
                                    {(launchChatMetrics?.route_breakdown ?? []).length === 0 ? (
                                        <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-muted-foreground">
                                            No launch ops data yet.
                                        </div>
                                    ) : (
                                        launchChatMetrics?.route_breakdown.map((route) => (
                                            <div key={route.route_kind} className="flex items-center justify-between rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm">
                                                <span className="font-mono text-white/90">{route.route_kind}</span>
                                                <span className="text-muted-foreground">{route.count}</span>
                                            </div>
                                        ))
                                    )}
                                </div>

                                <div className="space-y-3">
                                    <div className="text-sm font-semibold">Recent Routes</div>
                                    {launchRecentEvents.length === 0 ? (
                                        <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-muted-foreground">
                                            No recent route events.
                                        </div>
                                    ) : launchRecentEvents.map((event) => (
                                        <div key={event.id} className="rounded-lg border border-white/10 bg-white/5 p-3">
                                            <div className="flex items-start justify-between gap-3">
                                                <div>
                                                    <div className="text-sm font-medium">{event.route_kind}</div>
                                                    <div className="text-[11px] text-muted-foreground">
                                                        {formatLaunchEventTime(event.created_at)} · {event.channel ?? "unknown"} · {event.command ?? "no-command"}
                                                    </div>
                                                </div>
                                                <span className={`rounded-full px-2 py-1 text-[10px] font-mono border ${event.outcome === "success" ? "border-emerald-400/20 bg-emerald-400/10 text-emerald-300" : event.outcome === "blocked" ? "border-orange-400/20 bg-orange-400/10 text-orange-300" : "border-amber-400/20 bg-amber-400/10 text-amber-300"}`}>
                                                    {event.outcome}
                                                </span>
                                            </div>
                                            <div className="mt-2 text-xs text-white/90">{event.message_preview}</div>
                                            <div className="mt-2 flex flex-wrap gap-2 text-[10px] text-muted-foreground">
                                                {event.intent_memory_hit && <span>intent-memory</span>}
                                                {event.request_memory_hit && <span>request-cache</span>}
                                                {event.execution_memory_hit && <span>execution-cache</span>}
                                                {event.deterministic_used && <span>deterministic</span>}
                                                {event.llm_used && <span>llm</span>}
                                                {event.ai_digest_used && <span>digest</span>}
                                                {event.freshness_bypassed && <span>fresh-bypass</span>}
                                            </div>
                                            {event.note && (
                                                <div className="mt-2 text-[11px] text-muted-foreground">{event.note}</div>
                                            )}
                                        </div>
                                    ))}
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                </motion.div>

                <motion.div variants={itemVariants} className="md:col-span-2">
                    <Card>
                        <CardHeader>
                            <CardTitle>Memory Safety</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="grid gap-3 md:grid-cols-4">
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                                    <div className="text-xs text-muted-foreground">Request Active</div>
                                    <div className="mt-1 text-xl font-semibold">{memoryMetrics?.request_active ?? 0}</div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                                    <div className="text-xs text-muted-foreground">Request Suppressed</div>
                                    <div className="mt-1 text-xl font-semibold text-amber-300">{memoryMetrics?.request_suppressed ?? 0}</div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                                    <div className="text-xs text-muted-foreground">Execution Active</div>
                                    <div className="mt-1 text-xl font-semibold">{memoryMetrics?.execution_active ?? 0}</div>
                                </div>
                                <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                                    <div className="text-xs text-muted-foreground">Execution Suppressed</div>
                                    <div className="mt-1 text-xl font-semibold text-amber-300">{memoryMetrics?.execution_suppressed ?? 0}</div>
                                </div>
                            </div>

                            <div className="flex items-center justify-between gap-3">
                                <div className="text-xs text-muted-foreground">
                                    최근 memory를 보고 잘못 학습된 cache를 즉시 차단하거나 삭제합니다.
                                </div>
                                <motion.button
                                    onClick={() => refetchMemory()}
                                    disabled={memoryLoading || memoryActionMutation.isPending}
                                    className="rounded-lg bg-white/5 px-3 py-2 text-xs text-muted-foreground hover:bg-white/10 disabled:opacity-50"
                                    whileHover={{ scale: 1.02 }}
                                    whileTap={{ scale: 0.98 }}
                                >
                                    {memoryLoading ? "Refreshing..." : "Refresh Memory"}
                                </motion.button>
                            </div>

                            {memoryStatus && (
                                <div className="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-xs text-muted-foreground">
                                    {memoryStatus}
                                </div>
                            )}

                            <div className="grid gap-4 md:grid-cols-2">
                                <div className="space-y-3">
                                    <div className="text-sm font-semibold">Request Memory</div>
                                    {requestMemory.length === 0 ? (
                                        <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-muted-foreground">
                                            No request memory records.
                                        </div>
                                    ) : requestMemory.map((record) => (
                                        <div key={`${record.memory_scope}-${record.normalized_request}`} className="rounded-lg border border-white/10 bg-white/5 p-3">
                                            <div className="flex items-start justify-between gap-3">
                                                <div>
                                                    <div className="text-sm font-medium">{record.intent_command || "unknown"}</div>
                                                    <div className="text-[11px] text-muted-foreground">{record.memory_scope}</div>
                                                </div>
                                                <span className={`rounded-full px-2 py-1 text-[10px] font-mono border ${record.suppressed ? "border-amber-400/30 bg-amber-400/10 text-amber-300" : "border-emerald-400/20 bg-emerald-400/10 text-emerald-300"}`}>
                                                    {record.suppressed ? "suppressed" : "active"}
                                                </span>
                                            </div>
                                            <div className="mt-2 text-xs text-white/90">{record.original_request}</div>
                                            <div className="mt-2 text-[11px] text-muted-foreground">{previewText(record.response_text)}</div>
                                            <div className="mt-2 flex flex-wrap gap-2 text-[10px] text-muted-foreground">
                                                <span>use {record.use_count}</span>
                                                <span>+{record.positive_feedback_count}</span>
                                                <span>-{record.negative_feedback_count}</span>
                                            </div>
                                            {record.suppressed_reason && (
                                                <div className="mt-2 text-[11px] text-amber-200">reason: {record.suppressed_reason}</div>
                                            )}
                                            <div className="mt-3 flex gap-2">
                                                {record.suppressed ? (
                                                    <motion.button
                                                        onClick={() => handleRequestMemoryAction("restore", record)}
                                                        disabled={memoryActionMutation.isPending}
                                                        className="rounded-lg bg-emerald-500/10 px-3 py-1.5 text-xs text-emerald-300 hover:bg-emerald-500/20 disabled:opacity-50"
                                                        whileHover={{ scale: 1.02 }}
                                                        whileTap={{ scale: 0.98 }}
                                                    >
                                                        Restore
                                                    </motion.button>
                                                ) : (
                                                    <motion.button
                                                        onClick={() => handleRequestMemoryAction("suppress", record)}
                                                        disabled={memoryActionMutation.isPending}
                                                        className="rounded-lg bg-amber-500/10 px-3 py-1.5 text-xs text-amber-300 hover:bg-amber-500/20 disabled:opacity-50"
                                                        whileHover={{ scale: 1.02 }}
                                                        whileTap={{ scale: 0.98 }}
                                                    >
                                                        Suppress
                                                    </motion.button>
                                                )}
                                                <motion.button
                                                    onClick={() => handleRequestMemoryAction("delete", record)}
                                                    disabled={memoryActionMutation.isPending}
                                                    className="rounded-lg bg-red-500/10 px-3 py-1.5 text-xs text-red-300 hover:bg-red-500/20 disabled:opacity-50"
                                                    whileHover={{ scale: 1.02 }}
                                                    whileTap={{ scale: 0.98 }}
                                                >
                                                    Delete
                                                </motion.button>
                                            </div>
                                        </div>
                                    ))}
                                </div>

                                <div className="space-y-3">
                                    <div className="text-sm font-semibold">Execution Memory</div>
                                    {executionMemory.length === 0 ? (
                                        <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-muted-foreground">
                                            No execution memory records.
                                        </div>
                                    ) : executionMemory.map((record) => (
                                        <div key={`${record.memory_scope}-${record.intent_command}-${record.params_key}`} className="rounded-lg border border-white/10 bg-white/5 p-3">
                                            <div className="flex items-start justify-between gap-3">
                                                <div>
                                                    <div className="text-sm font-medium">{record.intent_command}</div>
                                                    <div className="text-[11px] text-muted-foreground">{record.memory_scope}</div>
                                                </div>
                                                <span className={`rounded-full px-2 py-1 text-[10px] font-mono border ${record.suppressed ? "border-amber-400/30 bg-amber-400/10 text-amber-300" : "border-emerald-400/20 bg-emerald-400/10 text-emerald-300"}`}>
                                                    {record.suppressed ? "suppressed" : "active"}
                                                </span>
                                            </div>
                                            <div className="mt-2 text-xs text-white/90">{record.params_key}</div>
                                            <div className="mt-2 text-[11px] text-muted-foreground">{previewText(record.response_text)}</div>
                                            <div className="mt-2 flex flex-wrap gap-2 text-[10px] text-muted-foreground">
                                                <span>ttl {record.freshness_ttl_seconds}s</span>
                                                <span>use {record.use_count}</span>
                                                <span>+{record.positive_feedback_count}</span>
                                                <span>-{record.negative_feedback_count}</span>
                                            </div>
                                            {record.suppressed_reason && (
                                                <div className="mt-2 text-[11px] text-amber-200">reason: {record.suppressed_reason}</div>
                                            )}
                                            <div className="mt-3 flex gap-2">
                                                {record.suppressed ? (
                                                    <motion.button
                                                        onClick={() => handleExecutionMemoryAction("restore", record)}
                                                        disabled={memoryActionMutation.isPending}
                                                        className="rounded-lg bg-emerald-500/10 px-3 py-1.5 text-xs text-emerald-300 hover:bg-emerald-500/20 disabled:opacity-50"
                                                        whileHover={{ scale: 1.02 }}
                                                        whileTap={{ scale: 0.98 }}
                                                    >
                                                        Restore
                                                    </motion.button>
                                                ) : (
                                                    <motion.button
                                                        onClick={() => handleExecutionMemoryAction("suppress", record)}
                                                        disabled={memoryActionMutation.isPending}
                                                        className="rounded-lg bg-amber-500/10 px-3 py-1.5 text-xs text-amber-300 hover:bg-amber-500/20 disabled:opacity-50"
                                                        whileHover={{ scale: 1.02 }}
                                                        whileTap={{ scale: 0.98 }}
                                                    >
                                                        Suppress
                                                    </motion.button>
                                                )}
                                                <motion.button
                                                    onClick={() => handleExecutionMemoryAction("delete", record)}
                                                    disabled={memoryActionMutation.isPending}
                                                    className="rounded-lg bg-red-500/10 px-3 py-1.5 text-xs text-red-300 hover:bg-red-500/20 disabled:opacity-50"
                                                    whileHover={{ scale: 1.02 }}
                                                    whileTap={{ scale: 0.98 }}
                                                >
                                                    Delete
                                                </motion.button>
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            </div>

                            <div className="mt-4 space-y-3">
                                <div className="text-sm font-semibold">Recent Memory Admin Events</div>
                                {recentMemoryAdminEvents.length === 0 ? (
                                    <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-muted-foreground">
                                        No memory admin events.
                                    </div>
                                ) : recentMemoryAdminEvents.map((event: MemoryAdminEventRecord) => (
                                    <div key={event.id} className="rounded-lg border border-white/10 bg-white/5 p-3">
                                        <div className="flex items-start justify-between gap-3">
                                            <div className="space-y-1">
                                                <div className="text-sm font-medium">
                                                    {event.kind} {event.action}
                                                </div>
                                                <div className="text-[11px] text-muted-foreground">
                                                    {formatLaunchEventTime(event.created_at)}
                                                    {event.memory_scope ? ` · ${event.memory_scope}` : ""}
                                                    {event.actor ? ` · ${event.actor}` : ""}
                                                </div>
                                            </div>
                                            <span className={`rounded-full px-2 py-1 text-[10px] font-mono border ${event.ok ? "border-emerald-400/20 bg-emerald-400/10 text-emerald-300" : "border-red-400/20 bg-red-400/10 text-red-300"}`}>
                                                {event.ok ? "ok" : "failed"}
                                            </span>
                                        </div>
                                        <div className="mt-2 text-xs text-white/90">{previewText(event.target_key)}</div>
                                        {event.reason && (
                                            <div className="mt-2 text-[11px] text-amber-200">reason: {event.reason}</div>
                                        )}
                                        {event.message && (
                                            <div className="mt-1 text-[11px] text-muted-foreground">{event.message}</div>
                                        )}
                                    </div>
                                ))}
                            </div>

                            <div className="mt-4 space-y-3">
                                <div className="text-sm font-semibold">Recent Recommendation Review Events</div>
                                {recentRecommendationReviewEvents.length === 0 ? (
                                    <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-muted-foreground">
                                        No recommendation review events.
                                    </div>
                                ) : recentRecommendationReviewEvents.map((event: RecommendationReviewEventRecord) => (
                                    <div key={event.id} className="rounded-lg border border-white/10 bg-white/5 p-3">
                                        <div className="flex items-start justify-between gap-3">
                                            <div className="space-y-1">
                                                <div className="text-sm font-medium">
                                                    {event.action} · {event.recommendation_title}
                                                </div>
                                                <div className="text-[11px] text-muted-foreground">
                                                    {formatLaunchEventTime(event.created_at)}
                                                    {event.category ? ` · ${event.category}` : ""}
                                                    {event.status_after ? ` · ${event.status_after}` : ""}
                                                    {event.actor ? ` · ${event.actor}` : ""}
                                                </div>
                                            </div>
                                            <span className={`rounded-full px-2 py-1 text-[10px] font-mono border ${event.ok ? "border-emerald-400/20 bg-emerald-400/10 text-emerald-300" : "border-red-400/20 bg-red-400/10 text-red-300"}`}>
                                                {event.ok ? "ok" : "failed"}
                                            </span>
                                        </div>
                                        {event.note && (
                                            <div className="mt-2 text-[11px] text-amber-200">note: {event.note}</div>
                                        )}
                                        {event.message && (
                                            <div className="mt-1 text-[11px] text-muted-foreground">{event.message}</div>
                                        )}
                                    </div>
                                ))}
                            </div>
                        </CardContent>
                    </Card>
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
