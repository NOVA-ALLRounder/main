import type { ComposerPreflightSectionProps } from "@/features/launcher/components/composerParts/preflightParts/types";

export function getPreflightPanelClass(
  preflight: ComposerPreflightSectionProps["preflight"],
) {
  return preflight.ok === false || preflight.error
    ? "border-rose-500/35 bg-rose-500/10"
    : "border-white/10 bg-[#1b1b1b]/80";
}

export function getPreflightStatusClass(
  preflight: ComposerPreflightSectionProps["preflight"],
) {
  if (preflight.ok) return "text-emerald-300";
  if (preflight.ok === false) return "text-rose-300";
  return "text-gray-400";
}

export function getPreflightStatusLabel(
  preflight: ComposerPreflightSectionProps["preflight"],
) {
  if (preflight.loading) return "점검 중...";
  return preflight.ok ? "준비됨" : "차단됨";
}

export function hasQuickFixActions(
  preflight: ComposerPreflightSectionProps["preflight"],
) {
  return (
    preflight.focusBlocked ||
    (preflight.accessibility && !preflight.accessibility.ok) ||
    (preflight.screenCapture && !preflight.screenCapture.ok)
  );
}
