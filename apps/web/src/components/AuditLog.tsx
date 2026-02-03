import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { CheckCircle2, AlertTriangle, XCircle, ShieldCheck } from "lucide-react";
import { useLogs } from "@/lib/hooks";
import { format } from "date-fns";
import type { LogEntry } from "@/lib/types"; // Fixed type import

export function AuditLog() {
    const { data: logs } = useLogs();

    // Filter relevant logs for Audit (Workflow/Recovery/Privacy)
    // In a real app, API should support `?type=audit`
    const auditLogs = logs?.filter(log =>
        log.message.includes("Workflow") ||
        log.message.includes("Recovery") ||
        log.message.includes("Privacy") ||
        log.message.includes("Failed") ||
        log.message.includes("Success")
    ) ?? [];

    return (
        <Card className="h-full border-blue-500/10 bg-blue-500/5">
            <CardHeader className="pb-3">
                <CardTitle className="flex items-center gap-2 text-sm text-blue-400">
                    <ShieldCheck className="w-4 h-4" />
                    Agent Audit Log
                </CardTitle>
            </CardHeader>
            <CardContent>
                <div className="h-[200px] w-full rounded-md border border-white/5 bg-black/20 p-4 overflow-y-auto">
                    <div className="space-y-3">
                        {auditLogs.length > 0 ? (
                            auditLogs.map((log, i) => (
                                <LogItem key={i} log={log} />
                            ))
                        ) : (
                            <div className="text-xs text-muted-foreground text-center py-8">
                                No audit events recorded yet.
                            </div>
                        )}
                    </div>
                </div>
            </CardContent>
        </Card>
    );
}

function LogItem({ log }: { log: LogEntry }) {
    let Icon = CheckCircle2;
    let color = "text-green-500";

    if (log.message.includes("Failed") || log.level === "ERROR") {
        Icon = XCircle;
        color = "text-red-500";
    } else if (log.message.includes("Recovery") || log.message.includes("Retrying")) {
        Icon = ShieldCheck;
        color = "text-yellow-500"; // Self-Healing
    } else if (log.message.includes("Privacy")) {
        Icon = AlertTriangle;
        color = "text-purple-500";
    }

    return (
        <div className="flex gap-3 text-xs">
            <Icon className={`w-4 h-4 ${color} shrink-0`} />
            <div className="flex flex-col gap-0.5">
                <span className="font-medium text-foreground">{log.message}</span>
                <span className="text-[10px] text-muted-foreground font-mono">
                    {format(new Date(log.timestamp), "HH:mm:ss")}
                </span>
            </div>
        </div>
    );
}
