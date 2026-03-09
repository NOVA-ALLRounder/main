import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { fetchSelectionContext, runJudgment, scanProject } from "@/lib/api";
import type { ContextSelection, Judgment, ProjectScan } from "@/lib/types";
import { useState } from "react";

export function BetaActionsCard() {
    const [contextResult, setContextResult] = useState<ContextSelection | null>(null);
    const [contextLoading, setContextLoading] = useState(false);
    const [scanResult, setScanResult] = useState<ProjectScan | null>(null);
    const [scanLoading, setScanLoading] = useState(false);
    const [judgmentResult, setJudgmentResult] = useState<Judgment | null>(null);
    const [judgmentLoading, setJudgmentLoading] = useState(false);

    const handleGetContext = async () => {
        setContextLoading(true);
        try {
            const res = await fetchSelectionContext();
            setContextResult(res);
        } catch {
            setContextResult(null);
        } finally {
            setContextLoading(false);
        }
    };

    const handleScanProject = async () => {
        setScanLoading(true);
        try {
            const res = await scanProject(100);
            setScanResult(res);
        } catch {
            setScanResult(null);
        } finally {
            setScanLoading(false);
        }
    };

    const handleRunJudgment = async () => {
        setJudgmentLoading(true);
        try {
            const res = await runJudgment(undefined, 50);
            setJudgmentResult(res);
        } catch {
            setJudgmentResult(null);
        } finally {
            setJudgmentLoading(false);
        }
    };

    return (
        <Card className="h-auto mb-4 border-indigo-500/20 bg-indigo-500/5">
            <CardHeader>
                <CardTitle>Beta Features (Advanced)</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
                <div className="space-y-2">
                    <div className="text-xs text-muted-foreground">Context Awareness</div>
                    <button
                        onClick={handleGetContext}
                        disabled={contextLoading}
                        className="w-full text-[11px] py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        {contextLoading ? "Getting Selection..." : "Get Selected Text (macOS)"}
                    </button>
                    {contextResult && (
                        <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px]">
                            <div className={contextResult.found ? "text-emerald-300" : "text-rose-300"}>
                                {contextResult.found ? "Found Selection" : "No Selection / Error"}
                            </div>
                            {contextResult.text && (
                                <div className="mt-1 text-white/80 line-clamp-3 italic">"{contextResult.text}"</div>
                            )}
                            {contextResult.error && (
                                <div className="mt-1 text-rose-300">{contextResult.error}</div>
                            )}
                        </div>
                    )}
                </div>

                <div className="space-y-2">
                    <div className="text-xs text-muted-foreground">Project Scanner</div>
                    <button
                        onClick={handleScanProject}
                        disabled={scanLoading}
                        className="w-full text-[11px] py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        {scanLoading ? "Scanning..." : "Scan Current Project (Limit 100)"}
                    </button>
                    {scanResult && (
                        <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px] space-y-1">
                            <div>Type: <span className="text-indigo-300">{scanResult.project_type}</span></div>
                            <div className="text-muted-foreground">{scanResult.files.length} files found.</div>
                            {Object.keys(scanResult.key_files).length > 0 && (
                                <div className="mt-1 space-y-0.5">
                                    <div className="text-[10px] text-muted-foreground">Key Files:</div>
                                    {Object.entries(scanResult.key_files).slice(0, 3).map(([k, v]) => (
                                        <div key={k} className="flex justify-between">
                                            <span>{k}</span>
                                            <span className="text-white/60 truncate max-w-[100px]">{v}</span>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </div>
                    )}
                </div>

                <div className="space-y-2">
                    <div className="text-xs text-muted-foreground">OODA Loop Judgment</div>
                    <button
                        onClick={handleRunJudgment}
                        disabled={judgmentLoading}
                        className="w-full text-[11px] py-1.5 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        {judgmentLoading ? "Judging..." : "Run Judgment Logic"}
                    </button>
                    {judgmentResult && (
                        <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px] space-y-1">
                            <div className="flex justify-between">
                                <span>Status</span>
                                <span className={judgmentResult.status === "stop" || judgmentResult.status === "replan" ? "text-rose-300" : "text-emerald-300"}>
                                    {judgmentResult.status.toUpperCase()}
                                </span>
                            </div>
                            <div className="flex justify-between">
                                <span>Progress</span>
                                <span className={judgmentResult.no_progress ? "text-rose-300" : "text-emerald-300"}>
                                    {judgmentResult.no_progress ? "Stalled" : "Active"}
                                </span>
                            </div>
                            {judgmentResult.reasons.length > 0 && (
                                <div className="mt-1 text-rose-200">
                                    {judgmentResult.reasons.join(", ")}
                                </div>
                            )}
                        </div>
                    )}
                </div>
            </CardContent>
        </Card>
    );
}
