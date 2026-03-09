import { useCallback, useState } from "react";

import {
    IN_PROGRESS_RUN_STATUSES,
    type ExecutionSnapshot,
    type RunPhase,
} from "@/features/launcher/support";

export function useLauncherExecutionState() {
    const [lastPlanId, setLastPlanId] = useState<string | null>(null);
    const [lastStatus, setLastStatus] = useState<string | null>(null);
    const [runPhase, setRunPhase] = useState<RunPhase>("idle");
    const [runSnapshot, setRunSnapshot] = useState<ExecutionSnapshot | null>(null);

    const updateExecutionState = useCallback((snapshot: ExecutionSnapshot) => {
        setRunSnapshot(snapshot);
        const statusLower = snapshot.status.toLowerCase();
        if (statusLower === "approval_required") {
            setRunPhase("approval_required");
            return;
        }
        if (statusLower === "manual_required") {
            setRunPhase("manual_required");
            return;
        }
        if (IN_PROGRESS_RUN_STATUSES.has(statusLower)) {
            setRunPhase("running");
            return;
        }
        if (["failed", "error", "blocked"].includes(statusLower)) {
            setRunPhase("failed");
            return;
        }
        if (statusLower === "business_completed") {
            setRunPhase("completed");
            return;
        }
        if ((statusLower === "completed" || statusLower === "success") && snapshot.verifyOk) {
            setRunPhase("completed");
            return;
        }
        if (snapshot.businessComplete && snapshot.verifyOk) {
            setRunPhase("completed");
            return;
        }
        setRunPhase("failed");
    }, []);

    return {
        lastPlanId,
        setLastPlanId,
        lastStatus,
        setLastStatus,
        runPhase,
        setRunPhase,
        runSnapshot,
        setRunSnapshot,
        updateExecutionState,
    };
}
