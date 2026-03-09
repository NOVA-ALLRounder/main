import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { RefreshCw } from "lucide-react";
import { AuditLog } from "@/components/AuditLog";
import { useSystemStatus, useLogs, useRoutines, useRecommendationMetrics, useQualityScore, useVerificationRuns } from "@/lib/hooks";
import { format } from "date-fns";
import { motion } from "framer-motion";
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
    const cpuValue = status?.cpu_usage?.toFixed(1) ?? "0";
    const memoryUsed = ((status?.memory_used ?? 0) / 1024).toFixed(1);
    const memoryTotal = ((status?.memory_total ?? 16384) / 1024).toFixed(0);
    const approvalRate = recMetrics?.approval_rate?.toFixed(0) ?? "0";
    const lastRecTime = recMetrics?.last_created_at
        ? format(new Date(recMetrics.last_created_at), "HH:mm")
        : "—";

    const qualityValue = qualityScore?.score?.overall?.toFixed(1) ?? "—";
    const qualityLabel = qualityScore?.score?.recommendation ?? "pending";
    const qualityTime = qualityScore?.created_at
        ? format(new Date(qualityScore.created_at), "HH:mm")
        : "—";

    return (
        <div className="space-y-6">
            <motion.div
                className="flex items-center justify-between"
                initial={{ opacity: 0, y: -20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.5 }}
            >
                <h2 className="text-3xl font-bold tracking-tight text-glow">Dashboard</h2>
                <div className="flex items-center gap-2">
                    {isFetching && (
                        <RefreshCw className="w-4 h-4 animate-spin text-muted-foreground" />
                    )}
                    <span className="relative flex h-3 w-3">
                        <span className={`animate-ping absolute inline-flex h-full w-full rounded-full ${isOffline ? "bg-red-400" : "bg-green-400"} opacity-75`}></span>
                        <span className={`relative inline-flex rounded-full h-3 w-3 ${isOffline ? "bg-red-500" : "bg-green-500"}`}></span>
                    </span>
                    <span className="text-sm text-muted-foreground font-mono">{isOffline ? "Offline" : "Live"}</span>
                </div>
            </motion.div>

            <motion.div
                className="max-w-4xl mx-auto space-y-6"
                variants={containerVariants}
                initial="hidden"
                animate="visible"
            >
                {isOffline && (
                    <motion.div variants={cardVariants}>
                        <Card className="border-amber-400/30 bg-amber-500/10">
                            <CardContent className="p-4 text-sm text-amber-200">
                                API unreachable. Check that the core server is running on localhost:5680.
                            </CardContent>
                        </Card>
                    </motion.div>
                )}
                <ControlCard />
                <NaturalLanguageAutomationCard />
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
                    initial={{ opacity: 0, x: -20 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: 0.2, duration: 0.5 }}
                >
                    <Card className="h-full mb-4">
                        <CardHeader>
                            <CardTitle>System Logs</CardTitle>
                        </CardHeader>
                        <CardContent>
                            <div className="space-y-4 max-h-40 overflow-y-auto">
                                {logsLoading ? (
                                    <p className="text-sm text-muted-foreground">Loading...</p>
                                ) : logs && logs.length > 0 ? (
                                    logs.slice(0, 5).map((log, i) => (
                                        <div
                                            key={`${log.timestamp}-${i}`}
                                            className="flex items-center gap-4 text-sm border-b border-white/5 pb-2 last:border-0"
                                        >
                                            <div className="text-muted-foreground font-mono text-xs w-24 shrink-0">
                                                {log.timestamp ? format(new Date(log.timestamp), "HH:mm:ss") : "—"}
                                            </div>
                                            <div className="truncate">{log.message}</div>
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
                    initial={{ opacity: 0, x: 20 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: 0.3, duration: 0.5 }}
                >
                    <RecommendationsCard />
                    <RoutinesCard />
                    <QualityGateCard />
                    <VerificationActionsCard />
                    <ExecControlsCard />
                    <BetaActionsCard />
                    <FeedbackCard />
                    <FeedbackCard />
                    <QuickActionsCard />
                </motion.div>
            </div>
        </div>
    );
}
