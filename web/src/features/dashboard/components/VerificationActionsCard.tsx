import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useReleaseGate, useVerificationRuns } from "@/lib/hooks";
import { fetchHttpE2EHistory, fetchLatestHttpE2E, fetchLatestReleaseReadiness, fetchReleaseReadinessHistory, runHttpE2E, runPerformanceVerification, runReleaseReadiness, runRuntimeVerification, runVisualVerification, setReleaseBaseline } from "@/lib/api";
import type { HttpE2EHistoryEntry, HttpE2EReport, PerformanceVerification, ReleaseReadiness, ReleaseReadinessHistoryEntry, RuntimeVerifyResult, VisualVerifyResult } from "@/lib/types";
import {
    formatDashboardMonthDayTime,
    formatDashboardNumericMonthDayTime,
} from "@/features/dashboard/formatters";
import { useEffect, useState } from "react";

function formatMetricValue(value: number): string {
    if (Number.isNaN(value)) return "—";
    if (Number.isInteger(value) || Math.abs(value) >= 1000) {
        return Math.round(value).toString();
    }
    return value.toFixed(2);
}

export function VerificationActionsCard() {

    const { data: releaseGate, refetch: refetchReleaseGate } = useReleaseGate();
    const [runBackend, setRunBackend] = useState(true);
    const [runFrontend, setRunFrontend] = useState(true);
    const [runBuildChecks, setRunBuildChecks] = useState(false);
    const [runE2e, setRunE2e] = useState(false);
    const [runtimeResult, setRuntimeResult] = useState<RuntimeVerifyResult | null>(null);
    const [runtimeStatus, setRuntimeStatus] = useState<string | null>(null);
    const [runtimeLoading, setRuntimeLoading] = useState(false);
    const [performanceResult, setPerformanceResult] = useState<PerformanceVerification | null>(null);
    const [performanceStatus, setPerformanceStatus] = useState<string | null>(null);
    const [performanceLoading, setPerformanceLoading] = useState(false);
    const [visualPrompts, setVisualPrompts] = useState("");
    const [visualResult, setVisualResult] = useState<VisualVerifyResult | null>(null);
    const [visualStatus, setVisualStatus] = useState<string | null>(null);
    const [visualLoading, setVisualLoading] = useState(false);
    const [baselineStatus, setBaselineStatus] = useState<string | null>(null);
    const [baselineLoading, setBaselineLoading] = useState(false);
    const [releaseReadinessStatus, setReleaseReadinessStatus] = useState<string | null>(null);
    const [releaseReadinessLoading, setReleaseReadinessLoading] = useState(false);
    const [releaseReadinessReport, setReleaseReadinessReport] = useState<ReleaseReadiness | null>(null);
    const [releaseReadinessHistory, setReleaseReadinessHistory] = useState<ReleaseReadinessHistoryEntry[]>([]);
    const [releaseReadinessHistoryLoading, setReleaseReadinessHistoryLoading] = useState(false);
    const [httpE2eLoading, setHttpE2eLoading] = useState(false);
    const [httpE2eStatus, setHttpE2eStatus] = useState<string | null>(null);
    const [httpE2eReport, setHttpE2eReport] = useState<HttpE2EReport | null>(null);
    const [httpE2eHistory, setHttpE2eHistory] = useState<HttpE2EHistoryEntry[]>([]);
    const [httpE2eHistoryLoading, setHttpE2eHistoryLoading] = useState(false);

    const baselineTime = formatDashboardMonthDayTime(releaseGate?.baseline?.created_at);

    const loadReleaseReadinessHistory = async () => {
        setReleaseReadinessHistoryLoading(true);
        try {
            const history = await fetchReleaseReadinessHistory(5);
            setReleaseReadinessHistory(history);
        } catch {
            setReleaseReadinessHistory([]);
        } finally {
            setReleaseReadinessHistoryLoading(false);
        }
    };

    const loadHttpE2EHistory = async () => {
        setHttpE2eHistoryLoading(true);
        try {
            const history = await fetchHttpE2EHistory(5);
            setHttpE2eHistory(history);
        } catch {
            setHttpE2eHistory([]);
        } finally {
            setHttpE2eHistoryLoading(false);
        }
    };

    useEffect(() => {
        fetchLatestReleaseReadiness()
            .then((report) => {
                if (report) {
                    setReleaseReadinessReport(report);
                }
            })
            .catch(() => {
                setReleaseReadinessReport(null);
            });
        fetchLatestHttpE2E()
            .then((report) => {
                if (report) {
                    setHttpE2eReport(report);
                }
            })
            .catch(() => {
                setHttpE2eReport(null);
            });
        loadReleaseReadinessHistory();
        loadHttpE2EHistory();
    }, []);

    const handleRunRuntime = async () => {
        setRuntimeLoading(true);
        setRuntimeStatus(null);
        try {
            const result = await runRuntimeVerification({
                run_backend: runBackend,
                run_frontend: runFrontend,
                run_build_checks: runBuildChecks,
                run_e2e: runE2e,
            });
            setRuntimeResult(result);
            const issueCount = result.issues?.length ?? 0;
            setRuntimeStatus(issueCount === 0 ? "Runtime verification OK." : `Runtime issues: ${issueCount}`);
        } catch {
            setRuntimeStatus("Runtime verification failed.");
        } finally {
            setRuntimeLoading(false);
        }
    };

    const handleRunPerformance = async () => {
        setPerformanceLoading(true);
        setPerformanceStatus(null);
        try {
            const result = await runPerformanceVerification();
            setPerformanceResult(result);
            setPerformanceStatus(result.ok ? "Performance baseline OK." : "Performance issues detected.");
        } catch {
            setPerformanceStatus("Performance verification failed.");
        } finally {
            setPerformanceLoading(false);
        }
    };

    const handleRunVisual = async () => {
        const prompts = visualPrompts
            .split("\n")
            .map((p) => p.trim())
            .filter(Boolean);
        if (prompts.length === 0) {
            setVisualStatus("Add at least one prompt.");
            return;
        }
        setVisualLoading(true);
        setVisualStatus(null);
        try {
            const result = await runVisualVerification(prompts);
            setVisualResult(result);
            const failed = result.verdicts.filter((v) => !v.ok).length;
            setVisualStatus(failed === 0 ? "Visual checks passed." : `Visual issues: ${failed}`);
        } catch {
            setVisualStatus("Visual verification failed.");
        } finally {
            setVisualLoading(false);
        }
    };

    const handleSetBaseline = async () => {
        setBaselineLoading(true);
        setBaselineStatus(null);
        try {
            const baseline = await setReleaseBaseline();
            const created = baseline.created_at
                ? formatDashboardMonthDayTime(baseline.created_at)
                : "Saved";
            setBaselineStatus(`Baseline saved (${created}).`);
            refetchReleaseGate();
        } catch {
            setBaselineStatus("Failed to set baseline.");
        } finally {
            setBaselineLoading(false);
        }
    };

    const handleRunReleaseReadiness = async () => {
        setReleaseReadinessLoading(true);
        setReleaseReadinessStatus(null);
        try {
            const report = await runReleaseReadiness();
            setReleaseReadinessReport(report);
            setReleaseReadinessStatus(
                `Readiness ${report.status} · eval ${report.launch_eval.passed}/${report.launch_eval.total} · http ${report.http_e2e ? `${report.http_e2e.passed}/${report.http_e2e.total}` : "n/a"} · snapshot ${report.candidate_snapshot.scenario_count} · baseline preserved`
            );
            refetchReleaseGate();
            loadReleaseReadinessHistory();
        } catch {
            setReleaseReadinessStatus("Release readiness failed.");
        } finally {
            setReleaseReadinessLoading(false);
        }
    };

    const handleRunHttpE2E = async () => {
        setHttpE2eLoading(true);
        setHttpE2eStatus(null);
        try {
            const report = await runHttpE2E();
            setHttpE2eReport(report);
            setHttpE2eStatus(`HTTP E2E ${report.passed}/${report.total}`);
            loadHttpE2EHistory();
        } catch {
            setHttpE2eStatus("HTTP E2E failed.");
        } finally {
            setHttpE2eLoading(false);
        }
    };

    const runtimeIssues = runtimeResult?.issues ?? [];
    const runtimeBackendState = runtimeResult
        ? runtimeResult.backend_started
            ? runtimeResult.backend_health
                ? "OK"
                : "Unhealthy"
            : "Not started"
        : "—";
    const runtimeFrontendState = runtimeResult
        ? runtimeResult.frontend_started
            ? runtimeResult.frontend_health
                ? "OK"
                : "Unhealthy"
            : "Not started"
        : "—";
    const backendBuildState = runtimeResult?.backend_build_ok;
    const frontendBuildState = runtimeResult?.frontend_build_ok;
    const e2eState = runtimeResult?.e2e_passed;
    const buildLabel = (value?: boolean | null) => (value === undefined || value === null ? "—" : value ? "OK" : "Fail");

    return (
        <Card className="h-auto mb-4 border-white/10 bg-white/5">
            <CardHeader>
                <CardTitle>Verification Actions</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
                <div className="space-y-2">
                    <div className="text-xs text-muted-foreground">Runtime verification</div>
                    <div className="grid grid-cols-2 gap-2 text-[11px]">
                        <label className="flex items-center gap-2">
                            <input
                                type="checkbox"
                                checked={runBackend}
                                onChange={(e) => setRunBackend(e.target.checked)}
                                className="h-3 w-3"
                            />
                            Backend
                        </label>
                        <label className="flex items-center gap-2">
                            <input
                                type="checkbox"
                                checked={runFrontend}
                                onChange={(e) => setRunFrontend(e.target.checked)}
                                className="h-3 w-3"
                            />
                            Frontend
                        </label>
                        <label className="flex items-center gap-2">
                            <input
                                type="checkbox"
                                checked={runBuildChecks}
                                onChange={(e) => setRunBuildChecks(e.target.checked)}
                                className="h-3 w-3"
                            />
                            Build checks
                        </label>
                        <label className="flex items-center gap-2">
                            <input
                                type="checkbox"
                                checked={runE2e}
                                onChange={(e) => setRunE2e(e.target.checked)}
                                className="h-3 w-3"
                            />
                            E2E
                        </label>
                    </div>
                    <button
                        onClick={handleRunRuntime}
                        disabled={runtimeLoading}
                        className="w-full text-[11px] py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        {runtimeLoading ? "Running…" : "Run runtime verification"}
                    </button>
                    {runtimeStatus && <div className="text-[11px] text-muted-foreground">{runtimeStatus}</div>}
                    {runtimeResult && (
                        <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px] space-y-1">
                            <div className="flex items-center justify-between">
                                <span>Backend</span>
                                <span className={runtimeBackendState === "OK" ? "text-emerald-300" : "text-rose-300"}>
                                    {runtimeBackendState}
                                </span>
                            </div>
                            <div className="flex items-center justify-between">
                                <span>Frontend</span>
                                <span className={runtimeFrontendState === "OK" ? "text-emerald-300" : "text-rose-300"}>
                                    {runtimeFrontendState}
                                </span>
                            </div>
                            <div className="flex items-center justify-between">
                                <span>Build</span>
                                <span className={backendBuildState === false || frontendBuildState === false ? "text-rose-300" : "text-muted-foreground"}>
                                    Backend {buildLabel(backendBuildState)} · Frontend {buildLabel(frontendBuildState)}
                                </span>
                            </div>
                            {runE2e && (
                                <div className="flex items-center justify-between">
                                    <span>E2E</span>
                                    <span className={e2eState ? "text-emerald-300" : "text-rose-300"}>
                                        {e2eState === undefined ? "—" : e2eState ? "OK" : "Fail"}
                                    </span>
                                </div>
                            )}
                            {runtimeIssues.length > 0 && (
                                <div className="mt-1 text-rose-200">
                                    {runtimeIssues.slice(0, 3).map((issue, idx) => (
                                        <div key={`${issue}-${idx}`} className="truncate">
                                            • {issue}
                                        </div>
                                    ))}
                                </div>
                            )}
                        </div>
                    )}
                </div>

                <div className="space-y-2">
                    <div className="text-xs text-muted-foreground">Performance baseline</div>
                    <button
                        onClick={handleRunPerformance}
                        disabled={performanceLoading}
                        className="w-full text-[11px] py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        {performanceLoading ? "Running…" : "Run performance check"}
                    </button>
                    {performanceStatus && <div className="text-[11px] text-muted-foreground">{performanceStatus}</div>}
                    {performanceResult && (
                        <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px] space-y-1">
                            {performanceResult.metrics.map((metric) => (
                                <div key={metric.name} className="flex items-center justify-between">
                                    <span>{metric.name}</span>
                                    <span className={metric.ok ? "text-emerald-300" : "text-rose-300"}>
                                        {formatMetricValue(metric.value)} / {formatMetricValue(metric.threshold)}
                                    </span>
                                </div>
                            ))}
                        </div>
                    )}
                </div>

                <div className="space-y-2">
                    <div className="text-xs text-muted-foreground">Visual verification</div>
                    <textarea
                        value={visualPrompts}
                        onChange={(e) => setVisualPrompts(e.target.value)}
                        placeholder="One prompt per line (e.g. Error banner is hidden)"
                        rows={2}
                        className="w-full rounded-md bg-white/5 border border-white/10 px-2 py-1.5 text-[11px]"
                    />
                    <button
                        onClick={handleRunVisual}
                        disabled={visualLoading}
                        className="w-full text-[11px] py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        {visualLoading ? "Running…" : "Run visual check"}
                    </button>
                    {visualStatus && <div className="text-[11px] text-muted-foreground">{visualStatus}</div>}
                    {visualResult && (
                        <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px] space-y-1">
                            {visualResult.verdicts.length === 0 ? (
                                <div className="text-muted-foreground">No prompts checked.</div>
                            ) : (
                                visualResult.verdicts.slice(0, 4).map((verdict) => (
                                    <div key={verdict.prompt} className="flex items-center justify-between gap-2">
                                        <span className="truncate">{verdict.prompt}</span>
                                        <span className={verdict.ok ? "text-emerald-300" : "text-rose-300"}>
                                            {verdict.ok ? "OK" : "Fail"}
                                        </span>
                                    </div>
                                ))
                            )}
                        </div>
                    )}
                </div>

                <div className="space-y-2">
                    <div className="flex items-center justify-between text-xs text-muted-foreground">
                        <span>Release baseline</span>
                        <span>{baselineTime}</span>
                    </div>
                    <button
                        onClick={handleSetBaseline}
                        disabled={baselineLoading}
                        className="w-full text-[11px] py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        {baselineLoading ? "Saving…" : "Set release baseline"}
                    </button>
                    {baselineStatus && <div className="text-[11px] text-muted-foreground">{baselineStatus}</div>}
                </div>
                <div className="space-y-2">
                    <div className="flex items-center justify-between text-xs text-muted-foreground">
                        <span>Release readiness</span>
                        <span>{formatDashboardMonthDayTime(releaseReadinessReport?.generated_at)}</span>
                    </div>
                    <button
                        onClick={handleRunReleaseReadiness}
                        disabled={releaseReadinessLoading}
                        className="w-full text-[11px] py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        {releaseReadinessLoading ? "Running…" : "Run release readiness (read-only)"}
                    </button>
                    {releaseReadinessStatus && <div className="text-[11px] text-muted-foreground">{releaseReadinessStatus}</div>}
                    {!releaseReadinessStatus && (
                        <div className="text-[11px] text-muted-foreground">
                            Read-only verification. Use &quot;Set release baseline&quot; only when intentionally resetting the baseline.
                        </div>
                    )}
                    {releaseReadinessReport && (
                        <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px] space-y-1">
                            <div className="flex items-center justify-between">
                                <span>Status</span>
                                <span className={
                                    releaseReadinessReport.ready_for_launch
                                        ? "text-emerald-300"
                                        : releaseReadinessReport.status === "needs_data"
                                            ? "text-amber-300"
                                            : "text-rose-300"
                                }>
                                    {releaseReadinessReport.status}
                                </span>
                            </div>
                            <div className="flex items-center justify-between">
                                <span>Candidate snapshot</span>
                                <span className={releaseReadinessReport.candidate_snapshot.scenario_count < 5 ? "text-amber-300" : "text-emerald-300"}>
                                    {releaseReadinessReport.candidate_snapshot.scenario_count} scenarios
                                </span>
                            </div>
                            <div className="flex items-center justify-between">
                                <span>Launch eval</span>
                                <span className={releaseReadinessReport.launch_eval.failed > 0 ? "text-rose-300" : "text-emerald-300"}>
                                    {releaseReadinessReport.launch_eval.passed}/{releaseReadinessReport.launch_eval.total}
                                </span>
                            </div>
                            <div className="flex items-center justify-between">
                                <span>Release gate</span>
                                <span className={releaseReadinessReport.release_gate.ok ? "text-emerald-300" : "text-rose-300"}>
                                    {releaseReadinessReport.release_gate.ok ? "OK" : "Issue"}
                                </span>
                            </div>
                            <div className="flex items-center justify-between">
                                <span>HTTP E2E</span>
                                <span className={
                                    releaseReadinessReport.http_e2e
                                        ? releaseReadinessReport.http_e2e.ok
                                            ? "text-emerald-300"
                                            : "text-rose-300"
                                        : releaseReadinessReport.http_e2e_load_error
                                            ? "text-rose-300"
                                            : "text-amber-300"
                                }>
                                    {releaseReadinessReport.http_e2e
                                        ? `${releaseReadinessReport.http_e2e.passed}/${releaseReadinessReport.http_e2e.total}`
                                        : releaseReadinessReport.http_e2e_load_error
                                            ? "Load error"
                                            : "Missing"}
                                </span>
                            </div>
                            {releaseReadinessReport.blockers.length > 0 && (
                                <div className="text-rose-200">
                                    {releaseReadinessReport.blockers.slice(0, 3).map((item) => (
                                        <div key={item} className="truncate">• {item}</div>
                                    ))}
                                </div>
                            )}
                            {releaseReadinessReport.blockers.length === 0 && releaseReadinessReport.advisories.length > 0 && (
                                <div className="text-amber-200">
                                    {releaseReadinessReport.advisories.slice(0, 3).map((item) => (
                                        <div key={item} className="truncate">• {item}</div>
                                    ))}
                                </div>
                            )}
                            {!releaseReadinessReport.http_e2e && releaseReadinessReport.http_e2e_load_error && (
                                <div className="text-rose-200 truncate">
                                    • {releaseReadinessReport.http_e2e_load_error}
                                </div>
                            )}
                            <div className="text-muted-foreground truncate">
                                {releaseReadinessReport.report_markdown_path}
                            </div>
                            {releaseReadinessReport.archived_history_markdown_path && (
                                <div className="text-muted-foreground truncate">
                                    archive: {releaseReadinessReport.archived_history_markdown_path}
                                </div>
                            )}
                            {releaseReadinessReport.history_trend && (
                                <div className="mt-2 rounded border border-white/10 bg-white/5 p-2">
                                    <div className="flex items-center justify-between">
                                        <span className="text-muted-foreground">Trend</span>
                                        <span className={
                                            releaseReadinessReport.history_trend.warnings.length > 0
                                                ? "text-amber-300"
                                                : "text-emerald-300"
                                        }>
                                            {releaseReadinessReport.history_trend.compared_runs} runs
                                        </span>
                                    </div>
                                    <div className="mt-1 text-muted-foreground">
                                        {releaseReadinessReport.history_trend.summary}
                                    </div>
                                    <div className="mt-1 text-muted-foreground">
                                        HTTP E2E delta {releaseReadinessReport.history_trend.http_e2e_pass_rate_delta_pct >= 0 ? "+" : ""}
                                        {releaseReadinessReport.history_trend.http_e2e_pass_rate_delta_pct.toFixed(1)}pp
                                    </div>
                                    {releaseReadinessReport.history_trend.warnings.length > 0 && (
                                        <div className="mt-1 text-amber-200">
                                            {releaseReadinessReport.history_trend.warnings.slice(0, 3).map((item) => (
                                                <div key={item} className="truncate">• {item}</div>
                                            ))}
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>
                    )}
                    <div className="pt-2 border-t border-white/10 space-y-2">
                        <div className="flex items-center justify-between">
                            <span className="text-xs text-muted-foreground">HTTP source-of-truth smoke</span>
                            <span className="text-[11px] text-muted-foreground">
                                {formatDashboardMonthDayTime(httpE2eReport?.generated_at)}
                            </span>
                        </div>
                        <button
                            onClick={handleRunHttpE2E}
                            disabled={httpE2eLoading}
                            className="w-full text-[11px] py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                        >
                            {httpE2eLoading ? "Running…" : "Run live HTTP E2E"}
                        </button>
                        {httpE2eStatus && <div className="text-[11px] text-muted-foreground">{httpE2eStatus}</div>}
                        {httpE2eReport && (
                            <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px] space-y-1">
                                <div className="flex items-center justify-between">
                                    <span>Status</span>
                                    <span className={httpE2eReport.ok ? "text-emerald-300" : "text-rose-300"}>
                                        {httpE2eReport.passed}/{httpE2eReport.total}
                                    </span>
                                </div>
                                <div className="text-muted-foreground truncate">
                                    {httpE2eReport.report_markdown_path}
                                </div>
                                {!httpE2eReport.ok && (
                                    <div className="text-rose-200">
                                        {httpE2eReport.steps
                                            .filter((step) => !step.ok)
                                            .slice(0, 3)
                                            .map((step) => (
                                                <div key={step.name} className="truncate">• {step.name}: {step.detail}</div>
                                            ))}
                                    </div>
                                )}
                            </div>
                        )}
                        <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px] space-y-1">
                            <div className="flex items-center justify-between">
                                <span className="text-muted-foreground">Recent HTTP E2E history</span>
                                <span className="text-muted-foreground">
                                    {httpE2eHistoryLoading ? "loading" : `${httpE2eHistory.length} runs`}
                                </span>
                            </div>
                            {httpE2eHistory.length === 0 ? (
                                <div className="text-muted-foreground">No archived HTTP E2E runs yet.</div>
                            ) : (
                                httpE2eHistory.map((entry) => (
                                    <div key={`${entry.generated_at}-${entry.report_json_path}`} className="flex items-center justify-between gap-3">
                                        <div className="min-w-0">
                                            <div className="truncate">
                                                {formatDashboardMonthDayTime(entry.generated_at)} · {entry.passed}/{entry.total}
                                            </div>
                                            <div className="truncate text-muted-foreground">
                                                {entry.report_markdown_path}
                                            </div>
                                        </div>
                                        <span className={entry.ok ? "text-emerald-300" : "text-rose-300"}>
                                            {entry.ok ? "OK" : "Fail"}
                                        </span>
                                    </div>
                                ))
                            )}
                        </div>
                    </div>
                    <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px] space-y-1">
                        <div className="flex items-center justify-between">
                            <span className="text-muted-foreground">Recent readiness history</span>
                            <span className="text-muted-foreground">
                                {releaseReadinessHistoryLoading ? "loading" : `${releaseReadinessHistory.length} runs`}
                            </span>
                        </div>
                        {releaseReadinessHistory.length === 0 ? (
                            <div className="text-muted-foreground">No archived release readiness runs yet.</div>
                        ) : (
                            releaseReadinessHistory.map((entry) => (
                                <div key={`${entry.generated_at}-${entry.report_json_path}`} className="flex items-center justify-between gap-3">
                                    <div className="min-w-0">
                                        <div className="truncate">
                                            {formatDashboardMonthDayTime(entry.generated_at)} · snapshot {entry.candidate_snapshot_count} · eval {entry.launch_eval_passed}/{entry.launch_eval_total} · http {entry.http_e2e_passed != null && entry.http_e2e_total != null ? `${entry.http_e2e_passed}/${entry.http_e2e_total}` : "n/a"}
                                        </div>
                                        <div className="truncate text-muted-foreground">
                                            {entry.report_markdown_path}
                                        </div>
                                    </div>
                                    <span className={
                                        entry.ready_for_launch
                                            ? "text-emerald-300"
                                            : entry.status === "needs_data"
                                                ? "text-amber-300"
                                                : "text-rose-300"
                                    }>
                                        {entry.status}
                                    </span>
                                </div>
                            ))
                        )}
                    </div>
                </div>
                <VerificationRunHistory />
            </CardContent>
        </Card>
    );
}

function VerificationRunHistory() {
    const { data: runs } = useVerificationRuns(5);
    if (!runs || runs.length === 0) return null;
    return (
        <div className="space-y-2 mt-4 border-t border-white/10 pt-4">
            <div className="text-xs text-muted-foreground">Recent Verification Runs</div>
            <div className="space-y-1">
                {runs.slice(0, 3).map((run) => (
                    <div key={run.id} className="flex justify-between text-[10px] bg-white/5 p-1 rounded">
                        <span
                            className={
                                run.status === "success"
                                    ? "text-emerald-300"
                                    : run.status === "failure"
                                      ? "text-rose-300"
                                      : "text-amber-300"
                            }
                        >
                            {run.mode?.toUpperCase() ?? "—"}
                        </span>
                        <span className="text-muted-foreground">
                            {formatDashboardNumericMonthDayTime(run.created_at)}
                        </span>
                    </div>
                ))}
            </div>
        </div>
    );
}
