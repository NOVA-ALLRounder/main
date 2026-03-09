import type { DiagnosticsPanelProps } from "@/features/launcher/components/diagnosticsParts/panelTypes";

export type DiagnosticsOverviewSectionProps = Pick<
  DiagnosticsPanelProps,
  "telemetry" | "history" | "execution" | "handlers"
>;

export type StageDodSectionProps = Pick<DiagnosticsPanelProps, "history">;

export type PersistedStageTraceSectionProps = Pick<
  DiagnosticsPanelProps,
  "execution" | "stage" | "artifacts" | "handlers"
>;
