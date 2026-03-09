import type { ComposerPreflightSectionProps } from "@/features/launcher/components/composerParts/preflightParts/types";
import {
  getPreflightStatusClass,
  getPreflightStatusLabel,
} from "@/features/launcher/components/composerParts/preflightParts/support";

export function PreflightHeader({
  ui,
  profile,
  preflight,
  handlers,
}: ComposerPreflightSectionProps) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="text-xs text-gray-200">
        실행 전 점검:{" "}
        <span className={getPreflightStatusClass(preflight)}>
          {getPreflightStatusLabel(preflight)}
        </span>
        {preflight.activeApp && (
          <span className="ml-2 text-gray-400">front={preflight.activeApp}</span>
        )}
        {preflight.checkedAt && (
          <span className="ml-2 text-gray-500">
            {new Date(preflight.checkedAt).toLocaleTimeString()}
          </span>
        )}
        <div className="mt-1 text-[11px] text-gray-400">
          프로필 권장: {profile.formatProfileLabel(profile.profileRecommendation.profile)} ·{" "}
          {profile.profileRecommendation.reason}
        </div>
      </div>
      <div className="flex items-center gap-2">
        <button
          onClick={handlers.onRunPreflightCheck}
          disabled={preflight.loading || ui.isExecutionLocked}
          className="rounded-full border border-white/15 bg-white/5 px-2.5 py-1 text-[11px] hover:bg-white/10 disabled:opacity-50"
        >
          다시 점검
        </button>
        <button
          onClick={handlers.onTogglePreflightDetail}
          className="rounded-full border border-white/15 bg-white/5 px-2.5 py-1 text-[11px] hover:bg-white/10"
        >
          {preflight.showDetail ? "숨기기" : "상세"}
        </button>
      </div>
    </div>
  );
}
