import type { ComposerPreflightSectionProps } from "@/features/launcher/components/composerParts/preflightParts/types";

export function ComposerExecutionLockHint({
  ui,
}: Pick<ComposerPreflightSectionProps, "ui">) {
  if (ui.composerMode === "chat" || !ui.executionLockHint) {
    return null;
  }

  return (
    <div className="mt-2 rounded-lg border border-amber-400/30 bg-amber-500/10 px-3 py-2 text-[11px] text-amber-100">
      실행 차단 사유: {ui.executionLockHint}
    </div>
  );
}
