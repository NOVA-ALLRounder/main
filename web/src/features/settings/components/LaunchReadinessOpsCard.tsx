import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { motion } from "framer-motion";
import type { LaunchOpsResponse } from "@/lib/types";

type LaunchReadinessOpsCardProps = {
    chatMetrics?: LaunchOpsResponse["chat_metrics"];
    nlRunMetrics?: LaunchOpsResponse["nl_run_metrics"];
    execApprovalMetrics?: LaunchOpsResponse["exec_approval_metrics"];
    recommendationMetrics?: LaunchOpsResponse["recommendation_metrics"];
    recommendationReviewMetrics?: LaunchOpsResponse["recommendation_review_metrics"];
    recentEvents: LaunchOpsResponse["recent_events"];
    nlErrorRate: number;
    loading: boolean;
    onRefresh: () => void;
};

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

export function LaunchReadinessOpsCard({
    chatMetrics,
    nlRunMetrics,
    execApprovalMetrics,
    recommendationMetrics,
    recommendationReviewMetrics,
    recentEvents,
    nlErrorRate,
    loading,
    onRefresh,
}: LaunchReadinessOpsCardProps) {
    return (
        <Card>
            <CardHeader>
                <CardTitle>Launch Readiness Ops</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
                <div className="grid gap-3 md:grid-cols-3 xl:grid-cols-5">
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                        <div className="text-xs text-muted-foreground">Recent Requests</div>
                        <div className="mt-1 text-xl font-semibold">
                            {chatMetrics?.total_requests ?? 0}
                        </div>
                        <div className="text-[11px] text-muted-foreground">
                            window {chatMetrics?.window_size ?? 0}
                        </div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                        <div className="text-xs text-muted-foreground">Cached Response Hit Rate</div>
                        <div className="mt-1 text-xl font-semibold">
                            {(chatMetrics?.cached_response_hit_rate ?? 0).toFixed(1)}%
                        </div>
                        <div className="text-[11px] text-muted-foreground">
                            req {chatMetrics?.request_memory_hits ?? 0} · exec {chatMetrics?.execution_memory_hits ?? 0}
                        </div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                        <div className="text-xs text-muted-foreground">Intent Acceleration</div>
                        <div className="mt-1 text-xl font-semibold">
                            {chatMetrics?.intent_memory_hits ?? 0}
                        </div>
                        <div className="text-[11px] text-muted-foreground">
                            det {chatMetrics?.deterministic_routes ?? 0} · llm {chatMetrics?.llm_routes ?? 0}
                        </div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                        <div className="text-xs text-muted-foreground">Work Proposal Approval</div>
                        <div className="mt-1 text-xl font-semibold">
                            {(recommendationMetrics?.approval_rate ?? 0).toFixed(1)}%
                        </div>
                        <div className="text-[11px] text-muted-foreground">
                            pending {recommendationMetrics?.pending ?? 0} · later {recommendationMetrics?.later ?? 0}
                        </div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                        <div className="text-xs text-muted-foreground">Write Safety</div>
                        <div className="mt-1 text-xl font-semibold">
                            {(nlRunMetrics?.success_rate ?? 0).toFixed(1)}%
                        </div>
                        <div className="text-[11px] text-muted-foreground">
                            queue {execApprovalMetrics?.pending ?? 0} · expired {execApprovalMetrics?.expired_pending ?? 0}
                        </div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3">
                        <div className="text-xs text-muted-foreground">Review Friction</div>
                        <div className="mt-1 text-xl font-semibold">
                            {(recommendationReviewMetrics?.non_positive_feedback_rate ?? 0).toFixed(1)}%
                        </div>
                        <div className="text-[11px] text-muted-foreground">
                            fail {(recommendationReviewMetrics?.action_failure_rate ?? 0).toFixed(1)}% · events {recommendationReviewMetrics?.total_events ?? 0}
                        </div>
                    </div>
                </div>

                <div className="grid gap-3 md:grid-cols-4 xl:grid-cols-8">
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                        <div className="text-xs text-muted-foreground">Auto Digest</div>
                        <div className="mt-1 font-semibold">{chatMetrics?.ai_digest_auto_routes ?? 0}</div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                        <div className="text-xs text-muted-foreground">Freshness Bypass</div>
                        <div className="mt-1 font-semibold">{chatMetrics?.freshness_bypasses ?? 0}</div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                        <div className="text-xs text-muted-foreground">Blocked</div>
                        <div className="mt-1 font-semibold">{chatMetrics?.blocked_requests ?? 0}</div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                        <div className="text-xs text-muted-foreground">Low Confidence</div>
                        <div className="mt-1 font-semibold">{chatMetrics?.low_confidence_routes ?? 0}</div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                        <div className="text-xs text-muted-foreground">Errors</div>
                        <div className="mt-1 font-semibold text-amber-300">{chatMetrics?.error_routes ?? 0}</div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                        <div className="text-xs text-muted-foreground">Approval Required</div>
                        <div className="mt-1 font-semibold">{nlRunMetrics?.approval_required ?? 0}</div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                        <div className="text-xs text-muted-foreground">NL Run Errors</div>
                        <div className="mt-1 font-semibold text-amber-300">
                            {nlErrorRate.toFixed(1)}%
                        </div>
                    </div>
                    <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm">
                        <div className="text-xs text-muted-foreground">Review Failures</div>
                        <div className="mt-1 font-semibold">
                            {recommendationReviewMetrics?.failed_actions ?? 0}
                        </div>
                    </div>
                </div>

                <div className="flex items-center justify-between gap-3">
                    <div className="text-xs text-muted-foreground">
                        request/execution cache, deterministic route, digest fallback, 추천 승인율, 추천 리뷰 friction, write-action 승인 backlog를 한 카드에서 확인합니다.
                    </div>
                    <motion.button
                        onClick={onRefresh}
                        disabled={loading}
                        className="rounded-lg bg-white/5 px-3 py-2 text-xs text-muted-foreground hover:bg-white/10 disabled:opacity-50"
                        whileHover={{ scale: 1.02 }}
                        whileTap={{ scale: 0.98 }}
                    >
                        {loading ? "Refreshing..." : "Refresh Launch Ops"}
                    </motion.button>
                </div>

                <div className="grid gap-4 md:grid-cols-2">
                    <div className="space-y-3">
                        <div className="text-sm font-semibold">Route Breakdown</div>
                        {(chatMetrics?.route_breakdown ?? []).length === 0 ? (
                            <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-muted-foreground">
                                No launch ops data yet.
                            </div>
                        ) : (
                            chatMetrics?.route_breakdown.map((route) => (
                                <div key={route.route_kind} className="flex items-center justify-between rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm">
                                    <span className="font-mono text-white/90">{route.route_kind}</span>
                                    <span className="text-muted-foreground">{route.count}</span>
                                </div>
                            ))
                        )}
                    </div>

                    <div className="space-y-3">
                        <div className="text-sm font-semibold">Recent Routes</div>
                        {recentEvents.length === 0 ? (
                            <div className="rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-muted-foreground">
                                No recent route events.
                            </div>
                        ) : recentEvents.map((event) => (
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
    );
}
