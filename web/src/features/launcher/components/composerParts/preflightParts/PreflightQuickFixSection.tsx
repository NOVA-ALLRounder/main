import type { ComposerPreflightSectionProps } from "@/features/launcher/components/composerParts/preflightParts/types";

type FixAction = {
  key: string;
  label: string;
};

function buildQuickFixActions(
  preflight: ComposerPreflightSectionProps["preflight"],
): FixAction[] {
  const actions: FixAction[] = [];

  if (preflight.focusBlocked) {
    actions.push(
      { key: "prepare_isolated_mode", label: "격리 모드 준비" },
      { key: "activate_finder", label: "Finder 전면 복구" },
    );
  }

  if (preflight.accessibility && !preflight.accessibility.ok) {
    actions.push({ key: "open_accessibility_settings", label: "접근성 설정 열기" });
  }

  if (preflight.screenCapture && !preflight.screenCapture.ok) {
    actions.push(
      { key: "open_screen_capture_settings", label: "화면 기록 설정 열기" },
      { key: "reveal_core_binary", label: "코어 파일 보기" },
      { key: "request_screen_capture_access", label: "권한 요청" },
    );
  }

  return actions;
}

export function PreflightQuickFixSection({
  ui,
  preflight,
  handlers,
}: Pick<ComposerPreflightSectionProps, "ui" | "preflight" | "handlers">) {
  const actions = buildQuickFixActions(preflight);

  if (actions.length === 0) {
    return null;
  }

  return (
    <div className="rounded border border-amber-400/30 bg-amber-500/10 px-2 py-2 text-[11px] text-amber-200 md:col-span-3">
      <div className="mb-1 font-semibold">빠른 복구</div>
      <div className="flex flex-wrap gap-1.5">
        {actions.map((action) => (
          <button
            key={action.key}
            onClick={() => handlers.onHandlePreflightFix(action.key)}
            disabled={!!preflight.fixBusy || preflight.loading || ui.isExecutionLocked}
            className="rounded border border-amber-300/40 bg-amber-400/20 px-2 py-1 hover:bg-amber-400/30 disabled:opacity-50"
          >
            {preflight.fixBusy === action.key ? "처리 중..." : action.label}
          </button>
        ))}
      </div>
      {preflight.fixMessage && (
        <div className="mt-1 text-amber-100/90">{preflight.fixMessage}</div>
      )}
    </div>
  );
}
