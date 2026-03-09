import type { ComposerControlRowProps } from "@/features/launcher/components/composerParts/controlParts/types";
import { getModeButtonClass } from "@/features/launcher/components/composerParts/controlParts/styles";
import type { ComposerMode } from "@/features/launcher/support";

const MODES: Array<{ value: ComposerMode; label: string }> = [
  { value: "nl", label: "자연어" },
  { value: "chat", label: "대화" },
  { value: "program", label: "프로그램" },
];

export function ComposerModeSwitch({
  ui,
  handlers,
}: Pick<ComposerControlRowProps, "ui" | "handlers">) {
  return (
    <div className="launcher-mode-switch inline-flex shrink-0 items-center gap-1 rounded-xl bg-white/6 p-1">
      {MODES.map((mode) => (
        <button
          key={mode.value}
          onClick={() => handlers.onModeSelect(mode.value)}
          disabled={ui.isExecutionLocked}
          className={getModeButtonClass(ui.composerMode === mode.value, ui.isExecutionLocked)}
        >
          {mode.label}
        </button>
      ))}
    </div>
  );
}
