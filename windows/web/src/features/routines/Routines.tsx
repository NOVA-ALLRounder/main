import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { useRoutines } from "@/lib/hooks";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createRoutine, toggleRoutine } from "@/lib/api";
import { Plus, Clock, Zap } from "lucide-react";
import type { Routine } from "@/lib/types";

function RoutineCard({
    routine,
    onToggle,
    toggling,
}: {
    routine: Routine;
    onToggle: (id: number, enabled: boolean) => void;
    toggling: boolean;
}) {
    return (
        <Card className="group hover:border-primary/30 transition-all duration-300">
            <CardHeader className="flex flex-row items-start justify-between space-y-0 pb-2">
                <div className="space-y-1">
                    <CardTitle className="text-lg">{routine.name}</CardTitle>
                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                        <Clock className="w-3 h-3" />
                        <span className="font-mono">{routine.cron_expression}</span>
                    </div>
                </div>
                <Switch
                    checked={routine.enabled}
                    onCheckedChange={(checked) => onToggle(routine.id, checked)}
                    disabled={toggling}
                />
            </CardHeader>
            <CardContent>
                {routine.next_run && (
                    <p className="text-xs text-muted-foreground">
                        Next: {new Date(routine.next_run).toLocaleString()}
                    </p>
                )}
            </CardContent>
        </Card>
    );
}

function CreateRoutineDialog() {
    const [open, setOpen] = useState(false);
    const [name, setName] = useState("");
    const [cron, setCron] = useState("0 9 * * *");
    const [prompt, setPrompt] = useState("");

    const queryClient = useQueryClient();
    const mutation = useMutation({
        mutationFn: () => createRoutine(name, cron, prompt),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["routines"] });
            setOpen(false);
            setName("");
            setPrompt("");
        },
    });

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogTrigger asChild>
                <button className="glass px-4 py-2 rounded-xl flex items-center gap-2 text-sm font-medium hover:bg-white/10 transition-colors">
                    <Plus className="w-4 h-4" />
                    New Routine
                </button>
            </DialogTrigger>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>Create New Routine</DialogTitle>
                </DialogHeader>
                <div className="space-y-4 pt-4">
                    <div>
                        <label className="text-sm font-medium block mb-1">Name</label>
                        <input
                            type="text"
                            value={name}
                            onChange={(e) => setName(e.target.value)}
                            placeholder="Morning Briefing"
                            className="w-full px-3 py-2 rounded-lg bg-white/5 border border-white/10 focus:border-primary/50 outline-none text-sm"
                        />
                    </div>
                    <div>
                        <label className="text-sm font-medium block mb-1">Schedule (Cron)</label>
                        <input
                            type="text"
                            value={cron}
                            onChange={(e) => setCron(e.target.value)}
                            placeholder="0 9 * * *"
                            className="w-full px-3 py-2 rounded-lg bg-white/5 border border-white/10 focus:border-primary/50 outline-none text-sm font-mono"
                        />
                        <p className="text-xs text-muted-foreground mt-1">e.g., "0 9 * * *" = Every day at 9 AM</p>
                    </div>
                    <div>
                        <label className="text-sm font-medium block mb-1">Command / Prompt</label>
                        <textarea
                            value={prompt}
                            onChange={(e) => setPrompt(e.target.value)}
                            placeholder="Summarize today's news..."
                            rows={3}
                            className="w-full px-3 py-2 rounded-lg bg-white/5 border border-white/10 focus:border-primary/50 outline-none text-sm resize-none"
                        />
                    </div>
                    <button
                        onClick={() => mutation.mutate()}
                        disabled={mutation.isPending || !name}
                        className="w-full py-2 rounded-lg bg-primary text-primary-foreground font-medium hover:bg-primary/90 disabled:opacity-50 transition-colors flex items-center justify-center gap-2"
                    >
                        {mutation.isPending ? "Creating..." : (
                            <>
                                <Zap className="w-4 h-4" />
                                Create Routine
                            </>
                        )}
                    </button>
                </div>
            </DialogContent>
        </Dialog>
    );
}

export default function Routines() {
    const { data: routines, isLoading } = useRoutines();
    const [togglingId, setTogglingId] = useState<number | null>(null);
    const queryClient = useQueryClient();

    const toggleMutation = useMutation({
        mutationFn: ({ id, enabled }: { id: number; enabled: boolean }) => toggleRoutine(id, enabled),
        onMutate: ({ id }) => {
            setTogglingId(id);
        },
        onSettled: () => {
            setTogglingId(null);
            queryClient.invalidateQueries({ queryKey: ["routines"] });
        },
    });

    return (
        <div className="space-y-6">
            <div className="flex items-center justify-between">
                <h2 className="text-3xl font-bold tracking-tight text-glow">Routines</h2>
                <CreateRoutineDialog />
            </div>

            {isLoading ? (
                <div className="glass p-10 text-center text-muted-foreground">Loading...</div>
            ) : routines && routines.length > 0 ? (
                <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                    {routines.map((routine) => (
                        <RoutineCard
                            key={routine.id}
                            routine={routine}
                            onToggle={(id, enabled) => toggleMutation.mutate({ id, enabled })}
                            toggling={togglingId === routine.id && toggleMutation.isPending}
                        />
                    ))}
                </div>
            ) : (
                <Card className="p-10 text-center">
                    <div className="text-muted-foreground mb-4">No routines yet. Create your first one!</div>
                    <CreateRoutineDialog />
                </Card>
            )}
        </div>
    );
}
