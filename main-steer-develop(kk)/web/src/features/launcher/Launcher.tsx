import { useEffect, useMemo, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Activity, Pin, Search, Terminal, Zap } from "lucide-react";
import {
    approveRecommendation,
    getRuntimeMode,
    getSystemPreflight,
    setEmergencyStop,
    setRuntimeMode,
    type RuntimeMode,
} from "@/lib/api";
import { useRecommendations } from "@/lib/hooks";
import { emit } from "@tauri-apps/api/event";
import { getAllWindows, getCurrentWindow } from "@tauri-apps/api/window";
import ReactMarkdown, { type Components } from "react-markdown";
import { useAgentWorkflow } from "./useAgentWorkflow";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

type WindowWithTauriMeta = Window & {
    __TAURI_METADATA__?: unknown;
    __TAURI__?: { metadata?: unknown };
    __TAURI_INTERNALS__?: { metadata?: unknown };
};

const markdownComponents: Components = {
    code({ children, ...props }) {
        const inline = "inline" in props && props.inline;
        return !inline ? (
            <div className="bg-black/50 p-2 rounded-md my-2 overflow-x-auto font-mono text-xs border border-white/10">
                <code {...props}>{children}</code>
            </div>
        ) : (
            <code className="bg-white/10 px-1 py-0.5 rounded font-mono text-xs" {...props}>
                {children}
            </code>
        );
    },
};

type EmailWorkflowReport = {
    summaryPath?: string;
    tasksPath?: string;
    notionLine?: string;
};

const parseEmailWorkflowReport = (content: string): EmailWorkflowReport | null => {
    if (!content.toLowerCase().includes("email workflow")) return null;
    const summaryMatch = content.match(/- Summary:\s*(.+)/i);
    const tasksMatch = content.match(/- Tasks CSV:\s*(.+)/i);
    const notionMatch = content.match(/- Notion[^\n]*/i);
    return {
        summaryPath: summaryMatch?.[1]?.trim(),
        tasksPath: tasksMatch?.[1]?.trim(),
        notionLine: notionMatch?.[0]?.trim(),
    };
};

