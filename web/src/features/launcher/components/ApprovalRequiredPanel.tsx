export type ApprovalRequiredPanelProps = {
    action: string;
    riskLevel: string;
    policy: string;
    message: string;
    approvalBusy: boolean;
    onDecision: (decision: "allow_once" | "allow_always" | "deny") => void;
};

export function ApprovalRequiredPanel({
    action,
    riskLevel,
    policy,
    message,
    approvalBusy,
    onDecision,
}: ApprovalRequiredPanelProps) {
    return (
        <div className="px-4 py-3 border-b border-white/5 bg-[#1b1b1b]">
            <div className="text-[11px] uppercase tracking-wider text-amber-400 font-semibold">
                Approval Required
            </div>
            <div className="text-sm text-gray-200 mt-1">
                Action: <span className="font-mono">{action}</span>
            </div>
            <div className="text-xs text-gray-400">
                Risk: {riskLevel} · Policy: {policy}
            </div>
            <div className="text-xs text-gray-500 mt-1">
                {message}
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
                <button
                    disabled={approvalBusy}
                    onClick={() => onDecision("allow_once")}
                    className="text-xs px-3 py-1.5 rounded bg-emerald-500/20 text-emerald-200 border border-emerald-500/40 hover:bg-emerald-500/30 disabled:opacity-50"
                >
                    Approve once
                </button>
                <button
                    disabled={approvalBusy}
                    onClick={() => onDecision("allow_always")}
                    className="text-xs px-3 py-1.5 rounded bg-blue-500/20 text-blue-200 border border-blue-500/40 hover:bg-blue-500/30 disabled:opacity-50"
                >
                    Allow always
                </button>
                <button
                    disabled={approvalBusy}
                    onClick={() => onDecision("deny")}
                    className="text-xs px-3 py-1.5 rounded bg-rose-500/20 text-rose-200 border border-rose-500/40 hover:bg-rose-500/30 disabled:opacity-50"
                >
                    Deny
                </button>
            </div>
        </div>
    );
}
