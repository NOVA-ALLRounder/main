import type { ComposerControlRowProps } from "@/features/launcher/components/composerParts/controlParts/types";
import { getProfileOptionClass } from "@/features/launcher/components/composerParts/controlParts/styles";
import {
  isProfileOptionDisabled,
  shouldShowProfileControls,
} from "@/features/launcher/components/composerParts/controlParts/policy";

export function ComposerProfileOptions({
  ui,
  profile,
  handlers,
}: ComposerControlRowProps) {
  if (!shouldShowProfileControls(ui.composerMode, ui.showAdvancedControls)) {
    return null;
  }

  return (
    <div className="inline-flex shrink-0 items-center gap-1 rounded-xl bg-white/6 p-1">
      {profile.executionProfileOptions.map((option) => {
        const disabled = isProfileOptionDisabled(ui, profile, option);
        return (
          <button
            key={option.value}
            onClick={() => handlers.onExecutionProfileSelect(option.value)}
            disabled={disabled}
            title={`${option.label}: ${option.hint}`}
            className={getProfileOptionClass(
              profile.executionProfile === option.value,
              disabled,
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
