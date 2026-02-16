import { useState } from "react";
import {
    agentApprove,
    agentExecute,
    agentIntent,
    agentPlan,
    agentVerify,
    sendChatMessage,
} from "@/lib/api";

export type LauncherResult = {
    type: "response" | "error";
    content: string;
};

export type ApprovalContext = {
    planId: string;
    action: string;
    message: string;
    riskLevel: string;
    policy: string;
};

type UseAgentWorkflowArgs = {
    onSuccess: () => void;
    onError: () => void;
};

const withTimeout = async <T>(promise: Promise<T>, ms: number, label: string): Promise<T> => {
    return new Promise<T>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`${label} timeout`)), ms);
        promise
            .then((value) => {
                clearTimeout(timer);
                resolve(value);
            })
            .catch((err) => {
                clearTimeout(timer);
                reject(err);
            });
    });
};

const formatExecutionResult = (
    status: string,
    verifyOk: boolean,
    logs: string[] | undefined,
    verifyIssues: string[] | undefined,
    manualSteps: string[] | undefined,
    resumeFrom?: number | null,
): string => {
    const summaryLines = [
        `**Status**: ${status}`,
        `**Verify**: ${verifyOk ? "ok" : "issues"}`,
        resumeFrom != null ? `**Next Step**: ${resumeFrom + 1}` : "",
    ];
    const logLines = logs?.slice(0, 10).map((line) => `- ${line}`) ?? [];
    const verifyLines = verifyIssues?.length ? verifyIssues.map((issue) => `- ${issue}`) : [];
    const manualLines = manualSteps?.length ? manualSteps.map((step) => `- ${step}`) : [];
    return [
        summaryLines.filter(Boolean).join("\n"),
        logLines.length ? `\n**Logs**\n${logLines.join("\n")}` : "",
        verifyLines.length ? `\n**Verify Issues**\n${verifyLines.join("\n")}` : "",
        manualLines.length ? `\n**Manual Steps**\n${manualLines.join("\n")}` : "",
    ].join("\n");
};

