import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { analyzePatterns, createRoutine } from "@/lib/api";
import { useState } from "react";

export function QuickActionsCard() {
    const [showRoutine, setShowRoutine] = useState(false);
    const [routineName, setRoutineName] = useState("");
    const [routineCron, setRoutineCron] = useState("");
    const [routinePrompt, setRoutinePrompt] = useState("");
    const [routineLoading, setRoutineLoading] = useState(false);

    const [analyzing, setAnalyzing] = useState(false);
    const [patterns, setPatterns] = useState<string[] | null>(null);

    const handleCreateRoutine = async () => {
        if (!routineName || !routineCron || !routinePrompt) return;
        setRoutineLoading(true);
        try {
            await createRoutine(routineName, routineCron, routinePrompt);
            setShowRoutine(false);
            setRoutineName("");
            setRoutineCron("");
            setRoutinePrompt("");
        } catch {
            alert("Failed to create routine");
        } finally {
            setRoutineLoading(false);
        }
    };

    const handleAnalyze = async () => {
        setAnalyzing(true);
        try {
            const res = await analyzePatterns();
            setPatterns(res);
        } catch {
            setPatterns([]);
        } finally {
            setAnalyzing(false);
        }
    };

    return (
        <Card className="h-auto">
            <CardHeader>
                <CardTitle>Quick Actions</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
                <Dialog open={showRoutine} onOpenChange={setShowRoutine}>
                    <DialogTrigger asChild>
                        <button className="w-full text-left px-4 py-2 rounded-lg hover:bg-white/5 transition-colors text-sm">
                            ➕ Create New Routine
                        </button>
                    </DialogTrigger>
                    <DialogContent className="bg-[#1a1a1a] border-white/10 text-white">
                        <DialogHeader>
                            <DialogTitle>Create New Routine</DialogTitle>
                        </DialogHeader>
                        <div className="space-y-3 py-4">
                            <input
                                placeholder="Routine Name"
                                value={routineName}
                                onChange={(e) => setRoutineName(e.target.value)}
                                className="w-full bg-black/20 border border-white/10 rounded px-3 py-2 text-sm"
                            />
                            <input
                                placeholder="Cron Expression (e.g. 0 9 * * *)"
                                value={routineCron}
                                onChange={(e) => setRoutineCron(e.target.value)}
                                className="w-full bg-black/20 border border-white/10 rounded px-3 py-2 text-sm font-mono"
                            />
                            <textarea
                                placeholder="What should the agent do?"
                                value={routinePrompt}
                                onChange={(e) => setRoutinePrompt(e.target.value)}
                                rows={3}
                                className="w-full bg-black/20 border border-white/10 rounded px-3 py-2 text-sm"
                            />
                            <button
                                onClick={handleCreateRoutine}
                                disabled={routineLoading}
                                className="w-full bg-indigo-500 hover:bg-indigo-600 text-white py-2 rounded"
                            >
                                {routineLoading ? "Creating..." : "Create Routine"}
                            </button>
                        </div>
                    </DialogContent>
                </Dialog>

                <button
                    onClick={handleAnalyze}
                    disabled={analyzing}
                    className="w-full text-left px-4 py-2 rounded-lg hover:bg-white/5 transition-colors text-sm flex justify-between items-center"
                >
                    <span>🔍 Analyze Patterns</span>
                    {analyzing && <span className="text-xs text-muted-foreground">...</span>}
                </button>

                {patterns && (
                    <div className="bg-black/20 p-2 rounded text-xs space-y-1">
                        {patterns.length > 0 ? (
                            patterns.map((p, i) => <div key={i}>• {p}</div>)
                        ) : (
                            <div className="text-muted-foreground">No patterns found.</div>
                        )}
                        <button
                            onClick={() => setPatterns(null)}
                            className="text-[10px] text-white/50 w-full text-center mt-1"
                        >
                            Close
                        </button>
                    </div>
                )}

                <button
                    onClick={() => alert("Settings are currently managed via config.toml")}
                    className="w-full text-left px-4 py-2 rounded-lg hover:bg-white/5 transition-colors text-sm"
                >
                    ⚙️ Open Settings
                </button>
            </CardContent>
        </Card>
    );
}
