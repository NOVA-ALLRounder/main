import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { fetchCurrentGoal, sendFeedback } from "@/lib/api";
import { useEffect, useState } from "react";

export function FeedbackCard() {
    const [goal, setGoal] = useState("");
    const [feedback, setFeedback] = useState("");
    const [summary, setSummary] = useState("");
    const [status, setStatus] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);

    useEffect(() => {
        let mounted = true;
        fetchCurrentGoal()
            .then((g) => {
                if (!mounted || !g) return;
                setGoal((currentGoal) => currentGoal || g);
            })
            .catch(() => {});
        return () => {
            mounted = false;
        };
    }, []);

    const handleSubmit = async () => {
        if (!goal.trim() || !feedback.trim()) {
            setStatus("Goal and feedback are required.");
            return;
        }
        setLoading(true);
        setStatus(null);
        try {
            const res = await sendFeedback(goal.trim(), feedback.trim(), summary.trim() || undefined);
            setStatus(res.message || "Feedback submitted.");
            if (res.new_goal) {
                setGoal(res.new_goal);
            }
            setFeedback("");
        } catch {
            setStatus("Failed to submit feedback.");
        } finally {
            setLoading(false);
        }
    };

    return (
        <Card className="h-auto mb-4">
            <CardHeader>
                <CardTitle>Feedback Loop</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
                <input
                    value={goal}
                    onChange={(e) => setGoal(e.target.value)}
                    placeholder="Current goal (required)"
                    className="w-full rounded-md bg-white/5 border border-white/10 px-3 py-2 text-sm"
                />
                <textarea
                    value={feedback}
                    onChange={(e) => setFeedback(e.target.value)}
                    placeholder="What should be refined or fixed?"
                    rows={3}
                    className="w-full rounded-md bg-white/5 border border-white/10 px-3 py-2 text-sm"
                />
                <input
                    value={summary}
                    onChange={(e) => setSummary(e.target.value)}
                    placeholder="Optional context / history summary"
                    className="w-full rounded-md bg-white/5 border border-white/10 px-3 py-2 text-sm"
                />
                {status && <div className="text-xs text-muted-foreground">{status}</div>}
                <button
                    onClick={handleSubmit}
                    disabled={loading}
                    className="w-full bg-white/10 hover:bg-white/20 text-sm py-2 rounded transition-colors disabled:opacity-50"
                >
                    {loading ? "Submitting..." : "Submit Feedback"}
                </button>
            </CardContent>
        </Card>
    );
}