export const useAgentWorkflow = ({ onSuccess, onError }: UseAgentWorkflowArgs) => {
    const [results, setResults] = useState<LauncherResult[]>([]);
    const [loading, setLoading] = useState(false);
    const [pendingApproval, setPendingApproval] = useState<ApprovalContext | null>(null);
    const [approvalBusy, setApprovalBusy] = useState(false);
    const [lastPlanId, setLastPlanId] = useState<string | null>(null);
    const [lastStatus, setLastStatus] = useState<string | null>(null);

    const handleSend = async (input: string, clearInput: () => void) => {
        if (!input.trim()) return;
        setLoading(true);
        setResults([]);
        setPendingApproval(null);

        try {
            const intentRes = await withTimeout(agentIntent(input), 8000, "agentIntent");
            const isLikelySmallTalk =
                intentRes.intent === "generic_task" &&
                (intentRes.confidence < 0.35 || input.trim().length <= 12);

            if (isLikelySmallTalk) {
                const chat = await withTimeout(sendChatMessage(input), 12000, "sendChatMessage");
                const lines = [chat.response];
                if (chat.command) lines.push(`\n**Command**: ${chat.command}`);
                if (chat.error_code) lines.push(`\n**ErrorCode**: ${chat.error_code}`);
                setResults([{ type: "response", content: lines.join("") }]);
                clearInput();
                onSuccess();
                return;
            }

            if (intentRes.missing_slots && intentRes.missing_slots.length > 0) {
                const followUp = intentRes.follow_up || "추가 정보가 필요합니다";
                setResults([
                    {
                        type: "response",
                        content: `**추가 입력 필요**\n- Missing: ${intentRes.missing_slots.join(", ")}\n- ${followUp}`,
                    },
                ]);
                return;
            }

            const planRes = await withTimeout(
                agentPlan(intentRes.session_id),
                8000,
                "agentPlan",
            );
            setLastPlanId(planRes.plan_id);
            if (planRes.missing_slots?.length) {
                setResults([
                    {
                        type: "response",
                        content: `**추가 입력 필요**\n- Missing: ${planRes.missing_slots.join(", ")}`,
                    },
                ]);
                return;
            }

            const execRes = await withTimeout(
                agentExecute(planRes.plan_id),
                10000,
                "agentExecute",
            );
            const verifyRes = await withTimeout(
                agentVerify(planRes.plan_id),
                10000,
                "agentVerify",
            );
            const hasNoExtractIssue = verifyRes.issues.some((issue) =>
                issue.toLowerCase().includes("no extract step")
            );
            if (intentRes.intent === "generic_task" && hasNoExtractIssue) {
                const chat = await withTimeout(sendChatMessage(input), 12000, "sendChatMessage");
                const lines = [chat.response];
                if (chat.command) lines.push(`\n**Command**: ${chat.command}`);
                if (chat.error_code) lines.push(`\n**ErrorCode**: ${chat.error_code}`);
                setResults([{ type: "response", content: lines.join("") }]);
                clearInput();
                onSuccess();
                return;
            }

            setLastStatus(execRes.status);
            if (execRes.status === "approval_required" && execRes.approval?.action) {
                setPendingApproval({
                    planId: planRes.plan_id,
                    action: execRes.approval.action,
                    message: execRes.approval.message,
                    riskLevel: execRes.approval.risk_level,
                    policy: execRes.approval.policy,
                });
            }

            const intentLine = `**Intent**: ${intentRes.intent} (${Math.round(intentRes.confidence * 100)}%)`;
            const details = formatExecutionResult(
                execRes.status,
                verifyRes.ok,
                execRes.logs,
                verifyRes.issues,
                execRes.manual_steps,
                execRes.resume_from,
            );
            const approvalBlock =
                execRes.status === "approval_required" && execRes.approval
                    ? `\n**Approval Required**\n- Action: ${execRes.approval.action}\n- Risk: ${execRes.approval.risk_level}\n- Policy: ${execRes.approval.policy}\n- ${execRes.approval.message}`
                    : "";
            setResults([{ type: "response", content: `${intentLine}\n${details}${approvalBlock}` }]);
            clearInput();
            onSuccess();
        } catch (error) {
            console.error("Launcher send failed", error);
            try {
                const res = await withTimeout(sendChatMessage(input), 12000, "sendChatMessage");
                const fallbackPrefix =
                    error instanceof Error && error.message.includes("timeout")
                        ? "⚠️ 자동화 플로우가 지연되어 일반 채팅으로 전환했습니다.\n\n"
                        : "";
                const lines = [`${fallbackPrefix}${res.response}`];
                if (res.command) lines.push(`\n**Command**: ${res.command}`);
                if (res.error_code) lines.push(`\n**ErrorCode**: ${res.error_code}`);
                setResults([{ type: "response", content: lines.join("") }]);
                clearInput();
                onSuccess();
            } catch {
                setResults([{ type: "error", content: "Failed to reach agent. Check Core API health and runtime mode in Settings." }]);
                onError();
            }
        } finally {
            setLoading(false);
        }
    };

    const handleResume = async () => {
        if (!lastPlanId) return;
        setLoading(true);
        try {
            const execRes = await withTimeout(agentExecute(lastPlanId), 10000, "agentExecute");
            const verifyRes = await withTimeout(agentVerify(lastPlanId), 10000, "agentVerify");
            setLastStatus(execRes.status);

            if (execRes.status === "approval_required" && execRes.approval?.action) {
                setPendingApproval({
                    planId: lastPlanId,
                    action: execRes.approval.action,
                    message: execRes.approval.message,
                    riskLevel: execRes.approval.risk_level,
                    policy: execRes.approval.policy,
                });
            } else {
                setPendingApproval(null);
            }

            setResults([
                {
                    type: "response",
                    content: formatExecutionResult(
                        execRes.status,
                        verifyRes.ok,
                        execRes.logs,
                        verifyRes.issues,
                        execRes.manual_steps,
                        execRes.resume_from,
                    ),
                },
            ]);
            onSuccess();
        } catch (error) {
            console.error("Resume failed", error);
            onError();
        } finally {
            setLoading(false);
        }
    };

    const handleApprovalDecision = async (decision: "allow_once" | "allow_always" | "deny") => {
        if (!pendingApproval) return;
        setApprovalBusy(true);
        try {
            const approvalRes = await withTimeout(
                agentApprove(
                    pendingApproval.planId,
                    pendingApproval.action,
                    decision,
                ),
                8000,
                "agentApprove",
            );
            if (decision === "deny" || approvalRes.status === "denied") {
                setResults([
                    {
                        type: "response",
                        content: `**Approval Denied**\n- Action: ${pendingApproval.action}\n- Policy: ${approvalRes.policy}`,
                    },
                ]);
                setPendingApproval(null);
                onSuccess();
                return;
            }

            const execRes = await withTimeout(
                agentExecute(pendingApproval.planId),
                10000,
                "agentExecute",
            );
            const verifyRes = await withTimeout(
                agentVerify(pendingApproval.planId),
                10000,
                "agentVerify",
            );
            if (execRes.status === "approval_required" && execRes.approval?.action) {
                setPendingApproval({
                    planId: pendingApproval.planId,
                    action: execRes.approval.action,
                    message: execRes.approval.message,
                    riskLevel: execRes.approval.risk_level,
                    policy: execRes.approval.policy,
                });
            } else {
                setPendingApproval(null);
            }

            setResults([
                {
                    type: "response",
                    content: formatExecutionResult(
                        execRes.status,
                        verifyRes.ok,
                        execRes.logs,
                        verifyRes.issues,
                        execRes.manual_steps,
                        execRes.resume_from,
                    ),
                },
            ]);
            onSuccess();
        } catch (error) {
            console.error("Approval flow failed", error);
            onError();
        } finally {
            setApprovalBusy(false);
        }
    };

    return {
        results,
        setResults,
        loading,
        pendingApproval,
        approvalBusy,
        lastPlanId,
        lastStatus,
        setPendingApproval,
        handleSend,
        handleResume,
        handleApprovalDecision,
    };
};
