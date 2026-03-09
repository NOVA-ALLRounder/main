import type { LauncherComposerPanelProps } from "@/features/launcher/components/composerParts/panelTypes";

export type ComposerControlRowProps = Pick<
  LauncherComposerPanelProps,
  "ui" | "profile" | "handlers"
>;
