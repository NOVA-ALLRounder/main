import type { ComposerMode, ExecutionProfileOption } from "@/features/launcher/support";
import type { ComposerControlRowProps } from "@/features/launcher/components/composerParts/controlParts/types";

export function isProfileOptionDisabled(
  ui: ComposerControlRowProps["ui"],
  profile: ComposerControlRowProps["profile"],
  option: ExecutionProfileOption,
) {
  return ui.isExecutionLocked || (profile.safeExecutionMode && option.value !== "strict");
}

export function shouldShowProfileControls(
  composerMode: ComposerMode,
  showAdvancedControls: boolean,
) {
  return composerMode !== "chat" && showAdvancedControls;
}

export function canApplyRecommendedProfile(
  profile: ComposerControlRowProps["profile"],
) {
  return profile.executionProfile !== profile.profileRecommendation.profile;
}
