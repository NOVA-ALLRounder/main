export type ManualChecklistState = {
    focusReady: boolean;
    manualStepDone: boolean;
    handsOffReady: boolean;
};

export type ManualStepNeededPanelProps = {
    manualChecklist: ManualChecklistState;
    loading: boolean;
    artifactOpenBusy: string | null;
    firstFailedArtifactPath: string | null;
    onChecklistChange: (key: keyof ManualChecklistState, checked: boolean) => void;
    onResume: () => void;
    onGuidedRecovery: () => void;
};

export function ManualStepNeededPanel({
    manualChecklist,
    loading,
    artifactOpenBusy,
    firstFailedArtifactPath,
    onChecklistChange,
    onResume,
    onGuidedRecovery,
}: ManualStepNeededPanelProps) {
    return (
        <div className="px-4 py-3 border-b border-white/5 bg-[#171717]">
            <div className="text-[11px] uppercase tracking-wider text-sky-400 font-semibold">
                Manual Step Needed
            </div>
            <div className="text-xs text-gray-400 mt-1">
                브라우저에서 수동 작업을 완료한 뒤 Resume을 눌러 다음 단계로 진행하세요.
            </div>
            <div className="mt-3 space-y-2 text-xs text-gray-300">
                <label className="flex items-center gap-2">
                    <input
                        type="checkbox"
                        checked={manualChecklist.focusReady}
                        onChange={(e) => onChecklistChange("focusReady", e.target.checked)}
                    />
                    전면 앱을 작업 대상(브라우저/필요 앱)으로 복구함
                </label>
                <label className="flex items-center gap-2">
                    <input
                        type="checkbox"
                        checked={manualChecklist.manualStepDone}
                        onChange={(e) => onChecklistChange("manualStepDone", e.target.checked)}
                    />
                    수동 단계 입력/선택을 완료함
                </label>
                <label className="flex items-center gap-2">
                    <input
                        type="checkbox"
                        checked={manualChecklist.handsOffReady}
                        onChange={(e) => onChecklistChange("handsOffReady", e.target.checked)}
                    />
                    Resume 이후 키보드/마우스 간섭 없이 대기 가능
                </label>
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
                <button
                    disabled={
                        loading ||
                        !manualChecklist.focusReady ||
                        !manualChecklist.manualStepDone ||
                        !manualChecklist.handsOffReady
                    }
                    onClick={onResume}
                    className="text-xs px-3 py-1.5 rounded bg-sky-500/20 text-sky-200 border border-sky-500/40 hover:bg-sky-500/30 disabled:opacity-50"
                >
                    Resume
                </button>
                {firstFailedArtifactPath && (
                    <button
                        disabled={loading || artifactOpenBusy === firstFailedArtifactPath}
                        onClick={onGuidedRecovery}
                        className="text-xs px-3 py-1.5 rounded bg-amber-500/20 text-amber-200 border border-amber-500/40 hover:bg-amber-500/30 disabled:opacity-50"
                        title={firstFailedArtifactPath}
                    >
                        증거 열고 Resume
                    </button>
                )}
            </div>
        </div>
    );
}
