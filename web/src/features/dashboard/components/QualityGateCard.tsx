import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { calculateQualityScore, type ReleaseGateOverrides } from "@/lib/api";
import {
    useConsistencyCheck,
    useQualityScore,
    useReleaseGate,
    useSemanticVerification,
} from "@/lib/hooks";
import { formatDashboardShortTime } from "@/features/dashboard/formatters";
import { ShieldCheck } from "lucide-react";
import { useState } from "react";

export function QualityGateCard() {
    const { data: qualityScore, refetch: refetchQuality, isFetching: qualityFetching } = useQualityScore();
    const { data: consistency, refetch: refetchConsistency, isFetching: consistencyFetching } = useConsistencyCheck();
    const { data: semantic, refetch: refetchSemantic, isFetching: semanticFetching } = useSemanticVerification();
    const [gateOverrides, setGateOverrides] = useState<ReleaseGateOverrides | undefined>(undefined);
    const [perfInput, setPerfInput] = useState("");
    const [qualityInput, setQualityInput] = useState("");
    const { data: releaseGate, refetch: refetchReleaseGate, isFetching: releaseFetching } = useReleaseGate(gateOverrides);
    const [expanded, setExpanded] = useState(false);
    const [issueSearch, setIssueSearch] = useState("");
    const [issueTypeFilter, setIssueTypeFilter] = useState("all");
    const [issueSeverityFilter, setIssueSeverityFilter] = useState("all");

    const loading = qualityFetching || consistencyFetching || semanticFetching || releaseFetching;
    const handleRefresh = async () => {
        await Promise.all([
            refetchQuality(),
            refetchConsistency(),
            refetchSemantic(),
            refetchReleaseGate(),
        ]);
    };

    const qualityValue = qualityScore?.score?.overall?.toFixed(1) ?? "—";
    const qualityLabel = qualityScore?.score?.recommendation ?? "pending";
    const qualityTime = formatDashboardShortTime(qualityScore?.created_at);

    const gateOk = releaseGate?.ok ?? false;
    const gateWarnings = releaseGate?.warnings?.length ?? 0;
    const gateRegressions = releaseGate?.regressions?.length ?? 0;
    const gateStatus = releaseGate
        ? gateOk
            ? gateWarnings > 0
                ? `PASS · ${gateWarnings} warn`
                : "PASS"
            : `FAIL · ${gateRegressions}`
        : "—";
    const gateTime = formatDashboardShortTime(releaseGate?.current?.created_at);

    const consistencyCount = consistency?.issues?.length ?? 0;
    const semanticCount = semantic?.issues?.length ?? 0;
    const currentLaunchOps = releaseGate?.current?.launch_ops;
    const currentNlRunMetrics = releaseGate?.current?.nl_run_metrics;
    const currentExecApprovalMetrics = releaseGate?.current?.exec_approval_metrics;
    const currentRecommendationMetrics = releaseGate?.current?.recommendation_metrics;
    const currentRecommendationReviewMetrics = releaseGate?.current?.recommendation_review_metrics;
    const launchErrorRate = currentLaunchOps && currentLaunchOps.total_requests > 0
        ? (currentLaunchOps.error_routes / currentLaunchOps.total_requests) * 100
        : 0;
    const launchLowConfidenceRate = currentLaunchOps && currentLaunchOps.total_requests > 0
        ? (currentLaunchOps.low_confidence_routes / currentLaunchOps.total_requests) * 100
        : 0;
    const nlRunErrorRate = currentNlRunMetrics && currentNlRunMetrics.total > 0
        ? (currentNlRunMetrics.error / currentNlRunMetrics.total) * 100
        : 0;
    const recommendationReviewCount = currentRecommendationMetrics
        ? currentRecommendationMetrics.approved + currentRecommendationMetrics.rejected
        : 0;
    const recommendationApprovalRate = recommendationReviewCount > 0 && currentRecommendationMetrics
        ? (currentRecommendationMetrics.approved / recommendationReviewCount) * 100
        : 0;
    const reviewActionFailureRate = currentRecommendationReviewMetrics?.action_failure_rate ?? 0;
    const reviewNonPositiveRate = currentRecommendationReviewMetrics?.non_positive_feedback_rate ?? 0;
    const currentLaunchEval = releaseGate?.current?.launch_eval;
    const currentLaunchEvalSnapshot = releaseGate?.current?.launch_eval_candidate_snapshot;
    const currentLaunchEvalSnapshotRefreshError =
        releaseGate?.current?.launch_eval_candidate_snapshot_refresh_error;
    const hasAnyIssues = gateRegressions > 0 || gateWarnings > 0 || consistencyCount > 0 || semanticCount > 0;

    const handleApplyGateOverrides = () => {
        const perf = Number.isFinite(Number(perfInput)) ? Number(perfInput) : undefined;
        const quality = Number.isFinite(Number(qualityInput)) ? Number(qualityInput) : undefined;
        if (perf === undefined && quality === undefined) {
            setGateOverrides(undefined);
        } else {
            setGateOverrides({
                perf_regression_pct: perf,
                quality_drop: quality,
            });
        }
        refetchReleaseGate();
    };

    const handleResetGateOverrides = () => {
        setPerfInput("");
        setQualityInput("");
        setGateOverrides(undefined);
        refetchReleaseGate();
    };

    const gateTone = !releaseGate
        ? "border-white/10 bg-white/5"
        : gateOk
            ? gateWarnings > 0
                ? "border-amber-500/20 bg-amber-500/5"
                : "border-emerald-500/20 bg-emerald-500/5"
            : "border-rose-500/20 bg-rose-500/5";

    const hasOverrides = gateOverrides?.perf_regression_pct !== undefined || gateOverrides?.quality_drop !== undefined;

    const issueItems = [
        ...(releaseGate?.regressions ?? []).map((item) => ({
            kind: "release_regression",
            severity: "high",
            text: item,
        })),
        ...(releaseGate?.warnings ?? []).map((item) => ({
            kind: "release_warning",
            severity: "medium",
            text: item,
        })),
        ...(consistency?.issues ?? []).map((issue) => ({
            kind: "consistency",
            severity: "medium",
            text: `${issue.path} · ${issue.source}`,
        })),
        ...(semantic?.issues ?? []).map((issue) => ({
            kind: "semantic",
            severity: issue.severity,
            text: `${issue.file} (${issue.severity}) · ${issue.reason}`,
        })),
    ];

    const filteredIssueItems = issueItems.filter((item) => {
        if (issueTypeFilter !== "all" && item.kind !== issueTypeFilter) return false;
        if (issueSeverityFilter !== "all" && item.severity !== issueSeverityFilter) return false;
        if (issueSearch) {
            return item.text.toLowerCase().includes(issueSearch.toLowerCase());
        }
        return true;
    });

    const issuesByKind = (kind: string) => filteredIssueItems.filter((item) => item.kind === kind).map((item) => item.text);

    return (
        <Card className={`h-auto mb-4 ${gateTone}`}>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Quality & Gate</CardTitle>
                <div className="flex gap-2">
                    <button
                        onClick={async () => {
                            await calculateQualityScore();
                            handleRefresh();
                        }}
                        className="text-[10px] bg-white/10 hover:bg-white/20 px-2 py-1 rounded transition-colors"
                    >
                        🔄 Recalculate
                    </button>
                    <ShieldCheck className="h-4 w-4 text-emerald-400" />
                </div>
            </CardHeader>
            <CardContent className="space-y-3">
                <div className="flex items-center justify-between text-xs">
                    <span className="text-muted-foreground">Quality score</span>
                    <span className="text-emerald-300">
                        {qualityValue} · {qualityLabel}
                    </span>
                </div>
                <div className="flex items-center justify-between text-xs">
                    <span className="text-muted-foreground">Release gate</span>
                    <span className={gateOk ? "text-emerald-400" : "text-rose-400"}>
                        {gateStatus}
                    </span>
                </div>
                <div className="flex items-center justify-between text-[11px] text-muted-foreground">
                    <span>Last gate run</span>
                    <span>{gateTime}</span>
                </div>
                <div className="flex items-center justify-between text-xs">
                    <span className="text-muted-foreground">API consistency</span>
                    <span className={consistency?.ok ? "text-emerald-400" : "text-rose-400"}>
                        {consistency?.ok ? "OK" : "Issue"} · {consistencyCount}
                    </span>
                </div>
                <div className="flex items-center justify-between text-xs">
                    <span className="text-muted-foreground">Static/Semantic</span>
                    <span className={semantic?.ok ? "text-emerald-400" : "text-rose-400"}>
                        {semantic?.ok ? "OK" : "Issue"} · {semanticCount}
                    </span>
                </div>
                <div className="text-[11px] text-muted-foreground">
                    Last score {qualityTime}
                </div>
                {currentLaunchOps && (
                    <>
                        <div className="flex items-center justify-between text-xs">
                            <span className="text-muted-foreground">Launch ops cache hit</span>
                            <span className="text-emerald-300">
                                {currentLaunchOps.cached_response_hit_rate.toFixed(1)}%
                            </span>
                        </div>
                        <div className="flex items-center justify-between text-xs">
                            <span className="text-muted-foreground">Launch ops errors</span>
                            <span className={launchErrorRate > 5 ? "text-rose-400" : "text-emerald-300"}>
                                {launchErrorRate.toFixed(1)}% · low-conf {launchLowConfidenceRate.toFixed(1)}%
                            </span>
                        </div>
                    </>
                )}
                {currentRecommendationMetrics && (
                    <div className="flex items-center justify-between text-xs">
                        <span className="text-muted-foreground">Workflow approval quality</span>
                        <span className={recommendationApprovalRate < 20 && recommendationReviewCount >= 5 ? "text-rose-400" : "text-emerald-300"}>
                            {recommendationApprovalRate.toFixed(1)}% · pending {currentRecommendationMetrics.pending}
                        </span>
                    </div>
                )}
                {currentRecommendationReviewMetrics && (
                    <div className="flex items-center justify-between text-xs">
                        <span className="text-muted-foreground">Recommendation review friction</span>
                        <span className={reviewActionFailureRate > 10 || reviewNonPositiveRate > 60 ? "text-amber-300" : "text-emerald-300"}>
                            fail {reviewActionFailureRate.toFixed(1)}% · feedback {reviewNonPositiveRate.toFixed(1)}%
                        </span>
                    </div>
                )}
                {currentNlRunMetrics && (
                    <div className="flex items-center justify-between text-xs">
                        <span className="text-muted-foreground">Write safety</span>
                        <span className={currentNlRunMetrics.success_rate < 55 ? "text-rose-400" : "text-emerald-300"}>
                            {currentNlRunMetrics.success_rate.toFixed(1)}% · err {nlRunErrorRate.toFixed(1)}%
                        </span>
                    </div>
                )}
                {currentExecApprovalMetrics && (
                    <div className="flex items-center justify-between text-xs">
                        <span className="text-muted-foreground">Approval backlog</span>
                        <span className={currentExecApprovalMetrics.pending > 8 || currentExecApprovalMetrics.expired_pending > 0 ? "text-amber-300" : "text-emerald-300"}>
                            {currentExecApprovalMetrics.pending} pending · expired {currentExecApprovalMetrics.expired_pending}
                        </span>
                    </div>
                )}
                {currentLaunchEval && (
                    <div className="flex items-center justify-between text-xs">
                        <span className="text-muted-foreground">Launch eval</span>
                        <span className={currentLaunchEval.failed > 0 ? "text-rose-400" : "text-emerald-300"}>
                            {currentLaunchEval.passed}/{currentLaunchEval.total} · fail {currentLaunchEval.failed}
                        </span>
                    </div>
                )}
                {currentLaunchEvalSnapshot && (
                    <div className="flex items-center justify-between text-xs">
                        <span className="text-muted-foreground">Eval candidate coverage</span>
                        <span className={currentLaunchEvalSnapshot.scenario_count < 5 ? "text-amber-300" : "text-emerald-300"}>
                            {currentLaunchEvalSnapshot.scenario_count} scenarios
                        </span>
                    </div>
                )}
                {currentLaunchEvalSnapshotRefreshError && (
                    <div className="text-[11px] text-amber-300">
                        snapshot refresh: {currentLaunchEvalSnapshotRefreshError}
                    </div>
                )}
                <div className="rounded-md border border-white/10 bg-black/20 p-2 space-y-2">
                    <div className="text-[11px] text-muted-foreground">Gate thresholds (optional)</div>
                    <div className="grid grid-cols-2 gap-2">
                        <input
                            value={perfInput}
                            onChange={(e) => setPerfInput(e.target.value)}
                            placeholder="Perf regression (0.1)"
                            className="w-full rounded-md bg-white/5 border border-white/10 px-2 py-1 text-[11px]"
                        />
                        <input
                            value={qualityInput}
                            onChange={(e) => setQualityInput(e.target.value)}
                            placeholder="Quality drop (0.3)"
                            className="w-full rounded-md bg-white/5 border border-white/10 px-2 py-1 text-[11px]"
                        />
                    </div>
                    <div className="flex gap-2">
                        <button
                            onClick={handleApplyGateOverrides}
                            className="flex-1 text-[11px] py-1 rounded bg-white/10 hover:bg-white/20 transition-colors"
                        >
                            Apply
                        </button>
                        <button
                            onClick={handleResetGateOverrides}
                            className="flex-1 text-[11px] py-1 rounded bg-white/5 hover:bg-white/10 transition-colors"
                        >
                            Reset
                        </button>
                    </div>
                    {hasOverrides && (
                        <div className="text-[10px] text-amber-200">
                            Overrides active
                        </div>
                    )}
                </div>
                {hasAnyIssues && (
                    <button
                        onClick={() => setExpanded((prev) => !prev)}
                        className="text-[11px] text-indigo-200 hover:text-indigo-100"
                    >
                        {expanded ? "Hide details" : "Show details"}
                    </button>
                )}
                {hasAnyIssues && (
                    <Dialog>
                        <DialogTrigger asChild>
                            <button className="text-[11px] text-indigo-200 hover:text-indigo-100">
                                Open issue list
                            </button>
                        </DialogTrigger>
                        <DialogContent className="max-w-2xl">
                            <DialogHeader>
                                <DialogTitle>Verification Issues</DialogTitle>
                            </DialogHeader>
                            <div className="space-y-3">
                                <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
                                    <input
                                        value={issueSearch}
                                        onChange={(e) => setIssueSearch(e.target.value)}
                                        placeholder="Filter issues..."
                                        className="w-full rounded-md bg-white/5 border border-white/10 px-3 py-2 text-sm"
                                    />
                                    <div className="flex gap-2">
                                        <select
                                            value={issueTypeFilter}
                                            onChange={(e) => setIssueTypeFilter(e.target.value)}
                                            className="flex-1 rounded-md bg-white/5 border border-white/10 px-2 py-2 text-sm"
                                        >
                                            <option value="all">All types</option>
                                            <option value="release_regression">Release regressions</option>
                                            <option value="release_warning">Release warnings</option>
                                            <option value="consistency">API consistency</option>
                                            <option value="semantic">Static/Semantic</option>
                                        </select>
                                        <select
                                            value={issueSeverityFilter}
                                            onChange={(e) => setIssueSeverityFilter(e.target.value)}
                                            className="flex-1 rounded-md bg-white/5 border border-white/10 px-2 py-2 text-sm"
                                        >
                                            <option value="all">All severity</option>
                                            <option value="high">High</option>
                                            <option value="medium">Medium</option>
                                            <option value="low">Low</option>
                                        </select>
                                    </div>
                                </div>
                                <div className="space-y-3 max-h-[60vh] overflow-y-auto pr-1">
                                    {issuesByKind("release_regression").length > 0 && (
                                        <IssueSection
                                            title="Release Gate · Regressions"
                                            tone="rose"
                                            items={issuesByKind("release_regression")}
                                            filter=""
                                        />
                                    )}
                                    {issuesByKind("release_warning").length > 0 && (
                                        <IssueSection
                                            title="Release Gate · Warnings"
                                            tone="amber"
                                            items={issuesByKind("release_warning")}
                                            filter=""
                                        />
                                    )}
                                    {issuesByKind("consistency").length > 0 && (
                                        <IssueSection
                                            title="API Consistency"
                                            tone="rose"
                                            items={issuesByKind("consistency")}
                                            filter=""
                                        />
                                    )}
                                    {issuesByKind("semantic").length > 0 && (
                                        <IssueSection
                                            title="Static/Semantic"
                                            tone="amber"
                                            items={issuesByKind("semantic")}
                                            filter=""
                                        />
                                    )}
                                    {filteredIssueItems.length === 0 && (
                                        <div className="text-sm text-muted-foreground">No issues detected.</div>
                                    )}
                                </div>
                            </div>
                        </DialogContent>
                    </Dialog>
                )}
                {expanded && (
                    <div className="space-y-2 text-[11px]">
                        {gateRegressions > 0 && (
                            <div className="rounded-md border border-rose-500/20 bg-rose-500/10 p-2">
                                <div className="font-semibold text-rose-200">Release gate</div>
                                <ul className="mt-1 space-y-1 text-rose-100/80">
                                    {releaseGate?.regressions.slice(0, 3).map((item, idx) => (
                                        <li key={`reg-${idx}`} className="truncate">• {item}</li>
                                    ))}
                                </ul>
                            </div>
                        )}
                        {gateWarnings > 0 && (
                            <div className="rounded-md border border-amber-500/20 bg-amber-500/10 p-2">
                                <div className="font-semibold text-amber-200">Gate warnings</div>
                                <ul className="mt-1 space-y-1 text-amber-100/80">
                                    {releaseGate?.warnings.slice(0, 3).map((item, idx) => (
                                        <li key={`warn-${idx}`} className="truncate">• {item}</li>
                                    ))}
                                </ul>
                            </div>
                        )}
                        {consistencyCount > 0 && (
                            <div className="rounded-md border border-rose-500/20 bg-rose-500/10 p-2">
                                <div className="font-semibold text-rose-200">API Consistency</div>
                                <ul className="mt-1 space-y-1 text-rose-100/80">
                                    {consistency?.issues.slice(0, 3).map((issue, idx) => (
                                        <li key={`${issue.path}-${idx}`} className="truncate">
                                            • {issue.path}
                                        </li>
                                    ))}
                                </ul>
                            </div>
                        )}
                        {semanticCount > 0 && (
                            <div className="rounded-md border border-amber-500/20 bg-amber-500/10 p-2">
                                <div className="font-semibold text-amber-200">Static/Semantic</div>
                                <ul className="mt-1 space-y-1 text-amber-100/80">
                                    {semantic?.issues.slice(0, 3).map((issue, idx) => (
                                        <li key={`${issue.file}-${idx}`} className="truncate">
                                            • {issue.file} ({issue.severity})
                                        </li>
                                    ))}
                                </ul>
                            </div>
                        )}
                    </div>
                )}
                <button
                    onClick={handleRefresh}
                    className="w-full text-[11px] py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors"
                    disabled={loading}
                >
                    {loading ? "Checking..." : "Run checks"}
                </button>
            </CardContent>
        </Card>
    );
}

function IssueSection({
    title,
    tone,
    items,
    filter,
}: {
    title: string;
    tone: "rose" | "amber";
    items: string[];
    filter: string;
}) {
    const filtered = filter
        ? items.filter((item) => item.toLowerCase().includes(filter.toLowerCase()))
        : items;
    if (filtered.length === 0) return null;

    const classes =
        tone === "rose"
            ? "border-rose-500/20 bg-rose-500/10 text-rose-100/80"
            : "border-amber-500/20 bg-amber-500/10 text-amber-100/80";

    return (
        <div className={`rounded-md border p-3 ${classes}`}>
            <div className="font-semibold text-white/90 mb-2">{title}</div>
            <ul className="space-y-1 text-[12px]">
                {filtered.map((item, idx) => (
                    <li key={`${title}-${idx}`} className="break-words">
                        • {item}
                    </li>
                ))}
            </ul>
        </div>
    );
}
