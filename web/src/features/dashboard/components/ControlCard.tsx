import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { executeGoal } from "@/lib/api";
import { Activity, ArrowUpRight, Lightbulb, ShieldCheck } from "lucide-react";
import { useState } from "react";

type ControlCardProps = {
    activeRoutinesCount: number;
    pendingRecommendations: number;
    isOffline: boolean;
};

const GOAL_FIELD_ID = "dashboard-goal-prompt";
const GOAL_HINT_ID = "dashboard-goal-prompt-hint";
const GOAL_STATUS_ID = "dashboard-goal-prompt-status";

export function ControlCard({
    activeRoutinesCount,
    pendingRecommendations,
    isOffline,
}: ControlCardProps) {
    const [goal, setGoal] = useState("");
    const [loading, setLoading] = useState(false);
    const [message, setMessage] = useState<string | null>(null);
    const trimmedGoal = goal.trim();

    const handleExecute = async () => {
        if (!trimmedGoal) return;
        setLoading(true);
        setMessage(null);
        try {
            const res = await executeGoal(trimmedGoal);
            setMessage(res.message || "Goal started.");
            setGoal("");
        } catch {
            setMessage("Failed to start goal.");
        } finally {
            setLoading(false);
        }
    };

    return (
        <Card className="relative h-auto overflow-hidden border-white/10 bg-slate-950/80">
            <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-cyan-400/0 via-cyan-300/70 to-cyan-400/0" />
            <div className="absolute -left-16 top-10 h-40 w-40 rounded-full bg-cyan-500/10 blur-3xl" />
            <div className="absolute -right-14 bottom-0 h-36 w-36 rounded-full bg-indigo-500/10 blur-3xl" />
            <CardHeader className="relative space-y-4 pb-5">
                <div className="flex flex-wrap items-center gap-2 text-[11px] uppercase tracking-[0.28em] text-slate-400">
                    <span className="rounded-full border border-cyan-400/20 bg-cyan-400/10 px-3 py-1 text-cyan-100">
                        Primary Control
                    </span>
                    <span className="rounded-full border border-white/10 px-3 py-1">
                        {isOffline ? "Runtime Offline" : "Runtime Live"}
                    </span>
                </div>
                    <div className="space-y-3">
                        <CardTitle className="text-3xl font-semibold tracking-[-0.05em] text-white [text-wrap:balance] sm:text-[2.25rem]">
                            Dispatch a Live Goal
                        </CardTitle>
                    <p className="max-w-2xl text-sm leading-6 text-slate-300">
                        Write the operator intent once. The agent will pick up planning,
                        execution, and verification from the dashboard and launcher flow.
                    </p>
                </div>
            </CardHeader>
            <CardContent className="relative">
                <div className="space-y-4">
                    <form
                        className="rounded-[1.4rem] border border-white/10 bg-black/25 p-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]"
                        onSubmit={(event) => {
                            event.preventDefault();
                            void handleExecute();
                        }}
                    >
                        <label
                            htmlFor={GOAL_FIELD_ID}
                            className="mb-3 block text-[11px] uppercase tracking-[0.28em] text-slate-500"
                        >
                            Goal prompt
                        </label>
                        <textarea
                            id={GOAL_FIELD_ID}
                            name="goalPrompt"
                            value={goal}
                            onChange={(e) => setGoal(e.target.value)}
                            placeholder="Describe the task, guardrails, and expected result…"
                            rows={4}
                            autoComplete="off"
                            aria-describedby={message ? `${GOAL_HINT_ID} ${GOAL_STATUS_ID}` : GOAL_HINT_ID}
                            className="min-h-[148px] w-full resize-none rounded-2xl border border-white/10 bg-slate-950/85 px-4 py-3 text-sm leading-6 text-slate-100 outline-none transition-colors placeholder:text-slate-500 focus:border-cyan-400/40 focus-visible:border-cyan-300/60 focus-visible:ring-2 focus-visible:ring-cyan-300/20"
                            onKeyDown={(e) => {
                                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                                    e.preventDefault();
                                    void handleExecute();
                                }
                            }}
                        />
                        <div className="mt-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                            <div id={GOAL_HINT_ID} className="text-xs text-slate-400">
                                Use Cmd/Ctrl + Enter for a fast launch.
                            </div>
                            <button
                                type="submit"
                                disabled={loading || !trimmedGoal}
                                className="inline-flex items-center justify-center gap-2 rounded-2xl border border-cyan-300/25 bg-cyan-400/15 px-4 py-2.5 text-sm font-medium text-cyan-50 transition-colors hover:bg-cyan-400/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-300/30 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-950 disabled:opacity-50"
                            >
                                {loading ? "Launching…" : "Launch Goal"}
                                <ArrowUpRight aria-hidden="true" className="h-4 w-4" />
                            </button>
                        </div>
                        {message && (
                            <div
                                id={GOAL_STATUS_ID}
                                role="status"
                                aria-live="polite"
                                className="mt-4 rounded-2xl border border-cyan-400/15 bg-cyan-400/10 px-3 py-2 text-xs text-cyan-100"
                            >
                                {message}
                            </div>
                        )}
                    </form>
                    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
                        <div className="rounded-[1.35rem] border border-white/10 bg-white/5 p-4">
                            <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.28em] text-slate-500">
                                <Activity aria-hidden="true" className="h-3.5 w-3.5 text-cyan-300" />
                                Runtime posture
                            </div>
                            <div className="mt-3 text-2xl font-semibold tracking-[-0.04em] text-white">
                                {activeRoutinesCount}
                            </div>
                            <p className="mt-1 text-xs leading-5 text-slate-300">
                                active routines keeping the local agent warm.
                            </p>
                        </div>
                        <div className="rounded-[1.35rem] border border-white/10 bg-white/5 p-4">
                            <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.28em] text-slate-500">
                                <Lightbulb aria-hidden="true" className="h-3.5 w-3.5 text-amber-300" />
                                Queue pressure
                            </div>
                            <div className="mt-3 text-2xl font-semibold tracking-[-0.04em] text-white">
                                {pendingRecommendations}
                            </div>
                            <p className="mt-1 text-xs leading-5 text-slate-300">
                                pending recommendations waiting for review.
                            </p>
                        </div>
                        <div className="rounded-[1.35rem] border border-white/10 bg-white/5 p-4">
                            <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.28em] text-slate-500">
                                <ShieldCheck
                                    aria-hidden="true"
                                    className="h-3.5 w-3.5 text-emerald-300"
                                />
                                Operator note
                            </div>
                            <p className="mt-3 text-sm leading-6 text-slate-300">
                                Strong prompts name the target app, intended outcome, and
                                what counts as done.
                            </p>
                        </div>
                    </div>
                </div>
            </CardContent>
        </Card>
    );
}
