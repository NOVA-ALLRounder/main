import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { VerificationRun } from "@/lib/types";
import { formatDashboardLongTime } from "@/features/dashboard/formatters";
import { useState } from "react";

function formatRunDetails(details: string): string {
    try {
        const parsed = JSON.parse(details);
        return JSON.stringify(parsed, null, 2);
    } catch {
        return details;
    }
}

type Props = {
    verificationRuns: VerificationRun[];
};

export function VerificationTimelineCard({ verificationRuns }: Props) {
    const [expandedRunId, setExpandedRunId] = useState<number | null>(null);
    const [runKindFilter, setRunKindFilter] = useState("all");
    const [runStatusFilter, setRunStatusFilter] = useState<"all" | "ok" | "fail">("all");

    const runKinds = Array.from(new Set((verificationRuns ?? []).map((run) => run.kind)));
    const filteredRuns = (verificationRuns ?? [])
        .filter((run) => (runKindFilter === "all" ? true : run.kind === runKindFilter))
        .filter((run) => {
            if (runStatusFilter === "all") return true;
            return runStatusFilter === "ok" ? run.ok : !run.ok;
        });

    return (
        <Card className="h-full mt-4">
            <CardHeader className="space-y-2">
                <CardTitle>Verification Timeline</CardTitle>
                <div className="flex flex-wrap gap-2 text-[11px]">
                    <select
                        value={runKindFilter}
                        onChange={(e) => setRunKindFilter(e.target.value)}
                        className="rounded-md bg-white/5 border border-white/10 px-2 py-1 text-[11px]"
                    >
                        <option value="all">All types</option>
                        {runKinds.map((kind) => (
                            <option key={kind} value={kind}>
                                {kind}
                            </option>
                        ))}
                    </select>
                    <select
                        value={runStatusFilter}
                        onChange={(e) => setRunStatusFilter(e.target.value as "all" | "ok" | "fail")}
                        className="rounded-md bg-white/5 border border-white/10 px-2 py-1 text-[11px]"
                    >
                        <option value="all">All status</option>
                        <option value="ok">OK only</option>
                        <option value="fail">Fail only</option>
                    </select>
                    <span className="text-muted-foreground px-2 py-1">{filteredRuns.length} runs</span>
                </div>
            </CardHeader>
            <CardContent>
                <div className="space-y-3 max-h-44 overflow-y-auto pr-1">
                    {filteredRuns.length > 0 ? (
                        filteredRuns.slice(0, 8).map((run) => (
                            <div
                                key={run.id}
                                className="flex items-start justify-between gap-3 border-b border-white/5 pb-2 last:border-0 text-xs"
                            >
                                <div className="min-w-0 space-y-1">
                                    <div className="font-semibold text-white/90">{run.kind}</div>
                                    <div className="text-muted-foreground line-clamp-2">{run.summary}</div>
                                    <div className="text-[10px] text-muted-foreground">
                                        {formatDashboardLongTime(run.created_at)}
                                    </div>
                                    {run.details && (
                                        <button
                                            onClick={() => setExpandedRunId((prev) => (prev === run.id ? null : run.id))}
                                            className="text-[10px] text-indigo-200 hover:text-indigo-100"
                                        >
                                            {expandedRunId === run.id ? "Hide details" : "View details"}
                                        </button>
                                    )}
                                    {expandedRunId === run.id && run.details && (
                                        <pre className="mt-2 max-h-32 overflow-auto rounded-md bg-black/40 p-2 text-[10px] text-white/80 whitespace-pre-wrap">
                                            {formatRunDetails(run.details)}
                                        </pre>
                                    )}
                                </div>
                                <div
                                    className={`shrink-0 text-[10px] px-2 py-0.5 rounded-full ${
                                        run.ok ? "bg-emerald-500/20 text-emerald-200" : "bg-rose-500/20 text-rose-200"
                                    }`}
                                >
                                    {run.ok ? "OK" : "FAIL"}
                                </div>
                            </div>
                        ))
                    ) : (
                        <p className="text-sm text-muted-foreground">No verification runs yet.</p>
                    )}
                </div>
            </CardContent>
        </Card>
    );
}
