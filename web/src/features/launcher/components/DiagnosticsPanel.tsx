import {
    type DiagnosticsPanelProps,
    DiagnosticsOverviewSection,
    PersistedStageTraceSection,
    StageDodSection,
} from "@/features/launcher/components/DiagnosticsPanelSections";

export type { DiagnosticsPanelProps } from "@/features/launcher/components/DiagnosticsPanelSections";

export function DiagnosticsPanel(props: DiagnosticsPanelProps) {
    if (!props.visibility.showDiagnostics) return null;

    return (
        <>
            <DiagnosticsOverviewSection
                telemetry={props.telemetry}
                history={props.history}
                execution={props.execution}
                handlers={props.handlers}
            />
            <StageDodSection history={props.history} />
            <PersistedStageTraceSection
                execution={props.execution}
                stage={props.stage}
                artifacts={props.artifacts}
                handlers={props.handlers}
            />
        </>
    );
}
