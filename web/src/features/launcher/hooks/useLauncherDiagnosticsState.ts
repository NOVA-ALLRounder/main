import { useCallback, useEffect, useState } from "react";
import {
    fetchLockMetrics,
    fetchRuntimeInfo,
    fetchTaskRunArtifacts,
    fetchTaskRunAssertions,
    fetchTaskRunStages,
    fetchTaskRuns,
} from "@/lib/api";
import { isLocalApiOfflineError } from "@/lib/queryRetry";
import type {
    LockMetrics,
    RuntimeInfo,
    TaskRunArtifact,
    TaskStageAssertion,
    TaskStageRun,
} from "@/lib/types";
import type { DodFailureTopItem, DodHistoryItem } from "@/features/launcher/support";

type UseLauncherDiagnosticsStateParams = {
    showDiagnostics: boolean;
    activeRunId?: string | null;
};

export function useLauncherDiagnosticsState({
    showDiagnostics,
    activeRunId,
}: UseLauncherDiagnosticsStateParams) {
    const [stageRuns, setStageRuns] = useState<TaskStageRun[]>([]);
    const [stageAssertions, setStageAssertions] = useState<TaskStageAssertion[]>([]);
    const [taskRunArtifacts, setTaskRunArtifacts] = useState<TaskRunArtifact[]>([]);
    const [dodHistory, setDodHistory] = useState<DodHistoryItem[]>([]);
    const [dodFailureTop, setDodFailureTop] = useState<DodFailureTopItem[]>([]);
    const [dodHistoryLoading, setDodHistoryLoading] = useState(false);
    const [lockMetrics, setLockMetrics] = useState<LockMetrics | null>(null);
    const [lockMetricsError, setLockMetricsError] = useState<string | null>(null);
    const [runtimeInfo, setRuntimeInfo] = useState<RuntimeInfo | null>(null);
    const [runtimeInfoError, setRuntimeInfoError] = useState<string | null>(null);

    const logDiagnosticsError = useCallback((message: string, error: unknown) => {
        if (isLocalApiOfflineError(error)) {
            return;
        }
        console.error(message, error);
    }, []);

    const mergeAssertions = useCallback((groups: TaskStageAssertion[][]) => {
        const merged = new Map<number, TaskStageAssertion>();
        for (const group of groups) {
            for (const assertion of group) {
                merged.set(assertion.id, assertion);
            }
        }
        return Array.from(merged.values()).sort((a, b) => a.id - b.id);
    }, []);

    const loadRunDiagnostics = useCallback(async (runId?: string | null) => {
        if (!runId) {
            setStageRuns([]);
            setStageAssertions([]);
            setTaskRunArtifacts([]);
            return;
        }
        try {
            const [stages, failedAssertions, recoveryAssertions, artifacts] = await Promise.all([
                fetchTaskRunStages(runId),
                fetchTaskRunAssertions(runId, {
                    failedOnly: true,
                    limit: showDiagnostics ? 250 : 80,
                }),
                fetchTaskRunAssertions(runId, {
                    stageName: "recovery",
                    limit: 40,
                }),
                showDiagnostics
                    ? fetchTaskRunArtifacts(runId, { limit: 250 })
                    : Promise.resolve([] as TaskRunArtifact[]),
            ]);
            setStageRuns(stages);
            setStageAssertions(mergeAssertions([failedAssertions, recoveryAssertions]));
            setTaskRunArtifacts(artifacts);
        } catch (error) {
            logDiagnosticsError("Failed to load stage diagnostics", error);
            setStageRuns([]);
            setStageAssertions([]);
            setTaskRunArtifacts([]);
        }
    }, [logDiagnosticsError, mergeAssertions, showDiagnostics]);

    const loadLockMetrics = useCallback(async () => {
        try {
            const metrics = await fetchLockMetrics();
            setLockMetrics(metrics);
            setLockMetricsError(null);
        } catch (error) {
            logDiagnosticsError("Failed to load lock metrics", error);
            setLockMetrics(null);
            setLockMetricsError("lock metrics unavailable");
        }
    }, [logDiagnosticsError]);

    const loadRuntimeInfo = useCallback(async () => {
        try {
            const info = await fetchRuntimeInfo();
            setRuntimeInfo(info);
            setRuntimeInfoError(null);
        } catch (error) {
            logDiagnosticsError("Failed to load runtime info", error);
            setRuntimeInfo(null);
            setRuntimeInfoError("runtime info unavailable");
        }
    }, [logDiagnosticsError]);

    const loadDodHistory = useCallback(async () => {
        setDodHistoryLoading(true);
        try {
            const runs = await fetchTaskRuns(6);
            const sorted = runs
                .slice()
                .sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
                .slice(0, 5);
            const failureCounter = new Map<string, { count: number; sampleActual: string }>();
            const history = await Promise.all(
                sorted.map(async (run) => {
                    try {
                        const [stages, failedAssertions] = await Promise.all([
                            fetchTaskRunStages(run.run_id),
                            fetchTaskRunAssertions(run.run_id, {
                                failedOnly: true,
                                limit: 50,
                            }),
                        ]);
                        for (const assertion of failedAssertions) {
                            if (assertion.passed) continue;
                            const key = assertion.assertion_key;
                            const prev = failureCounter.get(key);
                            if (prev) {
                                prev.count += 1;
                            } else {
                                failureCounter.set(key, {
                                    count: 1,
                                    sampleActual: assertion.actual,
                                });
                            }
                        }
                        const assertionTotal = stages.reduce(
                            (sum, stage) => sum + (stage.assertion_total ?? 0),
                            0
                        );
                        const assertionFailed = stages.reduce(
                            (sum, stage) => sum + (stage.assertion_failed ?? 0),
                            0
                        );
                        return {
                            runId: run.run_id,
                            createdAt: run.created_at,
                            status: run.status,
                            plannerComplete: run.planner_complete,
                            executionComplete: run.execution_complete,
                            businessComplete: run.business_complete,
                            assertionTotal,
                            assertionFailed,
                        } satisfies DodHistoryItem;
                    } catch {
                        return {
                            runId: run.run_id,
                            createdAt: run.created_at,
                            status: run.status,
                            plannerComplete: run.planner_complete,
                            executionComplete: run.execution_complete,
                            businessComplete: run.business_complete,
                            assertionTotal: 0,
                            assertionFailed: 0,
                        } satisfies DodHistoryItem;
                    }
                })
            );
            setDodHistory(history);
            const topFailures = Array.from(failureCounter.entries())
                .sort((a, b) => b[1].count - a[1].count)
                .slice(0, 3)
                .map(([key, value]) => ({
                    key,
                    count: value.count,
                    sampleActual: value.sampleActual,
                }));
            setDodFailureTop(topFailures);
        } catch (error) {
            logDiagnosticsError("Failed to load DoD history", error);
            setDodHistory([]);
            setDodFailureTop([]);
        } finally {
            setDodHistoryLoading(false);
        }
    }, [logDiagnosticsError]);

    useEffect(() => {
        void loadRuntimeInfo();
    }, [loadRuntimeInfo]);

    useEffect(() => {
        if (!showDiagnostics) return;
        void loadLockMetrics();
    }, [showDiagnostics, loadLockMetrics]);

    useEffect(() => {
        if (!showDiagnostics) return;
        void loadDodHistory();
    }, [showDiagnostics, loadDodHistory]);

    useEffect(() => {
        if (!showDiagnostics || !activeRunId) return;
        void loadRunDiagnostics(activeRunId);
    }, [showDiagnostics, activeRunId, loadRunDiagnostics]);

    return {
        stageRuns,
        stageAssertions,
        taskRunArtifacts,
        setStageRuns,
        setStageAssertions,
        dodHistory,
        dodFailureTop,
        dodHistoryLoading,
        lockMetrics,
        lockMetricsError,
        runtimeInfo,
        runtimeInfoError,
        loadRunDiagnostics,
        loadLockMetrics,
        loadRuntimeInfo,
        loadDodHistory,
    };
}
