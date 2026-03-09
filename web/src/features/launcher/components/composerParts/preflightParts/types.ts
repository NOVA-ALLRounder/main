import type { LauncherComposerPanelProps } from "@/features/launcher/components/composerParts/panelTypes";

export type ComposerPreflightSectionProps = Pick<
  LauncherComposerPanelProps,
  "ui" | "profile" | "preflight" | "handlers"
>;
