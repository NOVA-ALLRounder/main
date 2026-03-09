import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
    addExecAllowlist,
    approveExecApproval,
    rejectExecApproval,
    removeExecAllowlist,
    runExecResultsGuard,
} from "@/lib/api";
import { useExecAllowlist, useExecApprovals, useExecResults } from "@/lib/hooks";
import { format } from "date-fns";
import { useState } from "react";

export function ExecControlsCard() {
    const { data: allowlist, refetch: refetchAllowlist } = useExecAllowlist(100);
    const [execStatusFilter, setExecStatusFilter] = useState("all");
    const { data: execResults, refetch: refetchExecResults } = useExecResults(
        60,
        execStatusFilter === "all" ? undefined : execStatusFilter
    );
    const [pattern, setPattern] = useState("");
    const [cwd, setCwd] = useState("");
    const [status, setStatus] = useState<string | null>(null);
    const [guardStatus, setGuardStatus] = useState<string | null>(null);
    const [guardAge, setGuardAge] = useState("300");
    const [expandedExecId, setExpandedExecId] = useState<string | null>(null);
    const { data: approvals, refetch: refetchApprovals } = useExecApprovals("pending");

    const handleAddAllowlist = async () => {
        if (!pattern.trim()) {
            setStatus("Pattern is required.");
            return;
        }
        try {
            await addExecAllowlist(pattern.trim(), cwd.trim() || undefined);
            setPattern("");
            setCwd("");
            setStatus("Allowlist entry added.");
            refetchAllowlist();
        } catch {
            setStatus("Failed to add allowlist entry.");
        }
    };

    const handleRemoveAllowlist = async (id: number) => {
        try {
            await removeExecAllowlist(id);
            setStatus("Allowlist entry removed.");
            refetchAllowlist();
        } catch {
            setStatus("Failed to remove allowlist entry.");
        }
    };

    const handleApproveExec = async (id: string) => {
        try {
            await approveExecApproval(id, "Dashboard User");
            setStatus("Approved execution.");
            refetchApprovals();
        } catch {
            setStatus("Failed to approve.");
        }
    };

    const handleRejectExec = async (id: string) => {
        try {
            await rejectExecApproval(id, "Dashboard User");
            setStatus("Rejected execution.");
            refetchApprovals();
        } catch {
            setStatus("Failed to reject.");
        }
    };

    const handleGuard = async () => {
        const maxAge = Number.isFinite(Number(guardAge)) ? Number(guardAge) : undefined;
        try {
            const res = await runExecResultsGuard(maxAge, 200);
            setGuardStatus(`Guarded ${res.scanned}, timed out ${res.timed_out}`);
            refetchExecResults();
        } catch {
            setGuardStatus("Guard failed.");
        }
    };

    return (
        <Card className="h-auto mb-4">
            <CardHeader>
                <CardTitle>Exec Controls</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
                {approvals && approvals.length > 0 && (
                    <div className="space-y-2 border-b border-white/10 pb-4">
                        <div className="flex items-center justify-between text-xs">
                            <span className="font-semibold text-amber-300">⚠️ Pending Approvals</span>
                            <span className="text-muted-foreground">{approvals.length} request(s)</span>
                        </div>
                        <div className="space-y-2 max-h-32 overflow-y-auto">
                            {approvals.map((approval) => (
                                <div key={approval.id} className="bg-amber-500/10 border border-amber-500/20 p-2 rounded">
                                    <div className="text-[11px] font-mono mb-1 break-all">{approval.command}</div>
                                    <div className="flex gap-2">
                                        <button
                                            onClick={() => handleApproveExec(approval.id)}
                                            className="flex-1 bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-200 text-[10px] py-1 rounded transition-colors"
                                        >
                                            Approve
                                        </button>
                                        <button
                                            onClick={() => handleRejectExec(approval.id)}
                                            className="flex-1 bg-rose-500/20 hover:bg-rose-500/30 text-rose-200 text-[10px] py-1 rounded transition-colors"
                                        >
                                            Reject
                                        </button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </div>
                )}
                <div className="space-y-2">
                    <div className="text-xs text-muted-foreground">Allowlist</div>
                    <div className="flex gap-2">
                        <input
                            value={pattern}
                            onChange={(e) => setPattern(e.target.value)}
                            placeholder="Pattern (e.g. git status)"
                            className="flex-1 rounded-md bg-white/5 border border-white/10 px-2 py-1 text-xs"
                        />
                        <input
                            value={cwd}
                            onChange={(e) => setCwd(e.target.value)}
                            placeholder="CWD (optional)"
                            className="flex-1 rounded-md bg-white/5 border border-white/10 px-2 py-1 text-xs"
                        />
                    </div>
                    <button
                        onClick={handleAddAllowlist}
                        className="w-full text-xs py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors"
                    >
                        Add allowlist entry
                    </button>
                    {status && <div className="text-[11px] text-muted-foreground">{status}</div>}
                    <div className="space-y-1 max-h-24 overflow-y-auto text-[11px]">
                        {allowlist && allowlist.length > 0 ? (
                            allowlist.slice(0, 10).map((item) => (
                                <div key={item.id} className="flex items-center justify-between gap-2 bg-white/5 px-2 py-1 rounded">
                                    <div className="truncate">
                                        {item.pattern}
                                        {item.cwd ? ` · ${item.cwd}` : ""}
                                    </div>
                                    <button
                                        onClick={() => handleRemoveAllowlist(item.id)}
                                        className="text-red-200 hover:text-red-100"
                                    >
                                        Remove
                                    </button>
                                </div>
                            ))
                        ) : (
                            <div className="text-muted-foreground">No allowlist entries.</div>
                        )}
                    </div>
                </div>

                <div className="space-y-2">
                    <div className="text-xs text-muted-foreground">Exec Results</div>
                    <div className="flex gap-2">
                        <select
                            value={execStatusFilter}
                            onChange={(e) => setExecStatusFilter(e.target.value)}
                            className="flex-1 rounded-md bg-white/5 border border-white/10 px-2 py-1 text-xs"
                        >
                            <option value="all">All status</option>
                            <option value="pending">pending</option>
                            <option value="success">success</option>
                            <option value="error">error</option>
                            <option value="timeout">timeout</option>
                        </select>
                        <input
                            value={guardAge}
                            onChange={(e) => setGuardAge(e.target.value)}
                            placeholder="Max age secs (300)"
                            className="flex-1 rounded-md bg-white/5 border border-white/10 px-2 py-1 text-xs"
                        />
                        <button
                            onClick={handleGuard}
                            className="text-xs px-3 py-1 rounded bg-white/10 hover:bg-white/20 transition-colors"
                        >
                            Run guard
                        </button>
                    </div>
                    {guardStatus && <div className="text-[11px] text-muted-foreground">{guardStatus}</div>}
                    <div className="space-y-1 max-h-24 overflow-y-auto text-[11px]">
                        {execResults && execResults.length > 0 ? (
                            execResults.slice(0, 8).map((item) => (
                                <div key={item.id} className="bg-white/5 px-2 py-1 rounded">
                                    <div className="flex items-center justify-between gap-2">
                                        <div className="truncate">
                                            {item.command} · {item.status}
                                        </div>
                                        <span className="text-[10px] text-muted-foreground">
                                            {item.updated_at ? format(new Date(item.updated_at), "HH:mm:ss") : "—"}
                                        </span>
                                    </div>
                                    {(item.output || item.error) && (
                                        <button
                                            onClick={() => setExpandedExecId((prev) => (prev === item.id ? null : item.id))}
                                            className="text-[10px] text-indigo-200 hover:text-indigo-100 mt-1"
                                        >
                                            {expandedExecId === item.id ? "Hide details" : "View details"}
                                        </button>
                                    )}
                                    {expandedExecId === item.id && (
                                        <pre className="mt-1 max-h-24 overflow-auto rounded-md bg-black/40 p-2 text-[10px] text-white/80 whitespace-pre-wrap">
                                            {item.output || item.error}
                                        </pre>
                                    )}
                                </div>
                            ))
                        ) : (
                            <div className="text-muted-foreground">No exec results.</div>
                        )}
                    </div>
                </div>
            </CardContent>
        </Card>
    );
}
