import type { ComposerControlRowProps } from "@/features/launcher/components/composerParts/controlParts/types";
import { getAdvancedToggleClass } from "@/features/launcher/components/composerParts/controlParts/styles";

export function ComposerAdvancedToggle({
  ui,
  handlers,
}: Pick<ComposerControlRowProps, "ui" | "handlers">) {
  if (ui.composerMode === "chat") {
    return null;
  }

  return (
    <button
      onClick={handlers.onToggleAdvancedControls}
      disabled={ui.isExecutionLocked}
      className={getAdvancedToggleClass(ui.showAdvancedControls, ui.isExecutionLocked)}
      title="고급 실행 옵션/점검 패널 표시"
    >
      옵션 {ui.showAdvancedControls ? "ON" : "OFF"}
    </button>
  );
}
