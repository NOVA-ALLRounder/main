import { useState } from "react";
import { useRecommendations } from "@/lib/hooks";
import { approveRecommendation, laterRecommendation, rejectRecommendation, restoreRecommendation } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { CheckCircle, XCircle, ExternalLink, Workflow } from "lucide-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";

export default function Workflows() {
    const [category, setCategory] = useState("work");
    const { data: recommendations, isLoading } = useRecommendations(category);
    const queryClient = useQueryClient();
    const categoryBrowsingEnabled = (import.meta.env.VITE_ALLVIA_ENABLE_RECOMMENDATION_CATEGORY_BROWSING as string | undefined)
        ?.trim()
        .toLowerCase() === "true";
    const categoryOptions = categoryBrowsingEnabled ? [
        { value: "work", label: "Work" },
        { value: "all", label: "All" },
        { value: "personal", label: "Personal" },
        { value: "system", label: "System" },
    ] : [
        { value: "work", label: "Work" },
    ];

    // Mutations


    const approve = useMutation({
        mutationFn: (id: number) => approveRecommendation(id, "web_workflows"),
        onSuccess: () => queryClient.invalidateQueries({ queryKey: ["recommendations"] }),
        onError: (error) => alert(`Approve failed: ${error}`),
    });

    const reject = useMutation({
        mutationFn: (id: number) => rejectRecommendation(id, "web_workflows"),
        onSuccess: () => queryClient.invalidateQueries({ queryKey: ["recommendations"] }),
        onError: (error) => alert(`Reject failed: ${error}`),
    });

    const later = useMutation({
        mutationFn: (id: number) => laterRecommendation(id, "web_workflows"),
        onSuccess: () => queryClient.invalidateQueries({ queryKey: ["recommendations"] }),
        onError: (error) => alert(`Later failed: ${error}`),
    });

    const restore = useMutation({
        mutationFn: (id: number) => restoreRecommendation(id, "web_workflows"),
        onSuccess: () => queryClient.invalidateQueries({ queryKey: ["recommendations"] }),
        onError: (error) => alert(`Restore failed: ${error}`),
    });

    const n8nEditorBaseUrl = (() => {
        const raw = import.meta.env.VITE_N8N_EDITOR_URL as string | undefined;
        const trimmed = raw?.trim().replace(/\/+$/, "");
        return trimmed || "http://localhost:5678";
    })();

    const resolveWorkflowUrl = (workflowUrl?: string | null, workflowId?: string | null) => {
        const direct = workflowUrl?.trim();
        if (direct) return direct;
        const id = workflowId?.trim();
        if (!id) return null;
        if (id.startsWith("provisioning:")) return null;
        return `${n8nEditorBaseUrl}/workflow/${encodeURIComponent(id)}`;
    };

    return (
        <div className="space-y-6">
            <div className="flex items-center justify-between">
                <h2 className="text-3xl font-bold tracking-tight text-glow">Workflows</h2>
                {categoryBrowsingEnabled && (
                    <div className="flex flex-wrap items-center gap-2">
                        {categoryOptions.map((option) => (
                            <button
                                key={option.value}
                                onClick={() => setCategory(option.value)}
                                className={`rounded-full px-3 py-1 text-xs font-semibold transition-colors ${
                                    category === option.value
                                        ? "bg-primary text-primary-foreground"
                                        : "bg-white/5 text-muted-foreground hover:bg-white/10"
                                }`}
                            >
                                {option.label}
                            </button>
                        ))}
                    </div>
                )}
            </div>

            {isLoading ? (
                <div className="glass p-10 text-center text-muted-foreground">Loading workflows...</div>
            ) : recommendations && recommendations.length > 0 ? (
                <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                    {recommendations.map((rec) => (
                        <Card key={rec.id} className="group hover:border-primary/30 transition-all duration-300">
                            <CardHeader className="flex flex-row items-start justify-between space-y-0 pb-2">
                                <div className="space-y-1">
                                    <CardTitle className="text-lg flex items-center gap-2">
                                        <Workflow className="w-4 h-4 text-primary" />
                                        {rec.title}
                                    </CardTitle>
                                    <div className="flex items-center gap-2">
                                        <span className={`text-xs px-2 py-0.5 rounded-full font-mono font-bold
                            ${rec.status === 'pending' ? 'bg-yellow-500/20 text-yellow-500' :
                                                rec.status === 'later' ? 'bg-sky-500/20 text-sky-400' :
                                                rec.status === 'approved' ? 'bg-green-500/20 text-green-500' :
                                                    rec.status === 'rejected' ? 'bg-red-500/20 text-red-500' : 'bg-gray-500/20 text-gray-400'}
                        `}>
                                            {rec.status.toUpperCase()}
                                        </span>
                                        <span className="text-xs px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-300 font-mono">
                                            {rec.category.toUpperCase()}
                                        </span>
                                        <span className="text-xs text-muted-foreground">Confidence: {(rec.confidence * 100).toFixed(0)}%</span>
                                    </div>
                                </div>
                            </CardHeader>
                            <CardContent className="space-y-4">
                                <p className="text-sm text-muted-foreground line-clamp-3">
                                    {rec.summary}
                                </p>
                                <div className="text-xs text-muted-foreground">
                                    Business score: {(rec.business_score * 100).toFixed(0)}%
                                </div>
                                {!rec.approval_ready && rec.status === 'pending' && rec.approval_reasons.length > 0 && (
                                    <div className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-100">
                                        <div className="font-medium text-amber-200">Needs more evidence before approval</div>
                                        <div className="mt-1 line-clamp-3">{rec.approval_reasons.join(" ")}</div>
                                    </div>
                                )}

                                <div className="flex items-center gap-2 pt-2">
                                    {/* Status Actions */}
                                    {rec.status === 'pending' && (
                                        <>
                                            <button
                                                onClick={() => approve.mutate(rec.id)}
                                                disabled={approve.isPending || !rec.approval_ready}
                                                title={!rec.approval_ready ? rec.approval_reasons.join(" ") : undefined}
                                                className={`flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg text-sm font-medium transition-colors ${
                                                    rec.approval_ready
                                                        ? "bg-green-500/10 text-green-500 hover:bg-green-500/20"
                                                        : "bg-white/5 text-muted-foreground cursor-not-allowed"
                                                }`}
                                            >
                                                <CheckCircle className="w-4 h-4" /> Approve
                                            </button>
                                            <button
                                                onClick={() => later.mutate(rec.id)}
                                                disabled={later.isPending}
                                                className="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg bg-sky-500/10 text-sky-400 hover:bg-sky-500/20 text-sm font-medium transition-colors"
                                            >
                                                Later
                                            </button>
                                            <button
                                                onClick={() => reject.mutate(rec.id)}
                                                disabled={reject.isPending}
                                                className="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg bg-red-500/10 text-red-500 hover:bg-red-500/20 text-sm font-medium transition-colors"
                                            >
                                                <XCircle className="w-4 h-4" /> Reject
                                            </button>
                                        </>
                                    )}
                                    {rec.status === 'later' && (
                                        <button
                                            onClick={() => restore.mutate(rec.id)}
                                            disabled={restore.isPending}
                                            className="w-full flex items-center justify-center gap-2 py-2 rounded-lg bg-sky-500/10 text-sky-300 hover:bg-sky-500/20 text-sm font-medium transition-colors"
                                        >
                                            Show Again
                                        </button>
                                    )}
                                    {rec.status === 'approved' && (
                                        (() => {
                                            const workflowUrl = resolveWorkflowUrl(rec.workflow_url, rec.workflow_id);
                                            if (!workflowUrl) {
                                                return (
                                                    <div className="w-full flex items-center justify-center gap-2 py-2 rounded-lg bg-white/5 text-muted-foreground text-sm font-medium border border-white/10">
                                                        <ExternalLink className="w-4 h-4" /> Preparing workflow...
                                                    </div>
                                                );
                                            }
                                            return (
                                                <a
                                                    href={workflowUrl}
                                                    target="_blank"
                                                    rel="noopener noreferrer"
                                                    className="w-full flex items-center justify-center gap-2 py-2 rounded-lg bg-primary/10 text-primary hover:bg-primary/20 text-sm font-medium transition-colors"
                                                >
                                                    <ExternalLink className="w-4 h-4" /> Open in n8n
                                                </a>
                                            );
                                        })()
                                    )}
                                </div>
                            </CardContent>
                        </Card>
                    ))}
                </div>
            ) : (
                <Card className="p-10 text-center">
                    <div className="text-muted-foreground mb-4">No workflows found for this category.</div>
                </Card>
            )}
        </div>
    );
}
