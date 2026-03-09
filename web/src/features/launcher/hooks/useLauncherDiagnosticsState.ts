import { useCallback, useEffect, useState } from "react";
import {
    fetchLockMetrics,
    fetchRuntimeInfo,
    fetchTaskRunArtifacts,
    fetchTaskRunAssertions,
    fetchTaskRunStages,
    fetchTaskRuns,
} from "@/lib/api";
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
};

export function useLauncherDiagnosticsState({
    showDiagnostics,
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

    const loadRunDiagnostics = useCallback(async (runId?: string | null) => {
        if (!runId) {
            setStageRuns([]);
            setStageAssertions([]);
            setTaskRunArtifacts([]);
            return;
        }
        try {
            const [stages, assertions, artifacts] = await Promise.all([
                fetchTaskRunStages(runId),
                fetchTaskRunAssertions(runId),
                fetchTaskRunArtifacts(runId),
            ]);
            setStageRuns(stages);
            setStageAssertions(assertions);
            setTaskRunArtifacts(artifacts);
        } catch (error) {
            console.error("Failed to load stage diagnostics", error);
            setStageRuns([]);
            setStageAssertions([]);
            setTaskRunArtifacts([]);
        }
    }, []);

    const loadLockMetrics = useCallback(async () => {
        try {
            const metrics = await fetchLockMetrics();
            setLockMetrics(metrics);
            setLockMetricsError(null);
        } catch (error) {
            console.error("Failed to load lock metrics", error);
            setLockMetrics(null);
            setLockMetricsError("lock metrics unavailable");
        }
    }, []);

    const loadRuntimeInfo = useCallback(async () => {
        try {
            const info = await fetchRuntimeInfo();
            setRuntimeInfo(info);
            setRuntimeInfoError(null);
        } catch (error) {
            console.error("Failed to load runtime info", error);
            setRuntimeInfo(null);
            setRuntimeInfoError("runtime info unavailable");
        }
    }, []);

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
                        const assertions = await fetchTaskRunAssertions(run.run_id);
                        for (const assertion of assertions) {
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
                        return {
                            runId: run.run_id,
                            createdAt: run.created_at,
                            status: run.status,
                            plannerComplete: run.planner_complete,
                            executionComplete: run.execution_complete,
                            businessComplete: run.business_complete,
                            assertionTotal: assertions.length,
                            assertionFailed: assertions.filter((a) => !a.passed).length,
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
            console.error("Failed to load DoD history", error);
            setDodHistory([]);
            setDodFailureTop([]);
        } finally {
            setDodHistoryLoading(false);
        }
    }, []);

    useEffect(() => {
        void loadRuntimeInfo();
    }, [loadRuntimeInfo]);

    useEffect(() => {
        if (!showDiagnostics) return;
        void loadLockMetrics();
        void loadRuntimeInfo();
    }, [showDiagnostics, loadLockMetrics, loadRuntimeInfo]);

    useEffect(() => {
        void loadDodHistory();
    }, [loadDodHistory]);

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
