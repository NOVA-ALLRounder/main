import type { HudMeta } from "@/features/launcher/support";

export type LauncherStatusBarProps = {
    currentHud: HudMeta;
    runId: string | null;
    runStatus: string | null;
    runPhase: "idle" | "running" | "retrying" | "approval_required" | "manual_required" | "completed" | "failed";
    safeExecutionMode: boolean;
    runtimeInfoError: string | null;
    isDevBundleMismatch: boolean;
    onCopyRunId: (runId: string) => void;
};

export function LauncherStatusBar({
    currentHud,
    runId,
    runStatus,
    runPhase,
    safeExecutionMode,
    runtimeInfoError,
    isDevBundleMismatch,
    onCopyRunId,
}: LauncherStatusBarProps) {
    return (
        <div className="launcher-statusbar px-4 py-2 border-t border-white/10 bg-[#10141b] flex items-center justify-between">
            <div className="flex items-center gap-2">
                <span className={`h-2 w-2 rounded-full ${currentHud.dot}`} />
                <span className={`text-[11px] px-2 py-0.5 rounded-full border ${currentHud.chip}`}>
                    {currentHud.label}
                </span>
                {runId && (
                    <span className="inline-flex items-center gap-1.5 text-[11px] text-gray-500">
                        <span>run: {runId}</span>
                        <button
                            onClick={() => onCopyRunId(runId)}
                            className="px-1.5 py-0.5 rounded border border-white/15 text-gray-300 hover:bg-white/10"
                        >
                            복사
                        </button>
                    </span>
                )}
                <span
                    className={`text-[11px] px-2 py-0.5 rounded-full border ${
                        safeExecutionMode
                            ? "border-sky-400/35 bg-sky-500/15 text-sky-200"
                            : "border-white/20 bg-white/5 text-gray-400"
                    }`}
                >
                    안전 {safeExecutionMode ? "ON" : "OFF"}
                </span>
                {runtimeInfoError && (
                    <span className="text-[11px] px-2 py-0.5 rounded-full border border-rose-400/35 bg-rose-500/15 text-rose-200">
                        runtime info unavailable
                    </span>
                )}
                {isDevBundleMismatch && (
                    <span className="text-[11px] px-2 py-0.5 rounded-full border border-amber-400/35 bg-amber-500/15 text-amber-100">
                        DEV 코드와 실행 코어가 다를 수 있음
                    </span>
                )}
            </div>
            {runStatus && (
                <div className="text-[11px] text-gray-500 text-right">
                    <div>status={runStatus}</div>
                    {(runPhase === "running" || runPhase === "retrying") && (
                        <div className="text-amber-300/90">입력 충돌 감지 시 자동 pause/abort</div>
                    )}
                </div>
            )}
        </div>
    );
}
