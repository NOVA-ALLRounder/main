import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
    agentApprove,
    agentExecute,
    agentIntent,
    agentPlan,
    agentVerify,
    fetchTaskRunAssertions,
    fetchTaskRunStages,
    removeApprovalPolicy,
} from "@/lib/api";
import { useApprovalPolicies, useNlRunMetrics, useNlRuns } from "@/lib/hooks";
import type { ExecutionProfile, TaskStageAssertion, TaskStageRun } from "@/lib/types";
import { format } from "date-fns";
import { useState } from "react";

export function NaturalLanguageAutomationCard() {
    const { data: nlRuns } = useNlRuns(10);
    const { data: nlMetrics } = useNlRunMetrics(50);
    const { data: approvalPolicies, refetch: refetchApprovalPolicies } = useApprovalPolicies(10);
    const [prompt, setPrompt] = useState("");
    const [slotsInput, setSlotsInput] = useState("");
    const [sessionId, setSessionId] = useState<string | null>(null);
    const [planId, setPlanId] = useState<string | null>(null);
    const [intent, setIntent] = useState<string | null>(null);
    const [confidence, setConfidence] = useState<number | null>(null);
    const [missingSlots, setMissingSlots] = useState<string[]>([]);
    const [followUp, setFollowUp] = useState<string | null>(null);
    const [planSteps, setPlanSteps] = useState<string[]>([]);
    const [execStatus, setExecStatus] = useState<string | null>(null);
    const [execLogs, setExecLogs] = useState<string[]>([]);
    const [verifyStatus, setVerifyStatus] = useState<string | null>(null);
    const [verifyIssues, setVerifyIssues] = useState<string[]>([]);
    const [approveAction, setApproveAction] = useState("open_booking_link");
    const [approvalDecision, setApprovalDecision] = useState("allow_once");
    const [approvalRisk, setApprovalRisk] = useState<string | null>(null);
    const [approvalPolicy, setApprovalPolicy] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);
    const [history, setHistory] = useState<Array<{ time: string; prompt: string; status: string }>>([]);
    const [summary, setSummary] = useState<string | null>(null);
    const [approvalHistory, setApprovalHistory] = useState<Array<{ time: string; action: string; result: string }>>([]);
    const [historyFilter, setHistoryFilter] = useState("");
    const [executionProfile, setExecutionProfile] = useState<ExecutionProfile>("strict");
    const [lastRunId, setLastRunId] = useState<string | null>(null);
    const [stageRuns, setStageRuns] = useState<TaskStageRun[]>([]);
    const [stageAssertions, setStageAssertions] = useState<TaskStageAssertion[]>([]);

    const loadRunDiagnostics = async (runId?: string | null) => {
        if (!runId) {
            setStageRuns([]);
            setStageAssertions([]);
            return;
        }
        try {
            const [stages, assertions] = await Promise.all([
                fetchTaskRunStages(runId),
                fetchTaskRunAssertions(runId),
            ]);
            setLastRunId(runId);
            setStageRuns(stages);
            setStageAssertions(assertions);
        } catch (error) {
            console.error("Failed to load dashboard run diagnostics", error);
            setLastRunId(runId);
            setStageRuns([]);
            setStageAssertions([]);
        }
    };

    const parseSlots = (raw: string): Record<string, string> | undefined => {
        if (!raw.trim()) return undefined;
        const parsed = JSON.parse(raw) as Record<string, unknown>;
        const normalized: Record<string, string> = {};
        for (const [key, value] of Object.entries(parsed)) {
            if (value === null || value === undefined) continue;
            normalized[key] = String(value);
        }
        return normalized;
    };

    const handleIntent = async () => {
        if (!prompt.trim()) return;
        setLoading(true);
        setError(null);
        try {
            const res = await agentIntent(prompt.trim());
            setSessionId(res.session_id);
            setIntent(res.intent);
            setConfidence(res.confidence);
            setMissingSlots(res.missing_slots);
            setFollowUp(res.follow_up ?? null);
            setExecStatus(null);
            setExecLogs([]);
            setVerifyStatus(null);
            setVerifyIssues([]);
            setSummary(null);
        } catch {
            setError("Failed to parse intent.");
        } finally {
            setLoading(false);
        }
    };

    const handlePlan = async () => {
        if (!sessionId) {
            setError("Run intent first.");
            return;
        }
        setLoading(true);
        setError(null);
        try {
            let slots: Record<string, string> | undefined;
            try {
                slots = parseSlots(slotsInput);
            } catch {
                setError("Slots JSON is invalid.");
                return;
            }
            const res = await agentPlan(sessionId, slots);
            setPlanId(res.plan_id);
            setPlanSteps(res.steps.map((step) => `${step.step_type}: ${step.description}`));
            setMissingSlots(res.missing_slots);
        } catch {
            setError("Failed to build plan.");
        } finally {
            setLoading(false);
        }
    };

    const handleExecute = async () => {
        if (!planId) {
            setError("Build plan first.");
            return;
        }
        setLoading(true);
        setError(null);
        try {
            const res = await agentExecute(planId, executionProfile);
            setExecStatus(res.status);
            setExecLogs(res.logs);
            setLastRunId(res.run_id ?? null);
            const summaryLine = res.logs.find((line) => line.startsWith("Summary: "));
            setSummary(summaryLine ? summaryLine.replace("Summary: ", "") : null);
            await loadRunDiagnostics(res.run_id);
            setHistory((prev) => [
                { time: format(new Date(), "HH:mm:ss"), prompt: prompt || "(no prompt)", status: res.status },
                ...prev,
            ].slice(0, 5));
        } catch {
            setError("Execution failed.");
        } finally {
            setLoading(false);
        }
    };

    const handleVerify = async () => {
        if (!planId) {
            setError("Build plan first.");
            return;
        }
        setLoading(true);
        setError(null);
        try {
            const res = await agentVerify(planId);
            setVerifyStatus(res.ok ? "OK" : "FAIL");
            setVerifyIssues(res.issues);
        } catch {
            setError("Verification failed.");
        } finally {
            setLoading(false);
        }
    };

    const handleApprove = async () => {
        if (!planId) {
            setError("Build plan first.");
            return;
        }
        if (!approveAction.trim()) {
            setError("Approval action is required.");
            return;
        }
        setLoading(true);
        setError(null);
        try {
            const res = await agentApprove(planId, approveAction.trim(), approvalDecision);
            setExecStatus(res.requires_approval ? "approval_required" : res.status);
            setExecLogs((prev) => [...prev, res.message]);
            setApprovalRisk(res.risk_level);
            setApprovalPolicy(res.policy);
            setApprovalHistory((prev) => [
                {
                    time: format(new Date(), "HH:mm:ss"),
                    action: `${approveAction.trim()} (${approvalDecision})`,
                    result: res.status,
                },
                ...prev,
            ].slice(0, 5));
            if (approvalDecision === "allow_always" || approvalDecision === "deny") {
                await refetchApprovalPolicies();
            }
        } catch {
            setError("Approval failed.");
        } finally {
            setLoading(false);
        }
    };

    const handleRemovePolicy = async (policyKey: string) => {
        try {
            await removeApprovalPolicy(policyKey);
            await refetchApprovalPolicies();
        } catch {
            setError("Failed to remove approval policy.");
        }
    };

    const handleRunAll = async () => {
        if (!prompt.trim()) return;
        setLoading(true);
        setError(null);
        try {
            const intentRes = await agentIntent(prompt.trim());
            setSessionId(intentRes.session_id);
            setIntent(intentRes.intent);
            setConfidence(intentRes.confidence);
            setMissingSlots(intentRes.missing_slots);
            setFollowUp(intentRes.follow_up ?? null);

            let slots: Record<string, string> | undefined;
            try {
                slots = parseSlots(slotsInput);
            } catch {
                setError("Slots JSON is invalid.");
                return;
            }

            const planRes = await agentPlan(intentRes.session_id, slots);
            setPlanId(planRes.plan_id);
            setPlanSteps(planRes.steps.map((step) => `${step.step_type}: ${step.description}`));
            setMissingSlots(planRes.missing_slots);

            const execRes = await agentExecute(planRes.plan_id, executionProfile);
            setExecStatus(execRes.status);
            setExecLogs(execRes.logs);
            setLastRunId(execRes.run_id ?? null);
            const summaryLine = execRes.logs.find((line) => line.startsWith("Summary: "));
            setSummary(summaryLine ? summaryLine.replace("Summary: ", "") : null);
            await loadRunDiagnostics(execRes.run_id);
            setHistory((prev) => [
                { time: format(new Date(), "HH:mm:ss"), prompt: prompt || "(no prompt)", status: execRes.status },
                ...prev,
            ].slice(0, 5));

            const verifyRes = await agentVerify(planRes.plan_id);
            setVerifyStatus(verifyRes.ok ? "OK" : "FAIL");
            setVerifyIssues(verifyRes.issues);
        } catch {
            setError("Run all failed.");
        } finally {
            setLoading(false);
        }
    };

    const handleReset = () => {
        setPrompt("");
        setSlotsInput("");
        setSessionId(null);
        setPlanId(null);
        setIntent(null);
        setConfidence(null);
        setMissingSlots([]);
        setFollowUp(null);
        setPlanSteps([]);
        setExecStatus(null);
        setExecLogs([]);
        setVerifyStatus(null);
        setVerifyIssues([]);
        setSummary(null);
        setLastRunId(null);
        setStageRuns([]);
        setStageAssertions([]);
        setError(null);
        setApprovalHistory([]);
        setApprovalRisk(null);
        setApprovalPolicy(null);
    };

    const failureSummary = (() => {
        const issues = new Set<string>();
        const logText = execLogs.join(" ").toLowerCase();
        if (execStatus === "manual_required") issues.add("수동 입력 필요");
        if (execStatus === "approval_required") issues.add("승인 대기");
        if (execStatus === "blocked") issues.add("정책 차단");
        if (logText.includes("search button not found")) issues.add("검색 버튼 미탐지");
        if (logText.includes("auto fill skipped")) issues.add("필드 매칭 실패");
        if (logText.includes("auto fill failed")) issues.add("자동 입력 실패");
        if (logText.includes("failed to open url")) issues.add("페이지 열기 실패");
        if (verifyStatus === "FAIL") issues.add("검증 실패");
        verifyIssues.forEach((issue) => issues.add(issue));
        return Array.from(issues);
    })();

    const timeline = [
        { label: "Intent", status: sessionId ? "done" : "idle" },
        { label: "Plan", status: planId ? "done" : sessionId ? "pending" : "idle" },
        {
            label: "Execute",
            status: execStatus
                ? execStatus === "completed"
                    ? "done"
                    : execStatus === "approval_required" || execStatus === "manual_required"
                        ? "pending"
                        : "blocked"
                : planId
                    ? "pending"
                    : "idle",
        },
        {
            label: "Verify",
            status: verifyStatus ? (verifyStatus === "OK" ? "done" : "blocked") : execStatus ? "pending" : "idle",
        },
        {
            label: "Approve",
            status: approvalHistory.length > 0 ? "done" : execStatus === "approval_required" ? "pending" : "idle",
        },
    ];

    const statusClass = (status: string) => {
        if (status === "done") return "bg-emerald-500/20 text-emerald-200 border-emerald-400/30";
        if (status === "blocked") return "bg-rose-500/20 text-rose-200 border-rose-400/30";
        if (status === "pending") return "bg-amber-500/20 text-amber-200 border-amber-400/30";
        return "bg-white/5 text-white/60 border-white/10";
    };

    const riskClass = (risk: string | null) => {
        if (!risk) return "text-white/60";
        if (risk === "high") return "text-rose-200";
        if (risk === "medium") return "text-amber-200";
        return "text-emerald-200";
    };

    return (
        <Card className="h-auto mb-4 border-indigo-500/20 bg-indigo-500/5">
            <CardHeader>
                <CardTitle>Natural Language Automation</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
                <input
                    value={prompt}
                    onChange={(e) => setPrompt(e.target.value)}
                    placeholder="예: 3월 10일 서울에서 도쿄 왕복 30만원 이하 항공권 찾아줘"
                    className="w-full rounded-md bg-white/5 border border-white/10 px-3 py-2 text-sm"
                />
                <textarea
                    value={slotsInput}
                    onChange={(e) => setSlotsInput(e.target.value)}
                    placeholder='Slots JSON (optional) e.g. {"from":"ICN","to":"NRT","date_start":"2025-03-10"}'
                    rows={2}
                    className="w-full rounded-md bg-white/5 border border-white/10 px-3 py-2 text-xs"
                />
                <div className="grid grid-cols-3 gap-2">
                    <button
                        onClick={() => setExecutionProfile("strict")}
                        className={`text-[11px] py-1.5 rounded border transition-colors ${
                            executionProfile === "strict"
                                ? "bg-white/20 border-white/30 text-white"
                                : "bg-white/5 border-white/10 text-white/70 hover:bg-white/10"
                        }`}
                    >
                        정확(strict)
                    </button>
                    <button
                        onClick={() => setExecutionProfile("test")}
                        className={`text-[11px] py-1.5 rounded border transition-colors ${
                            executionProfile === "test"
                                ? "bg-white/20 border-white/30 text-white"
                                : "bg-white/5 border-white/10 text-white/70 hover:bg-white/10"
                        }`}
                    >
                        테스트(test)
                    </button>
                    <button
                        onClick={() => setExecutionProfile("fast")}
                        className={`text-[11px] py-1.5 rounded border transition-colors ${
                            executionProfile === "fast"
                                ? "bg-white/20 border-white/30 text-white"
                                : "bg-white/5 border-white/10 text-white/70 hover:bg-white/10"
                        }`}
                    >
                        빠름(fast)
                    </button>
                </div>
                <div className="grid grid-cols-2 gap-2">
                    <button
                        onClick={handleIntent}
                        disabled={loading}
                        className="text-xs py-2 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        Intent
                    </button>
                    <button
                        onClick={handlePlan}
                        disabled={loading}
                        className="text-xs py-2 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        Plan
                    </button>
                    <button
                        onClick={handleExecute}
                        disabled={loading}
                        className="text-xs py-2 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        Execute
                    </button>
                    <button
                        onClick={handleVerify}
                        disabled={loading}
                        className="text-xs py-2 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        Verify
                    </button>
                </div>
                <div className="grid grid-cols-2 gap-2">
                    <button
                        onClick={handleRunAll}
                        disabled={loading}
                        className="text-xs py-2 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        Run All
                    </button>
                    <button
                        onClick={handleReset}
                        className="text-xs py-2 rounded bg-white/5 hover:bg-white/10 transition-colors"
                    >
                        Reset
                    </button>
                </div>
                <div className="grid grid-cols-2 gap-2">
                    <button
                        onClick={handleExecute}
                        disabled={loading || !planId}
                        className="text-[11px] py-2 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        Retry Execute
                    </button>
                    <button
                        onClick={() => setExecLogs([])}
                        className="text-[11px] py-2 rounded bg-white/5 hover:bg-white/10 transition-colors"
                    >
                        Clear Logs
                    </button>
                </div>
                <div className="flex gap-2">
                    <select
                        value={approvalDecision}
                        onChange={(e) => setApprovalDecision(e.target.value)}
                        className="rounded-md bg-white/5 border border-white/10 px-2 py-1 text-[11px]"
                    >
                        <option value="allow_once">Allow once</option>
                        <option value="allow_always">Allow always</option>
                        <option value="deny">Deny</option>
                    </select>
                    <input
                        value={approveAction}
                        onChange={(e) => setApproveAction(e.target.value)}
                        placeholder="Approval action"
                        className="flex-1 rounded-md bg-white/5 border border-white/10 px-2 py-1 text-xs"
                    />
                    <button
                        onClick={handleApprove}
                        disabled={loading}
                        className="text-xs px-3 py-1 rounded bg-white/10 hover:bg-white/20 transition-colors disabled:opacity-50"
                    >
                        Approve
                    </button>
                </div>
                {error && <div className="text-xs text-rose-200">{error}</div>}
                {(approvalRisk || approvalPolicy) && (
                    <div className="text-[11px] text-muted-foreground">
                        Approval risk: <span className={riskClass(approvalRisk)}>{approvalRisk ?? "unknown"}</span>
                        {approvalPolicy ? ` · Policy: ${approvalPolicy}` : ""}
                    </div>
                )}
                {nlMetrics && (
                    <div className="grid grid-cols-3 gap-2 text-[11px]">
                        <div className="rounded-md border border-white/10 bg-white/5 p-2">
                            <div className="text-muted-foreground">Success rate</div>
                            <div className="text-white/80">{nlMetrics.success_rate.toFixed(0)}%</div>
                        </div>
                        <div className="rounded-md border border-white/10 bg-white/5 p-2">
                            <div className="text-muted-foreground">Completed</div>
                            <div className="text-white/80">{nlMetrics.completed}/{nlMetrics.total}</div>
                        </div>
                        <div className="rounded-md border border-white/10 bg-white/5 p-2">
                            <div className="text-muted-foreground">Manual/Approval</div>
                            <div className="text-white/80">
                                {nlMetrics.manual_required + nlMetrics.approval_required}
                            </div>
                        </div>
                    </div>
                )}
                {approvalPolicies && approvalPolicies.length > 0 && (
                    <div className="border border-white/10 rounded-md p-2 text-[11px]">
                        <div className="text-[10px] text-muted-foreground mb-1">Approval policies</div>
                        <div className="space-y-1">
                            {approvalPolicies.map((policy) => (
                                <div key={policy.policy_key} className="flex items-center justify-between gap-2">
                                    <div className="truncate text-muted-foreground">
                                        {policy.policy_key} · {policy.decision}
                                    </div>
                                    <button
                                        onClick={() => handleRemovePolicy(policy.policy_key)}
                                        className="text-[10px] px-2 py-0.5 rounded bg-white/10 hover:bg-white/20 transition-colors"
                                    >
                                        Clear
                                    </button>
                                </div>
                            ))}
                        </div>
                    </div>
                )}
                <div className="grid grid-cols-5 gap-2">
                    {timeline.map((step) => (
                        <div
                            key={step.label}
                            className={`rounded border px-2 py-1 text-[10px] text-center ${statusClass(step.status)}`}
                        >
                            {step.label}
                        </div>
                    ))}
                </div>
                {failureSummary.length > 0 && (
                    <div className="rounded-md border border-amber-400/30 bg-amber-500/10 p-2 text-[11px] text-amber-100">
                        <div className="font-semibold mb-1">실패/차단 요약</div>
                        <div className="space-y-0.5">
                            {failureSummary.slice(0, 5).map((issue) => (
                                <div key={issue}>• {issue}</div>
                            ))}
                        </div>
                    </div>
                )}
                <div className="text-[11px] text-muted-foreground space-y-1">
                    {summary && (
                        <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px]">
                            <div className="font-semibold text-white/90 mb-1">Summary</div>
                            <div className="text-white/80 break-words">{summary}</div>
                        </div>
                    )}
                    {intent && (
                        <div>
                            Intent: {intent} · {confidence !== null ? `${(confidence * 100).toFixed(0)}%` : ""}
                        </div>
                    )}
                    {sessionId && <div>Session: {sessionId}</div>}
                    {planId && <div>Plan: {planId}</div>}
                    {missingSlots.length > 0 && (
                        <div>Missing slots: {missingSlots.join(", ")}</div>
                    )}
                    {followUp && <div>Follow-up: {followUp}</div>}
                    {planSteps.length > 0 && (
                        <div className="max-h-24 overflow-y-auto">
                            {planSteps.slice(0, 6).map((step, idx) => (
                                <div key={`${step}-${idx}`}>• {step}</div>
                            ))}
                        </div>
                    )}
                    {execStatus && <div>Execute: {execStatus}</div>}
                    <div>Profile: {executionProfile}</div>
                    {execLogs.length > 0 && (
                        <div className="max-h-24 overflow-y-auto">
                            {execLogs.slice(0, 6).map((line, idx) => (
                                <div key={`${line}-${idx}`}>• {line}</div>
                            ))}
                        </div>
                    )}
                    {lastRunId && (
                        <div className="rounded-md border border-white/10 bg-black/20 p-2 text-[11px]">
                            <div className="font-semibold text-white/90 mb-1">Run diagnostics ({lastRunId})</div>
                            {stageRuns.length > 0 && (
                                <div className="space-y-0.5 text-white/80">
                                    {stageRuns.slice(0, 6).map((stage) => (
                                        <div key={`${stage.stage_name}-${stage.id}`}>
                                            • {stage.stage_order}.{stage.stage_name}={stage.status}
                                        </div>
                                    ))}
                                </div>
                            )}
                            {stageAssertions.filter((a) => !a.passed).length > 0 && (
                                <div className="mt-2 text-amber-200 space-y-0.5">
                                    {stageAssertions
                                        .filter((a) => !a.passed)
                                        .slice(0, 6)
                                        .map((assertion) => (
                                            <div key={`${assertion.id}-${assertion.assertion_key}`}>
                                                • {assertion.stage_name}.{assertion.assertion_key} expected={assertion.expected} actual={assertion.actual}
                                            </div>
                                        ))}
                                </div>
                            )}
                        </div>
                    )}
                    {verifyStatus && <div>Verify: {verifyStatus}</div>}
                    {verifyIssues.length > 0 && (
                        <div className="max-h-20 overflow-y-auto text-amber-200">
                            {verifyIssues.map((issue, idx) => (
                                <div key={`${issue}-${idx}`}>• {issue}</div>
                            ))}
                        </div>
                    )}
                    {approvalHistory.length > 0 && (
                        <div className="mt-2 border-t border-white/10 pt-2">
                            <div className="text-[10px] text-muted-foreground mb-1">Approval history</div>
                            {approvalHistory.map((item, idx) => (
                                <div key={`${item.time}-${idx}`} className="text-[10px] text-muted-foreground">
                                    {item.time} · {item.action} · {item.result}
                                </div>
                            ))}
                        </div>
                    )}
                    {history.length > 0 && (
                        <div className="mt-2 border-t border-white/10 pt-2">
                            <div className="flex items-center justify-between mb-1">
                                <div className="text-[10px] text-muted-foreground">Recent runs</div>
                                <input
                                    value={historyFilter}
                                    onChange={(e) => setHistoryFilter(e.target.value)}
                                    placeholder="Filter..."
                                    className="text-[10px] bg-white/5 border border-white/10 rounded px-2 py-0.5"
                                />
                            </div>
                            {history
                                .filter(item => {
                                    const q = historyFilter.trim().toLowerCase();
                                    if (!q) return true;
                                    return (
                                        item.prompt.toLowerCase().includes(q) ||
                                        item.status.toLowerCase().includes(q)
                                    );
                                })
                                .map((item, idx) => (
                                    <div key={`${item.time}-${idx}`} className="text-[10px] text-muted-foreground">
                                        {item.time} · {item.status} · {item.prompt}
                                    </div>
                                ))}
                        </div>
                    )}
                    {nlRuns && nlRuns.length > 0 && (
                        <div className="mt-2 border-t border-white/10 pt-2">
                            <div className="text-[10px] text-muted-foreground mb-1">Saved runs</div>
                            {nlRuns
                                .filter(run => {
                                    const q = historyFilter.trim().toLowerCase();
                                    if (!q) return true;
                                    return (
                                        run.intent.toLowerCase().includes(q) ||
                                        run.status.toLowerCase().includes(q) ||
                                        (run.summary ? run.summary.toLowerCase().includes(q) : false)
                                    );
                                })
                                .slice(0, 5)
                                .map((run) => (
                                    <div key={run.id} className="text-[10px] text-muted-foreground">
                                        {format(new Date(run.created_at), "HH:mm:ss")} · {run.status} · {run.intent}
                                        {run.summary ? ` · ${run.summary}` : ""}
                                    </div>
                                ))}
                        </div>
                    )}
                </div>
            </CardContent>
        </Card>
    );
}