export default function Launcher() {
    const [input, setInput] = useState("");
    const [selectedIndex, setSelectedIndex] = useState(0);
    const [successPulse, setSuccessPulse] = useState(false);
    const [shake, setShake] = useState(false);
    const [approvingIds, setApprovingIds] = useState<Set<number>>(new Set());
    const [approveErrors, setApproveErrors] = useState<Record<number, string>>({});
    const [approveCooldowns, setApproveCooldowns] = useState<Record<number, number>>({});
    const [showHelp, setShowHelp] = useState(false);
    const queryClient = useQueryClient();
    const inputRef = useRef<HTMLInputElement>(null);
    const scrollRef = useRef<HTMLDivElement>(null);
    const { data: recs, refetch } = useRecommendations();
    const { data: runtimeMode, isError: runtimeModeError } = useQuery({
        queryKey: ["runtimeModeLauncher"],
        queryFn: getRuntimeMode,
        refetchInterval: 3000,
        refetchIntervalInBackground: true,
    });
    const { data: preflight, isError: preflightError, isFetching: preflightFetching, refetch: refetchPreflight } = useQuery({
        queryKey: ["systemPreflightLauncher"],
        queryFn: getSystemPreflight,
        refetchInterval: 10000,
        refetchIntervalInBackground: true,
        retry: 2,
    });
    const modeMutation = useMutation({
        mutationFn: (mode: RuntimeMode["mode"]) => setRuntimeMode(mode),
        onSuccess: (data) => {
            queryClient.setQueryData(["runtimeModeLauncher"], data);
        },
    });
    const emergencyMutation = useMutation({
        mutationFn: (enabled: boolean) => setEmergencyStop(enabled),
        onSuccess: (data) => {
            queryClient.setQueryData(["runtimeModeLauncher"], data);
        },
    });
    const modeLabel =
        runtimeMode?.mode === "autopilot"
            ? "자동실행"
            : runtimeMode?.mode === "copilot"
                ? "코파일럿"
                : "관찰모드";
    const boolBadge = (value: boolean | undefined, isError = false) => {
        if (isError) {
            return { text: "error", className: "text-rose-300" };
        }
        if (value === undefined) {
            return { text: "loading", className: "text-gray-400" };
        }
        return value
            ? { text: "true", className: "text-emerald-300" }
            : { text: "false", className: "text-amber-300" };
    };

    useEffect(() => {
        inputRef.current?.focus();
    }, []);

    const triggerSuccess = () => {
        setSuccessPulse(true);
        setTimeout(() => setSuccessPulse(false), 1000);
    };

    const triggerError = () => {
        setShake(true);
        setTimeout(() => setShake(false), 500);
    };

    const {
        results,
        setResults,
        loading,
        pendingApproval,
        approvalBusy,
        lastPlanId,
        lastStatus,
        handleSend,
        handleResume,
        handleApprovalDecision,
    } = useAgentWorkflow({
        onSuccess: triggerSuccess,
        onError: triggerError,
    });

    const handlePin = async (content: string, title?: string) => {
        try {
            await emit("pin-data", {
                type: "text",
                content,
                title: title || "Pinned from Steer",
            });

            const windows = await getAllWindows();
            const widgetWin = windows.find((w) => w.label === "widget");
            if (widgetWin) {
                await widgetWin.show();
            }

            triggerSuccess();
        } catch (error) {
            console.error("Pin failed", error);
            triggerError();
        }
    };

    const pendingRecs = recs?.filter((r) => r.status === "pending") ?? [];
    const navigableItems = [
        ...results.map((r, i) => ({ type: "result", data: r, id: `res-${i}` })),
        ...pendingRecs.map((r) => ({ type: "recommendation", data: r, id: `rec-${r.id}` })),
    ];

    useEffect(() => {
        setSelectedIndex(0);
    }, [results, pendingRecs.length]);

    useEffect(() => {
        if (scrollRef.current && navigableItems.length > 0) {
            const selectedElement = scrollRef.current.children[selectedIndex];
            if (selectedElement) {
                selectedElement.scrollIntoView({ behavior: "smooth", block: "nearest" });
            }
        }
    }, [selectedIndex, navigableItems.length]);

    const runSend = async () => {
        if (!input.trim()) return;
        if (input.trim() === "test_perf") {
            const start = performance.now();
            const dummyItems = Array.from({ length: 1000 }, (_, i) => ({
                type: "response" as const,
                content: `**Perf Item #${i + 1}**: This is a dummy item to test rendering performance. ${Math.random()}`,
            }));
            const end = performance.now();
            setResults(dummyItems);
            setInput("");
            triggerSuccess();
            console.log(`[Perf] Generated 1000 items in ${(end - start).toFixed(2)}ms`);
            return;
        }

        await handleSend(input, () => setInput(""));
    };

    const extractErrorMessage = (error: unknown) => {
        if (typeof error === "string") return error;
        if (error && typeof error === "object") {
            const maybe = error as {
                message?: unknown;
                response?: { data?: { error?: unknown; details?: unknown } };
            };
            const responseError = maybe.response?.data?.error;
            const responseDetails = maybe.response?.data?.details;
            if (typeof responseError === "string" && typeof responseDetails === "string") {
                return `${responseError}: ${responseDetails}`;
            }
            if (typeof responseError === "string") return responseError;
            if (typeof maybe.message === "string") return maybe.message;
        }
        return "Approve failed";
    };

    const mapApproveError = (raw: string) => {
        const msg = raw.toLowerCase();
        if (msg.includes("unauthorized") || msg.includes("401")) {
            return "n8n API 인증 실패 (API 키 확인 필요)";
        }
        if (msg.includes("nodes") && msg.includes("empty")) {
            return "워크플로우 노드가 비어 있습니다. 프롬프트를 수정해 다시 시도하세요.";
        }
        if (msg.includes("timeout")) {
            return "요청 시간이 초과되었습니다. 잠시 후 다시 시도하세요.";
        }
        if (msg.includes("connection refused")) {
            return "코어 서버 연결 실패 (5680 포트 상태 확인 필요)";
        }
        if (msg.includes("n8n server unavailable") || msg.includes("failed to auto-start n8n")) {
            return "n8n 서버가 실행되지 않았습니다. n8n 설치/실행 후 다시 시도하세요.";
        }
        if (msg.includes("program not found")) {
            return "n8n 실행 명령을 찾지 못했습니다. Node.js/npx PATH를 확인하세요.";
        }
        return raw;
    };

    const handleApprove = async (id: number) => {
        const now = Date.now();
        const last = approveCooldowns[id] ?? 0;
        if (now - last < 3000) {
            return;
        }
        setApproveCooldowns((prev) => ({ ...prev, [id]: now }));
        setApproveErrors((prev) => {
            const next = { ...prev };
            delete next[id];
            return next;
        });
        setApprovingIds((prev) => new Set(prev).add(id));
        try {
            await approveRecommendation(id);
            triggerSuccess();
        } catch (e) {
            console.error("Approve failed", e);
            triggerError();
            const raw = extractErrorMessage(e);
            setApproveErrors((prev) => ({ ...prev, [id]: mapApproveError(raw) }));
        } finally {
            setApprovingIds((prev) => {
                const next = new Set(prev);
                next.delete(id);
                return next;
            });
            refetch();
        }
    };

    const handleKeyDown = async (e: React.KeyboardEvent) => {
        if (e.key === "ArrowDown") {
            e.preventDefault();
            setSelectedIndex((prev) => (prev + 1) % navigableItems.length);
        } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setSelectedIndex((prev) => (prev - 1 + navigableItems.length) % navigableItems.length);
        } else if (e.key === "Enter") {
            if (input.trim() && navigableItems.length === 0) {
                e.preventDefault();
                await runSend();
                return;
            }

            if (navigableItems.length > 0) {
                const selected = navigableItems[selectedIndex];
                if (selected && selected.type === "recommendation") {
                    e.preventDefault();
                    const rec = selected.data as { id: number };
                    await handleApprove(rec.id);
                }
            } else if (input.trim()) {
                await runSend();
            }
        }
    };

    const handleBackgroundClick = async (e: React.MouseEvent) => {
        if (e.target === e.currentTarget) {
            try {
                const tauriMeta =
                    (window as WindowWithTauriMeta).__TAURI_METADATA__ ||
                    (window as WindowWithTauriMeta).__TAURI__?.metadata ||
                    (window as WindowWithTauriMeta).__TAURI_INTERNALS__?.metadata;
                if (tauriMeta) {
                    await getCurrentWindow().hide();
                }
            } catch (error) {
                console.error("Failed to hide window:", error);
            }
        }
    };

    const copyText = async (text?: string) => {
        if (!text) return;
        try {
            await navigator.clipboard.writeText(text);
            triggerSuccess();
        } catch (error) {
            console.error("Clipboard copy failed:", error);
            triggerError();
        }
    };


    const timelineItems = useMemo(() => {
        const items: { title: string; detail: string; tone: "info" | "ok" | "warn" | "error" }[] = [];
        items.push({
            title: "Runtime",
            detail: `mode=${runtimeMode?.mode ?? "unknown"} e-stop=${runtimeMode?.emergency_stop ? "on" : "off"} auto=${runtimeMode?.allow_automation ? "yes" : "no"}`,
            tone: runtimeMode?.allow_automation ? "ok" : "warn",
        });
        if (preflight) {
            items.push({
                title: "Preflight",
                detail: preflight.ok ? "all checks passed" : (preflight.notes?.join(" | ") || "warnings found"),
                tone: preflight.ok ? "ok" : "warn",
            });
        }
        if (pendingApproval) {
            items.push({
                title: "Approval Required",
                detail: `${pendingApproval.action} (${pendingApproval.riskLevel})`,
                tone: "warn",
            });
        }
        if (lastStatus) {
            items.push({
                title: "Execution Status",
                detail: lastStatus,
                tone: lastStatus.includes("fail") || lastStatus.includes("blocked") ? "error" : "info",
            });
        }
        const recent = results.slice(-6).reverse().map((r) => ({
            title: r.type === "error" ? "Result Error" : "Result",
            detail: r.content.replace(/\s+/g, " ").slice(0, 120),
            tone: (r.type === "error" ? "error" : "info") as "error" | "info",
        }));
        return [...items, ...recent];
    }, [runtimeMode, preflight, pendingApproval, lastStatus, results]);

    const toneClass = (tone: "info" | "ok" | "warn" | "error") =>
        tone === "ok"
            ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-200"
            : tone === "warn"
                ? "border-amber-500/30 bg-amber-500/10 text-amber-100"
                : tone === "error"
                    ? "border-rose-500/30 bg-rose-500/10 text-rose-200"
                    : "border-white/10 bg-white/5 text-gray-200";    return (
        <div className="fixed inset-0 bg-transparent flex items-start justify-center pt-[20vh] p-8" onMouseDown={handleBackgroundClick}>
            <motion.div
                className={`w-full max-w-2xl bg-[#1e1e1e]/95 backdrop-blur-2xl rounded-2xl shadow-2xl overflow-hidden border transition-colors duration-500
                    ${successPulse ? "border-green-500/50 shadow-green-500/20" : "border-white/10 ring-1 ring-black/5"}`}
                initial={{ scale: 0.9, opacity: 0 }}
                animate={{ scale: 1, opacity: 1, x: shake ? [0, -10, 10, -10, 10, 0] : 0 }}
                transition={{ type: "spring", duration: 0.3 }}
            >
                <div className="flex items-center px-4 py-4 border-b border-white/5 bg-[#1e1e1e]">
                    <Search className="w-5 h-5 text-gray-400 mr-3" />
                    <input
                        ref={inputRef}
                        type="text"
                        className="flex-1 bg-transparent border-none outline-none text-lg text-white placeholder-gray-500 font-medium"
                        placeholder="무엇이든 물어보세요 (Ask anything...)"
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        onKeyDown={handleKeyDown}
                        autoFocus
                    />
                    {loading && <Activity className="w-5 h-5 text-blue-500 animate-spin" />}
                    {!loading && (
                        <button
                            onClick={runSend}
                            disabled={!input.trim()}
                            className="ml-2 text-xs text-gray-200 bg-white/10 hover:bg-white/20 px-2 py-1 rounded disabled:opacity-50"
                        >
                            Send
                        </button>
                    )}
                </div>

                <div className="px-4 py-3 border-b border-white/5 bg-[#1b1b1b]">
                    <div className="flex items-center justify-between mb-2">
                        <div className="text-[11px] uppercase tracking-wider text-gray-400 font-semibold">Control Center</div>
                        <div className="flex items-center gap-2">
                            <button
                                onClick={() => refetchPreflight()}
                                className="text-[10px] px-2 py-0.5 rounded bg-white/10 hover:bg-white/20 text-gray-300"
                            >
                                refresh
                            </button>
                            <div className={`text-[11px] font-mono ${preflightError ? "text-rose-300" : preflight?.ok ? "text-emerald-300" : preflight === undefined ? "text-gray-400" : "text-amber-300"}`}>
                                preflight: {preflightError ? "ERROR" : preflight === undefined ? "loading" : preflight.ok ? "PASS" : "WARN"}{preflightFetching ? "…" : ""}
                            </div>
                        </div>
                    </div>
                    <div className="flex flex-wrap gap-2 mb-2">
                        {(["observe", "copilot", "autopilot"] as const).map((mode) => {
                            const active = (runtimeMode?.mode ?? "autopilot") === mode;
                            return (
                                <button
                                    key={mode}
                                    onClick={() => modeMutation.mutate(mode)}
                                    disabled={modeMutation.isPending}
                                    className={`text-[11px] px-2.5 py-1 rounded border transition-colors ${
                                        active
                                            ? "bg-blue-500/20 border-blue-400/60 text-blue-200"
                                            : "bg-white/5 border-white/10 text-gray-300 hover:bg-white/10"
                                    }`}
                                >
                                    mode:{mode}
                                </button>
                            );
                        })}
                        <button
                            onClick={() => emergencyMutation.mutate(!(runtimeMode?.emergency_stop ?? false))}
                            disabled={emergencyMutation.isPending}
                            className={`text-[11px] px-2.5 py-1 rounded border transition-colors ${
                                runtimeMode?.emergency_stop
                                    ? "bg-rose-500/20 border-rose-400/60 text-rose-200"
                                    : "bg-emerald-500/20 border-emerald-400/60 text-emerald-200"
                            }`}
                        >
                            e-stop:{runtimeMode?.emergency_stop ? "ON" : "OFF"}
                        </button>
                    </div>
                    <div className="grid grid-cols-2 gap-2 text-[11px] text-gray-300">
                        <div className="rounded border border-white/10 bg-white/5 px-2 py-1.5">
                            Gmail: <span className={boolBadge(preflight?.gmail_credentials_set, preflightError).className}>{boolBadge(preflight?.gmail_credentials_set, preflightError).text}</span>
                        </div>
                        <div className="rounded border border-white/10 bg-white/5 px-2 py-1.5">
                            Notion: <span className={boolBadge(preflight?.notion_ready, preflightError).className}>{boolBadge(preflight?.notion_ready, preflightError).text}</span>
                        </div>
                    </div>
                </div>

                <div className="px-4 py-2 border-b border-white/5 bg-[#171717] flex flex-wrap gap-2">
                    <span className="text-[10px] px-2 py-1 rounded border border-white/15 bg-white/5 text-gray-300">status:{loading ? "running" : "idle"}</span>
                    <span className={`text-[10px] px-2 py-1 rounded border ${runtimeModeError ? "border-rose-500/40 bg-rose-500/10 text-rose-200" : runtimeMode?.allow_automation ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-200" : "border-amber-500/40 bg-amber-500/10 text-amber-200"}`}>automation:{runtimeModeError ? "error" : runtimeMode?.allow_automation ? "enabled" : "blocked"}</span>
                    <span className={`text-[10px] px-2 py-1 rounded border ${pendingApproval ? "border-amber-500/40 bg-amber-500/10 text-amber-100" : "border-white/15 bg-white/5 text-gray-400"}`}>approval:{pendingApproval ? "pending" : "none"}</span>
                    <span className={`text-[10px] px-2 py-1 rounded border ${lastStatus ? "border-blue-500/40 bg-blue-500/10 text-blue-200" : "border-white/15 bg-white/5 text-gray-400"}`}>exec:{lastStatus ?? "n/a"}</span>
                </div>

                {pendingApproval && (
                    <div className="px-4 py-3 border-b border-white/5 bg-[#1b1b1b]">
                        <div className="text-[11px] uppercase tracking-wider text-amber-400 font-semibold">Approval Required</div>
                        <div className="text-sm text-gray-200 mt-1">Action: <span className="font-mono">{pendingApproval.action}</span></div>
                        <div className="text-xs text-gray-400">Risk: {pendingApproval.riskLevel} • Policy: {pendingApproval.policy}</div>
                        <div className="text-xs text-gray-500 mt-1">{pendingApproval.message}</div>
                        <div className="mt-3 flex flex-wrap gap-2">
                            <button disabled={approvalBusy} onClick={() => handleApprovalDecision("allow_once")} className="text-xs px-3 py-1.5 rounded bg-emerald-500/20 text-emerald-200 border border-emerald-500/40 hover:bg-emerald-500/30 disabled:opacity-50">Approve once</button>
                            <button disabled={approvalBusy} onClick={() => handleApprovalDecision("allow_always")} className="text-xs px-3 py-1.5 rounded bg-blue-500/20 text-blue-200 border border-blue-500/40 hover:bg-blue-500/30 disabled:opacity-50">Allow always</button>
                            <button disabled={approvalBusy} onClick={() => handleApprovalDecision("deny")} className="text-xs px-3 py-1.5 rounded bg-rose-500/20 text-rose-200 border border-rose-500/40 hover:bg-rose-500/30 disabled:opacity-50">Deny</button>
                        </div>
                    </div>
                )}

                {lastStatus === "manual_required" && lastPlanId && !pendingApproval && (
                    <div className="px-4 py-3 border-b border-white/5 bg-[#171717]">
                        <div className="text-[11px] uppercase tracking-wider text-sky-400 font-semibold">Manual Step Needed</div>
                        <div className="text-xs text-gray-400 mt-1">브라우저에서 수동 작업을 완료한 후 Resume를 눌러 다음 단계로 진행하세요</div>
                        <div className="mt-3">
                            <button disabled={loading} onClick={handleResume} className="text-xs px-3 py-1.5 rounded bg-sky-500/20 text-sky-200 border border-sky-500/40 hover:bg-sky-500/30 disabled:opacity-50">Resume</button>
                        </div>
                    </div>
                )}

                <div ref={scrollRef} className="bg-[#191919] min-h-[300px] max-h-[500px] overflow-y-auto">
                    <AnimatePresence>
                        {results.map((res, i) => {
                            const isSelected = navigableItems.findIndex((x) => x.id === `res-${i}`) === selectedIndex;
                            const emailReport = parseEmailWorkflowReport(res.content);
                            return (
                                <motion.div key={i} initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} className={`p-4 rounded-lg mb-2 text-gray-200 text-sm leading-relaxed transition-colors relative group ${isSelected ? "bg-white/10" : "bg-[#2a2a2a]"}`}>
                                    {emailReport && (
                                        <div className="mb-3 rounded-md border border-emerald-500/30 bg-emerald-500/10 p-3">
                                            <div className="text-xs uppercase tracking-wide text-emerald-300 font-semibold mb-2">Email Workflow Report</div>
                                            {emailReport.summaryPath && (
                                                <div className="mb-1 text-xs text-gray-200 break-all">
                                                    Summary: {emailReport.summaryPath}
                                                    <button
                                                        onClick={() => copyText(emailReport.summaryPath)}
                                                        className="ml-2 px-2 py-0.5 rounded bg-white/10 hover:bg-white/20 text-[10px]"
                                                    >
                                                        Copy
                                                    </button>
                                                </div>
                                            )}
                                            {emailReport.tasksPath && (
                                                <div className="mb-1 text-xs text-gray-200 break-all">
                                                    Tasks CSV: {emailReport.tasksPath}
                                                    <button
                                                        onClick={() => copyText(emailReport.tasksPath)}
                                                        className="ml-2 px-2 py-0.5 rounded bg-white/10 hover:bg-white/20 text-[10px]"
                                                    >
                                                        Copy
                                                    </button>
                                                </div>
                                            )}
                                            {emailReport.notionLine && (
                                                <div className="text-xs text-amber-200">{emailReport.notionLine}</div>
                                            )}
                                        </div>
                                    )}
                                    <ReactMarkdown components={markdownComponents}>{res.content}</ReactMarkdown>
                                    <button onClick={() => handlePin(res.content)} className="absolute top-2 right-2 p-1.5 rounded-md text-gray-400 hover:text-white hover:bg-white/10 opacity-0 group-hover:opacity-100 transition-all" title="Pin to Widget">
                                        <Pin className="w-4 h-4" />
                                    </button>
                                </motion.div>
                            );
                        })}
                    </AnimatePresence>

                    {pendingRecs.length > 0 && (
                        <div className="p-2">
                            <div className="px-2 py-1 text-xs font-semibold text-gray-500 uppercase tracking-wider mb-1">Suggestions</div>
                            {pendingRecs.map((rec, idx) => {
                                const isSel = navigableItems[selectedIndex]?.id === `rec-${rec.id}`;
                                return (
                                    <div key={rec.id} className={`group flex items-center justify-between px-3 py-2 rounded-md cursor-pointer transition-all ${isSel ? "bg-blue-500/20 border border-blue-500/30" : "hover:bg-white/5 border border-transparent"}`} onClick={() => {
                                        const navIndex = navigableItems.findIndex((x) => x.id === `rec-${rec.id}`);
                                        if (navIndex >= 0) {
                                            setSelectedIndex(navIndex);
                                        } else {
                                            setSelectedIndex(idx);
                                        }
                                    }}>
                                        <div className="flex items-center gap-3">
                                            <div className={`w-8 h-8 rounded flex items-center justify-center ${isSel ? "bg-blue-500 text-white" : "bg-white/10 text-gray-400"}`}>
                                                <Zap className="w-4 h-4" />
                                            </div>
                                            <div>
                                                <div className={`text-sm font-medium ${isSel ? "text-blue-100" : "text-gray-200"}`}>{rec.title}</div>
                                                <div className="text-xs text-gray-500 line-clamp-1">{rec.summary}</div>
                                                {approveErrors[rec.id] && (
                                                    <div className="mt-1 text-[10px] text-rose-300">{approveErrors[rec.id]}</div>
                                                )}
                                            </div>
                                        </div>

                                        <div className="flex items-center gap-2">
                                            <button onClick={(e) => {
                                                e.stopPropagation();
                                                handlePin(rec.summary, rec.title);
                                            }} className="p-1.5 rounded-md text-gray-400 hover:text-white hover:bg-white/10 opacity-0 group-hover:opacity-100 transition-all" title="Pin to Widget">
                                                <Pin className="w-3 h-3" />
                                            </button>

                                            <button onClick={(e) => {
                                                e.stopPropagation();
                                                handleApprove(rec.id);
                                            }} disabled={approvingIds.has(rec.id)} className={`text-xs px-3 py-1.5 rounded transition-colors border ${isSel ? "bg-blue-500 text-white border-blue-400" : "text-gray-200 bg-white/10 border-white/10 hover:bg-white/20"} ${approvingIds.has(rec.id) ? "opacity-60 cursor-wait" : ""}`}>
                                                {approvingIds.has(rec.id) ? "Approving..." : approveErrors[rec.id] ? "Retry" : "Approve"}
                                            </button>

                                            <div className="text-[10px] text-gray-500 bg-white/5 px-2 py-1 rounded">Enter</div>
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}

                    {results.length === 0 && pendingRecs.length === 0 && (
                        <div className="p-8 text-center text-gray-500">
                            <Terminal className="w-12 h-12 mx-auto mb-3 opacity-20" />
                            <p className="text-sm">Type a command or chat with your agent.</p>
                            <div className="mt-4 flex flex-wrap justify-center gap-2">
                                <button onClick={() => setInput("오늘 우선순위 3개 정리해줘")} className="text-xs bg-white/5 px-2 py-1 rounded hover:bg-white/10 cursor-pointer">오늘 우선순위 정리</button>
                                <button onClick={() => setInput("email workflow run")} className="text-xs bg-white/5 px-2 py-1 rounded hover:bg-white/10 cursor-pointer">이메일 워크플로우 실행</button>
                                <button onClick={() => setInput("system status")} className="text-xs bg-white/5 px-2 py-1 rounded hover:bg-white/10 cursor-pointer">시스템 상태</button>
                            </div>
                        </div>
                    )}
                </div>

                <div className="px-4 py-3 border-t border-white/5 bg-[#161616]">
                    <div className="text-[11px] uppercase tracking-wider text-gray-400 font-semibold mb-2">Execution Timeline</div>
                    <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
                        {timelineItems.length === 0 ? (
                            <div className="text-xs text-gray-500">No activity yet.</div>
                        ) : (
                            timelineItems.map((item, idx) => (
                                <div key={`${item.title}-${idx}`} className={`rounded-md border px-2.5 py-2 text-xs ${toneClass(item.tone)}`}>
                                    <div className="font-semibold mb-1">{item.title}</div>
                                    <div className="opacity-90 break-words">{item.detail}</div>
                                </div>
                            ))
                        )}
                    </div>
                </div>

                <div className="px-4 py-2 border-t border-white/5 bg-[#1e1e1e] flex items-center justify-between">
                    <div className="flex items-center gap-2">
                        <div className={`w-2 h-2 rounded-full shadow-lg ${runtimeMode?.allow_automation ? "bg-emerald-500 shadow-emerald-500/50" : "bg-amber-500 shadow-amber-500/50"}`}></div>
                        <span className="text-xs text-gray-500">
                            Engine Active · {modeLabel} · 긴급정지={runtimeMode?.emergency_stop ? "ON" : "OFF"}
                        </span>
                    </div>
                    <div className="flex gap-4 text-xs text-gray-600">
                        <button
                            onClick={() => {
                                setInput("system status");
                                inputRef.current?.focus();
                            }}
                            className="hover:text-gray-400 cursor-pointer"
                        >
                            Settings
                        </button>
                        <button
                            onClick={() => setShowHelp((v) => !v)}
                            className="hover:text-gray-400 cursor-pointer"
                        >
                            Help
                        </button>
                    </div>
                </div>

                {showHelp && (
                    <div className="px-4 py-3 border-t border-white/5 bg-[#181818] text-xs text-gray-300">
                        <div className="font-semibold text-gray-200 mb-2">Quick Help</div>
                        <div className="flex flex-wrap gap-2">
                            <button onClick={() => setInput("오늘 우선순위 3개 정리해줘")} className="px-2 py-1 rounded bg-white/10 hover:bg-white/20">자유대화</button>
                            <button onClick={() => setInput("email workflow run")} className="px-2 py-1 rounded bg-white/10 hover:bg-white/20">이메일 워크플로우</button>
                            <button onClick={() => setInput("system status")} className="px-2 py-1 rounded bg-white/10 hover:bg-white/20">시스템 상태</button>
                            <button onClick={() => setInput("open notepad")} className="px-2 py-1 rounded bg-white/10 hover:bg-white/20">메모장 열기</button>
                        </div>
                    </div>
                )}
            </motion.div>
        </div>
    );
}








