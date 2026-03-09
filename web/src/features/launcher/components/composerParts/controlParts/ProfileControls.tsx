import type { ComposerControlRowProps } from "@/features/launcher/components/composerParts/controlParts/types";
import { getPillButtonClass } from "@/features/launcher/components/composerParts/controlParts/styles";
import {
  canApplyRecommendedProfile,
  shouldShowProfileControls,
} from "@/features/launcher/components/composerParts/controlParts/policy";

export function ComposerProfileControls({
  ui,
  profile,
  handlers,
}: ComposerControlRowProps) {
  if (!shouldShowProfileControls(ui.composerMode, ui.showAdvancedControls)) {
    return null;
  }

  return (
    <div className="hidden shrink-0 items-center gap-2 text-[11px] text-gray-300 lg:flex">
      <span className="rounded-full border border-white/15 bg-white/5 px-2 py-1">
        권장 {profile.formatProfileLabel(profile.profileRecommendation.profile)}
      </span>
      {canApplyRecommendedProfile(profile) && (
        <button
          onClick={handlers.onApplyRecommendedProfile}
          disabled={ui.isExecutionLocked}
          className="rounded-full border border-amber-400/35 bg-amber-500/15 px-2 py-1 text-amber-100 hover:bg-amber-500/25 disabled:opacity-50"
          title={profile.profileRecommendation.reason}
        >
          권장 적용
        </button>
      )}
      <button
        onClick={handlers.onToggleAutoApplyRecommendedProfile}
        disabled={ui.isExecutionLocked || profile.safeExecutionMode}
        className={getPillButtonClass(
          profile.autoApplyRecommendedProfile,
          ui.isExecutionLocked || profile.safeExecutionMode,
          "border-emerald-400/35 bg-emerald-500/15 text-emerald-200",
        )}
        title="Strict 차단 상황에서 권장 프로필로 자동 전환"
      >
        자동 전환 {profile.autoApplyRecommendedProfile ? "ON" : "OFF"}
      </button>
      <button
        onClick={handlers.onToggleSafeExecutionMode}
        disabled={ui.isExecutionLocked}
        className={getPillButtonClass(
          profile.safeExecutionMode,
          ui.isExecutionLocked,
          "border-sky-400/35 bg-sky-500/15 text-sky-200",
        )}
        title="안전 모드 ON이면 Strict 프로필로 고정하고 자동 전환을 잠급니다."
      >
        안전 모드 {profile.safeExecutionMode ? "ON" : "OFF"}
      </button>
      <button
        onClick={handlers.onToggleCompactLayoutMode}
        disabled={ui.isExecutionLocked}
        className={getPillButtonClass(
          profile.compactLayoutMode,
          ui.isExecutionLocked,
          "border-cyan-400/35 bg-cyan-500/15 text-cyan-200",
        )}
        title="컴팩트 레이아웃 ON이면 입력/결과 패널 높이를 줄여 집중도를 높입니다."
      >
        컴팩트 {profile.compactLayoutMode ? "ON" : "OFF"}
      </button>
    </div>
  );
}
