import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { motion } from "framer-motion";
import type {
    ExecutionMemoryRecord,
    MemoryAdminEventRecord,
    MemoryOpsMetrics,
    RecommendationReviewEventRecord,
    RequestMemoryRecord,
} from "@/lib/types";

type MemorySafetyCardProps = {
    metrics?: MemoryOpsMetrics;
    requestMemory: RequestMemoryRecord[];
    executionMemory: ExecutionMemoryRecord[];
    recentMemoryAdminEvents: MemoryAdminEventRecord[];
    recentRecommendationReviewEvents: RecommendationReviewEventRecord[];
    status: string | null;
    loading: boolean;
    actionPending: boolean;
    onRefresh: () => void;
    onRequestAction: (op: "suppress" | "restore" | "delete", record: RequestMemoryRecord) => void;
    onExecutionAction: (op: "suppress" | "restore" | "delete", record: ExecutionMemoryRecord) => void;
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

export function MemorySafetyCard({
    metrics,
    requestMemory,
    executionMemory,
    recentMemoryAdminEvents,
    recentRecommendationReviewEvents,
    status,
    loading,
    actionPending,
    onRefresh,
    onRequestAction,
    onExecutionAction,
}: MemorySafetyCardProps) {
    return (
        <Card>
            <CardHeader>
                <CardTitle>Memory Safety</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
                <div className="grid gap-3 md:grid-cols-4">
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                        <div className="text-xs text-muted-foreground">Request Active</div>
                        <div className="mt-1 text-xl font-semibold">{metrics?.request_active ?? 0}</div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                        <div className="text-xs text-muted-foreground">Request Suppressed</div>
                        <div className="mt-1 text-xl font-semibold text-amber-300">{metrics?.request_suppressed ?? 0}</div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                        <div className="text-xs text-muted-foreground">Execution Active</div>
                        <div className="mt-1 text-xl font-semibold">{metrics?.execution_active ?? 0}</div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                        <div className="text-xs text-muted-foreground">Execution Suppressed</div>
                        <div className="mt-1 text-xl font-semibold text-amber-300">{metrics?.execution_suppressed ?? 0}</div>
                    </div>
                </div>

                <div className="flex items-center justify-between gap-3">
                    <div className="text-xs text-muted-foreground">
                        최근 memory를 보고 잘못 학습된 cache를 즉시 차단하거나 삭제합니다.
                    </div>
                    <motion.button
                        onClick={onRefresh}
                        disabled={loading || actionPending}
                        className="rounded-lg bg-white/5 px-3 py-2 text-xs text-muted-foreground hover:bg-white/10 disabled:opacity-50"
                        whileHover={{ scale: 1.02 }}
                        whileTap={{ scale: 0.98 }}
                    >
                        {loading ? "Refreshing..." : "Refresh Memory"}
                    </motion.button>
                </div>

                {status && (
                    <div className="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-xs text-muted-foreground">
                        {status}
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
                                            onClick={() => onRequestAction("restore", record)}
                                            disabled={actionPending}
                                            className="rounded-lg bg-emerald-500/10 px-3 py-1.5 text-xs text-emerald-300 hover:bg-emerald-500/20 disabled:opacity-50"
                                            whileHover={{ scale: 1.02 }}
                                            whileTap={{ scale: 0.98 }}
                                        >
                                            Restore
                                        </motion.button>
                                    ) : (
                                        <motion.button
                                            onClick={() => onRequestAction("suppress", record)}
                                            disabled={actionPending}
                                            className="rounded-lg bg-amber-500/10 px-3 py-1.5 text-xs text-amber-300 hover:bg-amber-500/20 disabled:opacity-50"
                                            whileHover={{ scale: 1.02 }}
                                            whileTap={{ scale: 0.98 }}
                                        >
                                            Suppress
                                        </motion.button>
                                    )}
                                    <motion.button
                                        onClick={() => onRequestAction("delete", record)}
                                        disabled={actionPending}
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
                                            onClick={() => onExecutionAction("restore", record)}
                                            disabled={actionPending}
                                            className="rounded-lg bg-emerald-500/10 px-3 py-1.5 text-xs text-emerald-300 hover:bg-emerald-500/20 disabled:opacity-50"
                                            whileHover={{ scale: 1.02 }}
                                            whileTap={{ scale: 0.98 }}
                                        >
                                            Restore
                                        </motion.button>
                                    ) : (
                                        <motion.button
                                            onClick={() => onExecutionAction("suppress", record)}
                                            disabled={actionPending}
                                            className="rounded-lg bg-amber-500/10 px-3 py-1.5 text-xs text-amber-300 hover:bg-amber-500/20 disabled:opacity-50"
                                            whileHover={{ scale: 1.02 }}
                                            whileTap={{ scale: 0.98 }}
                                        >
                                            Suppress
                                        </motion.button>
                                    )}
                                    <motion.button
                                        onClick={() => onExecutionAction("delete", record)}
                                        disabled={actionPending}
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
                    ) : recentMemoryAdminEvents.map((event) => (
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
                    ) : recentRecommendationReviewEvents.map((event) => (
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
    );
}
