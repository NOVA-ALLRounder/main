import type { RecoveryActionsSectionProps } from "@/features/launcher/components/detailSummaryParts/sectionTypes";

export function RecoveryActionsSection({
  recoveryActions,
  loading,
  recoveryActionBusyKey,
  preflightFixBusy,
  artifactOpenBusy,
  onOneClickRecovery,
  onRunRecoveryAction,
}: RecoveryActionsSectionProps) {
  if (recoveryActions.length === 0) return null;

  return (
    <div className="mt-2 rounded border border-white/10 bg-white/5 px-2 py-2">
      <div className="text-[11px] uppercase tracking-wider text-amber-300 font-semibold mb-1">
        복구 액션
      </div>
      <div className="mb-2">
        <button
          onClick={onOneClickRecovery}
          disabled={loading || !!recoveryActionBusyKey}
          className="text-[11px] px-2.5 py-1 rounded-full border border-sky-400/40 bg-sky-500/15 text-sky-100 hover:bg-sky-500/25 disabled:opacity-50"
          title={recoveryActions[0]?.description ?? "권장 우선 복구 실행"}
        >
          {recoveryActionBusyKey
            ? "즉시 복구 실행..."
            : `즉시 복구 실행: ${recoveryActions[0]?.label ?? "권장 액션"}`}
        </button>
      </div>
      <div className="flex flex-wrap gap-1.5">
        {recoveryActions.map((action) => (
          <button
            key={action.key}
            onClick={() => onRunRecoveryAction(action)}
            disabled={
              loading ||
              !!recoveryActionBusyKey ||
              preflightFixBusy === action.fixAction ||
              artifactOpenBusy === action.path
            }
            className="text-[11px] px-2.5 py-1 rounded-full border border-amber-400/35 bg-amber-500/15 text-amber-100 hover:bg-amber-500/25 disabled:opacity-50"
            title={action.description}
          >
            {recoveryActionBusyKey === action.key ? `${action.label}...` : action.label}
          </button>
        ))}
      </div>
    </div>
  );
}
