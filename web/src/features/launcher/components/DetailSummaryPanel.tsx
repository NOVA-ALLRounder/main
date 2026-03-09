import {
    type DetailSummaryPanelProps,
    DetailSummaryFooter,
    RecoveryActionsSection,
    SummaryCardsSection,
} from "@/features/launcher/components/DetailSummarySections";

export type { DetailSummaryPanelProps } from "@/features/launcher/components/DetailSummarySections";

export function DetailSummaryPanel(props: DetailSummaryPanelProps) {
    return (
        <div className="px-4 py-3 border-b border-white/5 bg-[#171717]">
            <div className="text-[11px] uppercase tracking-wider text-indigo-300 font-semibold mb-2">
                핵심 요약
            </div>
            <SummaryCardsSection
                runPhase={props.runPhase}
                runStatus={props.runStatus}
                runScore={props.runScore}
                nextActionHint={props.nextActionHint}
            />
            <RecoveryActionsSection
                recoveryActions={props.recoveryActions}
                loading={props.loading}
                recoveryActionBusyKey={props.recoveryActionBusyKey}
                preflightFixBusy={props.preflightFixBusy}
                artifactOpenBusy={props.artifactOpenBusy}
                onOneClickRecovery={props.onOneClickRecovery}
                onRunRecoveryAction={props.onRunRecoveryAction}
            />
            <DetailSummaryFooter
                dodHistoryLoading={props.dodHistoryLoading}
                showDiagnostics={props.showDiagnostics}
                onLoadDodHistory={props.onLoadDodHistory}
                onToggleDiagnostics={props.onToggleDiagnostics}
            />
        </div>
    );
}
