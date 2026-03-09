import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Lightbulb } from "lucide-react";
import { useState } from "react";
import { useRecommendations } from "@/lib/hooks";
import { approveRecommendation, laterRecommendation, rejectRecommendation, restoreRecommendation, sendRecommendationFeedback } from "@/lib/api";

export function RecommendationsCard() {

    const { data: recs, refetch } = useRecommendations();
    const [filter, setFilter] = useState("");
    const pendingRecs = recs?.filter(r => r.status === 'pending') ?? [];
    const laterRecs = recs?.filter(r => r.status === 'later') ?? [];
    const failedRecs = recs?.filter(r => r.status === 'failed') ?? [];
    const [feedbackOpenId, setFeedbackOpenId] = useState<number | null>(null);
    const [feedbackText, setFeedbackText] = useState<Record<number, string>>({});
    const [feedbackStatus, setFeedbackStatus] = useState<Record<number, string>>({});

    const handleApprove = async (id: number) => {
        try {
            await approveRecommendation(id, "web_dashboard");
            refetch(); // Soft refresh
        } catch (e) {
            console.error("Failed to approve", e);
        }
    };

    const handleReject = async (id: number) => {
        try {
            await rejectRecommendation(id, "web_dashboard");
            refetch(); // Remove from list
        } catch (e) {
            console.error("Failed to reject", e);
        }
    };

    const handleLater = async (id: number) => {
        try {
            await laterRecommendation(id, "web_dashboard");
            refetch();
        } catch (e) {
            console.error("Failed to defer", e);
        }
    };

    const handleRestore = async (id: number) => {
        try {
            await restoreRecommendation(id, "web_dashboard");
            refetch();
        } catch (e) {
            console.error("Failed to restore", e);
        }
    };

    const handleFeedbackSubmit = async (recId: number) => {
        const text = (feedbackText[recId] || "").trim();
        if (!text) {
            setFeedbackStatus((prev) => ({ ...prev, [recId]: "Feedback is required." }));
            return;
        }
        try {
            const res = await sendRecommendationFeedback(recId, text, "web_dashboard");
            setFeedbackStatus((prev) => ({ ...prev, [recId]: res.message || "Feedback submitted." }));
            setFeedbackText((prev) => ({ ...prev, [recId]: "" }));
            setFeedbackOpenId((current) => (current === recId ? null : current));
            refetch();
        } catch {
            setFeedbackStatus((prev) => ({ ...prev, [recId]: "Failed to submit feedback." }));
        }
    };

    const filterText = filter.trim().toLowerCase();
    const applyFilter = (items: typeof pendingRecs) =>
        filterText
            ? items.filter(r =>
                r.title.toLowerCase().includes(filterText) ||
                r.summary.toLowerCase().includes(filterText)
            )
            : items;
    const filteredPending = applyFilter(pendingRecs);
    const filteredLater = applyFilter(laterRecs);
    const filteredFailed = applyFilter(failedRecs);

    if (pendingRecs.length === 0 && laterRecs.length === 0 && failedRecs.length === 0) return null;

    return (
        <Card className="h-auto mb-4 border-yellow-500/20 bg-yellow-500/5">
            <CardHeader>
                <CardTitle className="flex items-center gap-2 text-yellow-500">
                    <span className="relative flex h-3 w-3">
                        <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-yellow-400 opacity-75"></span>
                        <span className="relative inline-flex rounded-full h-3 w-3 bg-yellow-500"></span>
                    </span>
                    Work Proposals ({filteredPending.length})
                </CardTitle>
            </CardHeader>
            <CardContent>
                <div className="mb-3">
                    <input
                        value={filter}
                        onChange={(e) => setFilter(e.target.value)}
                        placeholder="Filter by title or summary..."
                        className="w-full rounded-md bg-white/5 border border-white/10 px-3 py-2 text-xs"
                    />
                </div>
                <div className="space-y-4">
                    {filteredPending.map(rec => (
                        <div key={rec.id} className="p-3 bg-black/40 rounded-lg border border-white/10 relative overflow-hidden group">
                            {/* Confidence Indicator */}
                            <div className="absolute top-0 right-0 p-1">
                                <span className={`text-[10px] px-1.5 py-0.5 rounded-bl-md font-mono ${rec.confidence > 0.8 ? 'bg-green-500/20 text-green-400' : 'bg-yellow-500/20 text-yellow-400'}`}>
                                    {(rec.confidence * 100).toFixed(0)}%
                                </span>
                            </div>

                            <h4 className="font-semibold text-sm mb-1 pr-8">{rec.title}</h4>
                            <p className="text-xs text-muted-foreground mb-2">{rec.summary}</p>

                            {/* [Evidence UI] */}
                            {rec.evidence && rec.evidence.length > 0 && (
                                <div className="mb-3 p-2.5 bg-indigo-500/10 rounded-md border border-indigo-500/20 text-[11px] space-y-1.5">
                                    <div className="flex items-center gap-1.5 text-indigo-400 font-bold uppercase tracking-wide">
                                        <Lightbulb className="w-3.5 h-3.5" />
                                        <span>Why this?</span>
                                    </div>
                                    <ul className="space-y-1 text-gray-300 leading-tight">
                                        {rec.evidence.slice(0, 3).map((ev, i) => (
                                            <li key={i} className="flex gap-1.5 items-start">
                                                <span className="text-indigo-500/50 block mt-0.5">•</span>
                                                <span className="opacity-90">{ev}</span>
                                            </li>
                                        ))}
                                    </ul>
                                </div>
                            )}

                            {!rec.approval_ready && rec.status === 'pending' && rec.approval_reasons.length > 0 && (
                                <div className="mb-3 rounded-md border border-amber-500/20 bg-amber-500/10 px-2.5 py-2 text-[11px] text-amber-100">
                                    {rec.approval_reasons.join(" ")}
                                </div>
                            )}

                            <div className="flex gap-2">
                                <button
                                    onClick={() => handleApprove(rec.id)}
                                    disabled={!rec.approval_ready}
                                    title={!rec.approval_ready ? rec.approval_reasons.join(" ") : undefined}
                                    className={`flex-1 text-xs py-1.5 rounded transition-colors font-medium ${
                                        rec.approval_ready
                                            ? "bg-green-600 hover:bg-green-700 text-white"
                                            : "bg-white/10 text-muted-foreground cursor-not-allowed"
                                    }`}
                                >
                                    Approve
                                </button>
                                <button
                                    onClick={() => handleLater(rec.id)}
                                    className="flex-1 bg-white/10 hover:bg-white/20 text-xs py-1.5 rounded transition-colors"
                                >
                                    Later
                                </button>
                                <button
                                    onClick={() => handleReject(rec.id)}
                                    className="flex-1 bg-red-500/20 hover:bg-red-500/30 text-xs py-1.5 rounded transition-colors text-red-200"
                                >
                                    Reject
                                </button>
                            </div>

                            <button
                                onClick={() => setFeedbackOpenId(feedbackOpenId === rec.id ? null : rec.id)}
                                className="mt-2 text-[11px] text-indigo-200 hover:text-indigo-100"
                            >
                                {feedbackOpenId === rec.id ? "Hide feedback" : "Send feedback"}
                            </button>

                            {feedbackOpenId === rec.id && (
                                <div className="mt-2 space-y-2">
                                    <textarea
                                        value={feedbackText[rec.id] || ""}
                                        onChange={(e) => setFeedbackText((prev) => ({ ...prev, [rec.id]: e.target.value }))}
                                        placeholder="What should be refined?"
                                        rows={2}
                                        className="w-full rounded-md bg-white/5 border border-white/10 px-2 py-1.5 text-xs"
                                    />
                                    {feedbackStatus[rec.id] && (
                                        <div className="text-[11px] text-muted-foreground">{feedbackStatus[rec.id]}</div>
                                    )}
                                    <button
                                        onClick={() => handleFeedbackSubmit(rec.id)}
                                        className="w-full bg-white/10 hover:bg-white/20 text-xs py-1.5 rounded transition-colors"
                                    >
                                        Submit Feedback
                                    </button>
                                </div>
                            )}
                        </div>
                    ))}
                </div>

                {filteredLater.length > 0 && (
                    <div className="mt-5 pt-4 border-t border-white/10">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground mb-2">
                            Later ({filteredLater.length})
                        </div>
                        <div className="space-y-2">
                            {filteredLater.slice(0, 3).map(rec => (
                                <div key={rec.id} className="flex items-center justify-between text-xs bg-white/5 rounded px-2 py-1.5">
                                    <span className="truncate">{rec.title}</span>
                                    <button
                                        onClick={() => handleRestore(rec.id)}
                                        className="text-indigo-300 hover:text-indigo-200"
                                    >
                                        Show again
                                    </button>
                                </div>
                            ))}
                        </div>
                    </div>
                )}

                {filteredFailed.length > 0 && (
                    <div className="mt-5 pt-4 border-t border-white/10">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground mb-2">
                            Failed ({filteredFailed.length})
                        </div>
                        <div className="space-y-2">
                            {filteredFailed.slice(0, 2).map(rec => (
                                <div key={rec.id} className="text-xs bg-red-500/10 border border-red-500/20 rounded px-2 py-1.5">
                                    <div className="font-medium truncate">{rec.title}</div>
                                    <div className="text-red-200/80 truncate">
                                        {rec.last_error ?? "Unknown error"}
                                    </div>
                                    <div className="mt-2 flex gap-2">
                                        <button
                                            onClick={() => handleApprove(rec.id)}
                                            disabled={!rec.approval_ready}
                                            title={!rec.approval_ready ? rec.approval_reasons.join(" ") : undefined}
                                            className={`text-[11px] px-2 py-1 rounded ${
                                                rec.approval_ready
                                                    ? "bg-red-500/20 hover:bg-red-500/30 text-red-200"
                                                    : "bg-white/10 text-muted-foreground cursor-not-allowed"
                                            }`}
                                        >
                                            Retry
                                        </button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </div>
                )}
            </CardContent>
        </Card>
    );
}
