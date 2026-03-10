import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Activity, Lightbulb, RefreshCw, ShieldCheck } from "lucide-react";
import { AuditLog } from "@/components/AuditLog";
import { useSystemStatus, useLogs, useRoutines, useRecommendationMetrics, useQualityScore, useVerificationRuns } from "@/lib/hooks";
import { motion, useReducedMotion } from "framer-motion";
import { formatDashboardLongTime, formatDashboardShortTime } from "@/features/dashboard/formatters";
import { VerificationActionsCard } from "@/features/dashboard/components/VerificationActionsCard";
import { RecommendationsCard } from "@/features/dashboard/components/RecommendationsCard";
import { ExecControlsCard } from "@/features/dashboard/components/ExecControlsCard";
import { QualityGateCard } from "@/features/dashboard/components/QualityGateCard";
import { QuickActionsCard } from "@/features/dashboard/components/QuickActionsCard";
import { FeedbackCard } from "@/features/dashboard/components/FeedbackCard";
import { RoutinesCard } from "@/features/dashboard/components/RoutinesCard";
import { ControlCard } from "@/features/dashboard/components/ControlCard";
import { BetaActionsCard } from "@/features/dashboard/components/BetaActionsCard";
import { NaturalLanguageAutomationCard } from "@/features/dashboard/components/NaturalLanguageAutomationCard";
import { OverviewMetricsGrid } from "@/features/dashboard/components/OverviewMetricsGrid";
import { VerificationTimelineCard } from "@/features/dashboard/components/VerificationTimelineCard";

const containerVariants = {
    hidden: { opacity: 0 },
    visible: { opacity: 1, transition: { staggerChildren: 0.1 } }
};

const cardVariants = {
    hidden: { opacity: 0, y: 20, scale: 0.95 },
    visible: { opacity: 1, y: 0, scale: 1, transition: { duration: 0.4 } }
};

const decimalFormatter = new Intl.NumberFormat(undefined, {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
});

const wholeNumberFormatter = new Intl.NumberFormat(undefined, {
    maximumFractionDigits: 0,
});

