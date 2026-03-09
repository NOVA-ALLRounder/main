import type { LauncherResult } from "@/features/launcher/support";

export const extractApproveErrorMessage = (error: unknown) => {
    if (typeof error === "string") return error;
    if (error && typeof error === "object") {
        const maybe = error as {
            message?: unknown;
            response?: { data?: { error?: unknown; details?: unknown } };
        };
        const responseError = maybe.response?.data?.error;
        if (typeof responseError === "string") return responseError;
        const responseDetails = maybe.response?.data?.details;
        if (typeof responseDetails === "string") return responseDetails;
        if (typeof maybe.message === "string") return maybe.message;
    }
    return "Approve failed";
};

export const mapApproveErrorMessage = (raw: string) => {
    const msg = raw.toLowerCase();
    if (msg.includes("unauthorized") || msg.includes("401")) {
        return "n8n API 인증 실패 (API 키 확인 필요)";
    }
    if (msg.includes("nodes") && msg.includes("empty")) {
        return "워크플로우 노드가 비어 있음 (재시도 또는 최소 템플릿 확인)";
    }
    if (msg.includes("timeout")) {
        return "요청 시간이 초과됨. 승인 접수는 됐을 수 있으니 잠시 후 상태를 확인하세요.";
    }
    if (msg.includes("connection refused")) {
        return "코어 서버 연결 실패 (5680 실행 상태 확인)";
    }
    return raw;
};

export function buildProvisionFailureResult(params: {
    recommendationId: number;
    reason: string;
    provisionOpId?: number | null;
    editorUrl: string;
}): LauncherResult {
    const { recommendationId, reason, provisionOpId, editorUrl } = params;
    return {
        type: "error",
        content: [
            "**Workflow 생성 실패**",
            `- recommendation_id: \`${recommendationId}\``,
            provisionOpId != null ? `- provision_op_id: \`${provisionOpId}\`` : "",
            `- 상세: ${reason}`,
            `- 수동 확인: ${editorUrl}`,
            "- Retry를 누르면 다시 생성을 시도합니다.",
        ]
            .filter(Boolean)
            .join("\n"),
    };
}

export function buildWorkflowApprovedResult(params: {
    recommendationId: number;
    workflowId?: string | null;
    provisionOpId?: number | null;
    workflowUrl: string;
    openFailedReason?: string | null;
}): LauncherResult {
    const {
        recommendationId,
        workflowId,
        provisionOpId,
        workflowUrl,
        openFailedReason,
    } = params;
    return {
        type: "response",
        content: [
            openFailedReason
                ? "**Workflow 승인 완료 (열기 실패)**"
                : "**Workflow 승인 완료**",
            `- recommendation_id: \`${recommendationId}\``,
            workflowId ? `- workflow_id: \`${workflowId}\`` : "",
            provisionOpId != null ? `- provision_op_id: \`${provisionOpId}\`` : "",
            openFailedReason ? `- n8n URL: ${workflowUrl}` : `- n8n 편집기: ${workflowUrl}`,
            openFailedReason ? `- 열기 오류: ${openFailedReason}` : "- 워크플로를 자동으로 열었습니다.",
            openFailedReason ? "" : "- 백엔드가 활성화 + 자동 실행(webhook/execute)을 시도합니다.",
            openFailedReason ? "" : "- 실행 기록은 n8n의 Executions 탭에서 확인하세요.",
        ]
            .filter(Boolean)
            .join("\n"),
    };
}

export function buildWorkflowPendingResult(params: {
    recommendationId: number;
    provisionOpId?: number | null;
    provisionStatus?: string | null;
    editorOpenNote: string;
}): LauncherResult {
    const { recommendationId, provisionOpId, provisionStatus, editorOpenNote } = params;
    return {
        type: "response",
        content: [
            "**승인 완료 · 생성 대기**",
            `- recommendation_id: \`${recommendationId}\``,
            provisionOpId != null ? `- provision_op_id: \`${provisionOpId}\`` : "",
            provisionStatus ? `- provision_status: \`${provisionStatus}\`` : "",
            editorOpenNote,
            "- workflow URL 생성 중입니다. 준비되면 해당 워크플로 편집기를 자동으로 엽니다.",
            "- 생성 후 백엔드가 자동 실행(webhook/execute)을 시도합니다.",
        ]
            .filter(Boolean)
            .join("\n"),
    };
}

export function buildWorkflowPendingNoticeResult(params: {
    recommendationId: number;
    provisionOpId?: number | null;
}): LauncherResult {
    const { recommendationId, provisionOpId } = params;
    return {
        type: "response",
        content: [
            "**승인 완료 · 생성 대기**",
            `- recommendation_id: \`${recommendationId}\``,
            provisionOpId != null ? `- provision_op_id: \`${provisionOpId}\`` : "",
            "- n8n 워크플로 생성 요청은 접수됐지만 아직 완료되지 않았습니다.",
            "- 잠시 후 자동 재시도하거나, n8n 상태를 먼저 확인하세요.",
            "- 참고: 캔버스의 Test 대기 상태와 별개로, 자동 실행은 production 경로로 시도됩니다.",
        ]
            .filter(Boolean)
            .join("\n"),
    };
}

export function buildOpenEditorResult(params: {
    recommendationId: number;
    workflowId?: string | null;
    targetUrl: string;
}): LauncherResult {
    const { recommendationId, workflowId, targetUrl } = params;
    return {
        type: "response",
        content: [
            "**n8n 편집기 열기**",
            `- recommendation_id: \`${recommendationId}\``,
            workflowId ? `- workflow_id: \`${workflowId}\`` : "",
            `- URL: ${targetUrl}`,
        ]
            .filter(Boolean)
            .join("\n"),
    };
}
