import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { toggleRoutine } from "@/lib/api";
import { useRoutineRuns, useRoutines } from "@/lib/hooks";
import { format } from "date-fns";

export function RoutinesCard() {
    const { data: routines, refetch: refetchRoutines } = useRoutines();

    const handleToggle = async (id: number, enabled: boolean) => {
        try {
            await toggleRoutine(id, enabled);
            refetchRoutines();
        } catch {
            // ignore toggle failure toast here; parent dashboard stays passive
        }
    };

    return (
        <Card className="h-auto mb-4 border-purple-500/20 bg-purple-500/5">
            <CardHeader>
                <CardTitle>Active Routines</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
                {routines && routines.length > 0 ? (
                    <div className="space-y-2 max-h-40 overflow-y-auto">
                        {routines.map((routine) => (
                            <div key={routine.id} className="flex items-center justify-between bg-white/5 p-2 rounded">
                                <div className="text-xs">
                                    <div className="font-semibold text-white/90">{routine.name}</div>
                                    <div className="font-mono text-white/60">{routine.cron_expression}</div>
                                </div>
                                <label className="relative inline-flex items-center cursor-pointer">
                                    <input
                                        type="checkbox"
                                        className="sr-only peer"
                                        checked={routine.enabled}
                                        onChange={(e) => handleToggle(routine.id, e.target.checked)}
                                    />
                                    <div className="w-9 h-5 bg-white/10 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-purple-500"></div>
                                </label>
                            </div>
                        ))}
                    </div>
                ) : (
                    <div className="text-muted-foreground text-xs">No routines configured.</div>
                )}
                <RoutineRunHistory />
            </CardContent>
        </Card>
    );
}

function RoutineRunHistory() {
    const { data: runs } = useRoutineRuns(5);
    if (!runs || runs.length === 0) return null;
    return (
        <div className="space-y-2 mt-2 border-t border-purple-500/10 pt-2">
            <div className="text-xs text-muted-foreground">Recent Activity</div>
            <div className="space-y-1">
                {runs.slice(0, 3).map((run) => (
                    <div key={run.id} className="flex justify-between text-[10px] bg-white/5 p-1 rounded">
                        <span className="text-purple-200">{run.routine_name}</span>
                        <div className="flex gap-2">
                            <span className={run.status === "success" ? "text-emerald-300" : "text-rose-300"}>
                                {run.status.toUpperCase()}
                            </span>
                            <span className="text-muted-foreground">
                                {format(new Date(run.started_at), "HH:mm")}
                            </span>
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
}