export default function Dashboard() {
    // Keep the data stability fixes (isError ignored, placeholderData in hooks)
    const { data: status, isFetching, isError: statusError } = useSystemStatus();
    const { data: logs, isLoading: logsLoading, isError: logsError } = useLogs();
    const { data: verificationRuns } = useVerificationRuns(20);
    const { data: routines } = useRoutines();
    const { data: recMetrics } = useRecommendationMetrics();
    const { data: qualityScore } = useQualityScore();
    const activeRoutinesCount = routines?.filter(r => r.enabled).length ?? 0;
    const isOffline = statusError || logsError;

    // Use stable values
    const cpuValue = decimalFormatter.format(status?.cpu_usage ?? 0);
    const memoryUsed = decimalFormatter.format((status?.memory_used ?? 0) / 1024);
    const memoryTotal = wholeNumberFormatter.format((status?.memory_total ?? 16384) / 1024);
    const approvalRate = wholeNumberFormatter.format(recMetrics?.approval_rate ?? 0);
    const lastRecTime = formatDashboardShortTime(recMetrics?.last_created_at);

    const qualityValue =
        qualityScore?.score?.overall != null
            ? decimalFormatter.format(qualityScore.score.overall)
            : "—";
    const qualityLabel = qualityScore?.score?.recommendation ?? "pending";
    const qualityTime = formatDashboardShortTime(qualityScore?.created_at);
    const prefersReducedMotion = useReducedMotion();
    const topMotionProps = prefersReducedMotion
        ? {}
        : {
            initial: { opacity: 0, y: -20 },
            animate: { opacity: 1, y: 0 },
            transition: { duration: 0.5 },
        };
    const containerMotionProps = prefersReducedMotion
        ? {}
        : {
            variants: containerVariants,
            initial: "hidden" as const,
            animate: "visible" as const,
        };
    const columnMotionProps = prefersReducedMotion
        ? {}
        : {
            initial: { opacity: 0 },
            animate: { opacity: 1 },
            transition: { duration: 0.45 },
        };

    return (
        <div className="space-y-6">
            <motion.div
                className="flex flex-col gap-6 xl:flex-row xl:items-end xl:justify-between"
                {...topMotionProps}
            >
                <div className="space-y-4">
                    <div className="inline-flex items-center gap-2 rounded-full border border-cyan-400/20 bg-cyan-400/10 px-3 py-1 text-[11px] uppercase tracking-[0.28em] text-cyan-50">
                        <span
                            aria-hidden="true"
                            className={`h-2 w-2 rounded-full ${isOffline ? "bg-rose-400" : "bg-emerald-300"}`}
                        />
                        Steer OS Control Surface
                    </div>
                    <div className="space-y-3">
                        <h2 className="text-4xl font-semibold tracking-[-0.06em] text-white [text-wrap:balance] sm:text-5xl">
                            Autonomy Cockpit
                        </h2>
                        <p className="max-w-2xl text-sm leading-6 text-slate-300">
                            Live operating view for execution, recommendation review, and
                            recovery posture across the local agent runtime.
                        </p>
                    </div>
                </div>
                <div className="grid gap-3 sm:grid-cols-3 xl:min-w-[520px]">
                    <div className="glass rounded-2xl px-4 py-3">
                        <div className="flex items-center justify-between gap-3">
                            <div>
                                <div className="text-[10px] uppercase tracking-[0.28em] text-slate-500">
                                    Runtime
                                </div>
                                <div className="mt-2 text-lg font-semibold text-white">
                                    {isOffline ? "Offline" : "Live"}
                                </div>
                            </div>
                            <div className="flex items-center gap-2 text-slate-300">
                                {isFetching && (
                                    <RefreshCw
                                        aria-hidden="true"
                                        className="h-4 w-4 animate-spin text-cyan-300"
                                    />
                                )}
                                <Activity aria-hidden="true" className="h-4 w-4 text-cyan-200" />
                            </div>
                        </div>
                    </div>
                    <div className="glass rounded-2xl px-4 py-3">
                        <div className="flex items-center justify-between gap-3">
                            <div>
                                <div className="text-[10px] uppercase tracking-[0.28em] text-slate-500">
                                    Queue
                                </div>
                                <div className="mt-2 text-lg font-semibold text-white">
                                    {recMetrics?.pending ?? 0} pending
                                </div>
                            </div>
                            <Lightbulb aria-hidden="true" className="h-4 w-4 text-amber-200" />
                        </div>
                    </div>
                    <div className="glass rounded-2xl px-4 py-3">
                        <div className="flex items-center justify-between gap-3">
                            <div>
                                <div className="text-[10px] uppercase tracking-[0.28em] text-slate-500">
                                    Gate
                                </div>
                                <div className="mt-2 text-lg font-semibold text-white">
                                    {qualityLabel}
                                </div>
                            </div>
                            <ShieldCheck
                                aria-hidden="true"
                                className="h-4 w-4 text-emerald-200"
                            />
                        </div>
                    </div>
                </div>
            </motion.div>

            <motion.div
                className="space-y-6"
                {...containerMotionProps}
            >
                {isOffline && (
                    <motion.div variants={prefersReducedMotion ? undefined : cardVariants}>
                        <Card className="border-amber-400/30 bg-amber-500/10">
                            <CardContent className="p-4 text-sm text-amber-200">
                                API unreachable. Check that the core server is running on localhost:5680.
                            </CardContent>
                        </Card>
                    </motion.div>
                )}
                <div className="grid gap-6 xl:grid-cols-[minmax(0,0.86fr)_minmax(0,1.14fr)]">
                    <motion.div variants={prefersReducedMotion ? undefined : cardVariants}>
                        <ControlCard
                            activeRoutinesCount={activeRoutinesCount}
                            pendingRecommendations={recMetrics?.pending ?? 0}
                            isOffline={isOffline}
                        />
                    </motion.div>
                    <motion.div variants={prefersReducedMotion ? undefined : cardVariants}>
                        <NaturalLanguageAutomationCard />
                    </motion.div>
                </div>
                <OverviewMetricsGrid
                    cpuValue={cpuValue}
                    memoryUsed={memoryUsed}
                    memoryTotal={memoryTotal}
                    activeRoutinesCount={activeRoutinesCount}
                    pendingRecommendations={recMetrics?.pending ?? 0}
                    totalRecommendations={recMetrics?.total ?? 0}
                    approvalRate={approvalRate}
                    lastRecTime={lastRecTime}
                    qualityValue={qualityValue}
                    qualityLabel={qualityLabel}
                    qualityTime={qualityTime}
                />
            </motion.div>

            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-7">
                <motion.div
                    className="col-span-4"
                    {...columnMotionProps}
                >
                    <Card className="h-full mb-4">
                        <CardHeader>
                            <CardTitle>System Logs</CardTitle>
                        </CardHeader>
                        <CardContent>
                            <div className="space-y-4 max-h-40 overflow-y-auto">
                                {logsLoading ? (
                                    <p className="text-sm text-muted-foreground">Loading…</p>
                                ) : logs && logs.length > 0 ? (
                                    logs.slice(0, 5).map((log, i) => (
                                        <div
                                            key={`${log.timestamp}-${i}`}
                                            className="flex items-center gap-4 text-sm border-b border-white/5 pb-2 last:border-0"
                                        >
                                            <div className="text-muted-foreground font-mono text-xs w-24 shrink-0">
                                                {formatDashboardLongTime(log.timestamp)}
                                            </div>
                                            <div className="min-w-0 truncate">{log.message}</div>
                                        </div>
                                    ))
                                ) : (
                                    <p className="text-sm text-muted-foreground">No recent logs.</p>
                                )}
                            </div>
                        </CardContent>
                    </Card>
                    <div className="h-60">
                        <AuditLog />
                    </div>
                    <VerificationTimelineCard verificationRuns={verificationRuns ?? []} />
                </motion.div>

                <motion.div
                    className="col-span-3"
                    {...columnMotionProps}
                >
                    <RecommendationsCard />
                    <RoutinesCard />
                    <QualityGateCard />
                    <VerificationActionsCard />
                    <ExecControlsCard />
                    <BetaActionsCard />
                    <FeedbackCard />
                    <QuickActionsCard />
                </motion.div>
            </div>
        </div>
    );
}
