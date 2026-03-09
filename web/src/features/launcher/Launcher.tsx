import { motion, AnimatePresence } from "framer-motion";
import { type Components } from "react-markdown";
import { ApprovalRequiredPanel } from "@/features/launcher/components/ApprovalRequiredPanel";
import { ManualStepNeededPanel } from "@/features/launcher/components/ManualStepNeededPanel";
import { ResultsFeedPanel } from "@/features/launcher/components/ResultsFeedPanel";
import { DiagnosticsPanel } from "@/features/launcher/components/DiagnosticsPanel";
import { DetailSummaryPanel } from "@/features/launcher/components/DetailSummaryPanel";
import { LauncherStatusBar } from "@/features/launcher/components/LauncherStatusBar";
import { LauncherComposerPanel } from "@/features/launcher/components/LauncherComposerPanel";
import { useLauncherViewModel } from "@/features/launcher/hooks/useLauncherViewModel";

const markdownComponents: Components = {
    code({ children, ...props }) {
        const inline = 'inline' in props && props.inline;
        return !inline ? (
            <div className="bg-black/50 p-2 rounded-md my-2 overflow-x-auto font-mono text-xs border border-white/10">
                <code {...props}>{children}</code>
            </div>
        ) : (
            <code className="bg-white/10 px-1 py-0.5 rounded font-mono text-xs" {...props}>
                {children}
            </code>
        );
    },
};

export default function Launcher() {
    const {
        rootProps,
        detailPanelVisible,
        composerPanelProps,
        statusBarProps,
        detailSummaryPanelProps,
        diagnosticsPanelProps,
        approvalPanelProps,
        manualStepPanelProps,
        resultsFeedPanelProps,
    } = useLauncherViewModel();

    return (
        <div
            className="launcher-root w-full h-full bg-[radial-gradient(120%_90%_at_50%_0%,rgba(18,44,89,0.45),rgba(9,13,22,0.96)_62%,rgba(7,10,17,0.98)_100%)] flex items-end justify-center pb-2 sm:pb-3 px-2 sm:px-3 pointer-events-none"
            onMouseDown={rootProps.onMouseDown}
        >
            <motion.div
                className={`launcher-card pointer-events-auto w-full ${rootProps.launcherWidthClass} max-h-[calc(100vh-6px)] bg-[#121722]/96 backdrop-blur-2xl rounded-[18px] shadow-2xl overflow-hidden border transition-colors duration-500
                    ${rootProps.successPulse ? 'border-green-500/50 shadow-green-500/20' : 'border-white/10 ring-1 ring-black/5'}
                `}
                initial={{ scale: 0.9, opacity: 0 }}
                animate={{
                    scale: 1,
                    opacity: 1,
                    x: rootProps.shake ? [0, -10, 10, -10, 10, 0] : 0
                }}
                transition={{ type: "spring", duration: 0.3 }}
            >
                <LauncherComposerPanel {...composerPanelProps} />

                <LauncherStatusBar {...statusBarProps} />

                <AnimatePresence>
                    {detailPanelVisible && (
                        <motion.div
                            initial={{ opacity: 0, height: 0 }}
                            animate={{ opacity: 1, height: "auto" }}
                            exit={{ opacity: 0, height: 0 }}
                            className="launcher-detail-panel border-t border-white/10 bg-[#121212]/96"
                        >
                            <DetailSummaryPanel {...detailSummaryPanelProps} />

                            <DiagnosticsPanel {...diagnosticsPanelProps} />

                            {approvalPanelProps && (
                                <ApprovalRequiredPanel {...approvalPanelProps} />
                            )}

                            {manualStepPanelProps && (
                                <ManualStepNeededPanel {...manualStepPanelProps} />
                            )}

                            <ResultsFeedPanel
                                {...resultsFeedPanelProps}
                                markdownComponents={markdownComponents}
                            />
                        </motion.div>
                    )}
                </AnimatePresence>
            </motion.div>
        </div>
    );
}
