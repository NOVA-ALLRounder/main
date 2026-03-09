import type { LauncherComposerPanelProps } from "@/features/launcher/components/composerParts/panelTypes";

export type ComposerInputSectionProps = Pick<
  LauncherComposerPanelProps,
  "refs" | "ui" | "handlers"
>;

export type ComposerQuickStripSectionProps = Pick<
  LauncherComposerPanelProps,
  "ui" | "quickActions" | "handlers"
>;

export type ComposerToolbarSectionProps = Pick<
  LauncherComposerPanelProps,
  "ui" | "runtime" | "handlers"
>;
