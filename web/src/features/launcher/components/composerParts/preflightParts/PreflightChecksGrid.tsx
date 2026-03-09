import type { ComposerPreflightSectionProps } from "@/features/launcher/components/composerParts/preflightParts/types";
import { hasQuickFixActions } from "@/features/launcher/components/composerParts/preflightParts/support";
import { PreflightQuickFixSection } from "@/features/launcher/components/composerParts/preflightParts/PreflightQuickFixSection";

export function PreflightChecksGrid({
  ui,
  preflight,
  handlers,
}: Pick<ComposerPreflightSectionProps, "ui" | "preflight" | "handlers">) {
  if (!preflight.showDetail) {
    return null;
  }

  return (
    <div className="mt-2 grid grid-cols-1 gap-2 md:grid-cols-3">
      {hasQuickFixActions(preflight) && (
        <PreflightQuickFixSection ui={ui} preflight={preflight} handlers={handlers} />
      )}
      {preflight.checks.map((check) => (
        <div
          key={check.key}
          className={`rounded border px-2 py-1.5 text-[11px] ${
            check.ok
              ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
              : "border-rose-400/30 bg-rose-500/10 text-rose-200"
          }`}
        >
          <div className="font-semibold">
            {check.ok ? "✅" : "❌"} {check.label}
          </div>
          <div className="mt-0.5 opacity-80">{check.message}</div>
          {(check.expected || check.actual) && (
            <div className="mt-0.5 opacity-70">
              expected={check.expected ?? "-"} / actual={check.actual ?? "-"}
            </div>
          )}
        </div>
      ))}
      {preflight.error && (
        <div className="rounded border border-rose-400/30 bg-rose-500/10 px-2 py-1.5 text-[11px] text-rose-200 md:col-span-3">
          API error: {preflight.error}
        </div>
      )}
      {preflight.focusBlocked && (
        <div className="rounded border border-amber-400/30 bg-amber-500/10 px-2 py-1.5 text-[11px] text-amber-200 md:col-span-3">
          권장: 실행 중 전면 앱 충돌을 피하려면 전용 데스크톱/사용자 세션에서 실행하세요.
        </div>
      )}
    </div>
  );
}
