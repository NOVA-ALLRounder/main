import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { executeGoal } from "@/lib/api";
import { useState } from "react";

export function ControlCard() {
    const [goal, setGoal] = useState("");
    const [loading, setLoading] = useState(false);
    const [message, setMessage] = useState<string | null>(null);

    const handleExecute = async () => {
        if (!goal.trim()) return;
        setLoading(true);
        setMessage(null);
        try {
            const res = await executeGoal(goal.trim());
            setMessage(res.message || "Goal started.");
            setGoal("");
        } catch {
            setMessage("Failed to start goal.");
        } finally {
            setLoading(false);
        }
    };

    return (
        <Card className="h-auto mb-4 border-indigo-500/20 bg-indigo-500/5">
            <CardHeader>
                <CardTitle>Agent Control</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
                <div className="flex gap-2">
                    <input
                        value={goal}
                        onChange={(e) => setGoal(e.target.value)}
                        placeholder="Enter a new goal to execute..."
                        className="flex-1 rounded-md bg-white/5 border border-white/10 px-3 py-2 text-sm focus:outline-none focus:border-indigo-500/50"
                        onKeyDown={(e) => e.key === "Enter" && handleExecute()}
                    />
                    <button
                        onClick={handleExecute}
                        disabled={loading || !goal.trim()}
                        className="bg-indigo-500/20 hover:bg-indigo-500/30 text-indigo-100 text-sm px-4 py-2 rounded transition-colors disabled:opacity-50 font-medium"
                    >
                        {loading ? "Starting..." : "Execute"}
                    </button>
                </div>
                {message && (
                    <div className="text-xs text-indigo-200 bg-indigo-500/10 p-2 rounded">
                        {message}
                    </div>
                )}
            </CardContent>
        </Card>
    );
}
