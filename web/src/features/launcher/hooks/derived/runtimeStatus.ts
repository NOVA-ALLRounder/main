import type { AgentPreflightCheck, ExecutionProfile, TaskStageAssertion } from "@/lib/types";
import {
  artifactPathLabel,
  profileLabel,
  type ApprovalContext,
  type ComposerMode,
  type ExecutionSnapshot,
  type HudMeta,
  type LauncherResult,
  type PendingDispatch,
  type ProfileRecommendation,
  type RecoveryAction,
  type RunPhase,
  type RunScore,
} from "@/features/launcher/support";

const HUD_META: Record<RunPhase, HudMeta> = {
  idle: {
    label: "엔진 대기 중",
    chip: "bg-white/10 text-gray-300 border-white/15",
    dot: "bg-gray-400",
  },
  running: {
    label: "실행 중",
    chip: "bg-blue-500/20 text-blue-200 border-blue-400/40",
    dot: "bg-blue-400",
  },
  retrying: {
    label: "재시도 중",
    chip: "bg-amber-500/20 text-amber-200 border-amber-400/40",
    dot: "bg-amber-400",
  },
  approval_required: {
    label: "승인 필요",
    chip: "bg-rose-500/20 text-rose-200 border-rose-400/40",
    dot: "bg-rose-400",
  },
  manual_required: {
    label: "수동 단계 필요",
    chip: "bg-sky-500/20 text-sky-200 border-sky-400/40",
    dot: "bg-sky-400",
  },
  completed: {
    label: "완료",
    chip: "bg-emerald-500/20 text-emerald-200 border-emerald-400/40",
    dot: "bg-emerald-400",
  },
  failed: {
    label: "실패",
    chip: "bg-rose-500/20 text-rose-200 border-rose-400/40",
    dot: "bg-rose-400",
  },
};

type FocusPreflightState = {
  focusPreflight: AgentPreflightCheck | undefined;
  accessibilityPreflight: AgentPreflightCheck | undefined;
  screenCapturePreflight: AgentPreflightCheck | undefined;
  focusPreflightBlocked: boolean;
  focusNeedsHandoff: boolean;
};

type DeriveProfileRecommendationParams = FocusPreflightState & {
  preflightOk: boolean | null;
};

type DeriveRecoveryActionsParams = FocusPreflightState & {
  preflightOk: boolean | null;
  failedAssertions: TaskStageAssertion[];
  firstFailedArtifactPath: string | null;
  lastStatus: string | null;
  lastPlanId: string | null;
  pendingApproval: ApprovalContext | null;
};

type DeriveRunScoreParams = {
  runSnapshot: ExecutionSnapshot | null;
  failedAssertions: TaskStageAssertion[];
};

type DeriveNextActionHintParams = FocusPreflightState & {
  preflightOk: boolean | null;
  safeExecutionMode: boolean;
  executionProfile: ExecutionProfile;
  profileRecommendation: ProfileRecommendation;
  pendingApproval: ApprovalContext | null;
  lastStatus: string | null;
  lastPlanId: string | null;
  runSnapshot: ExecutionSnapshot | null;
  runPhase: RunPhase;
  failedAssertions: TaskStageAssertion[];
};

type DeriveShellStatusParams = {
  runSnapshot: ExecutionSnapshot | null;
  runPhase: RunPhase;
  results: LauncherResult[];
  suggestionCount: number;
  pendingApproval: ApprovalContext | null;
  lastStatus: string | null;
  lastPlanId: string | null;
  showDetailPanel: boolean;
  composerMode: ComposerMode;
  loading: boolean;
  approvalBusy: boolean;
  safeExecutionMode: boolean;
  pendingDispatch: PendingDispatch | null;
  dispatchBlockedReason: string | null;
  dispatchBlockedUntilMs: number | null;
  dispatchNowMs: number;
  preflightLoading: boolean;
  preflightOk: boolean | null;
  preflightError: string | null;
  showAdvancedControls: boolean;
};

export function deriveFocusPreflightState(preflightChecks: AgentPreflightCheck[]) {
  const focusPreflight = preflightChecks.find((check) => check.key === "focus_handoff");
  const accessibilityPreflight = preflightChecks.find(
    (check) => check.key === "accessibility"
  );
  const screenCapturePreflight = preflightChecks.find(
    (check) => check.key === "screen_capture"
  );
  const focusPreflightBlocked = !!focusPreflight && !focusPreflight.ok;
  const focusActualLower = (focusPreflight?.actual ?? "").toLowerCase();
  const focusNeedsHandoff =
    !!focusPreflight &&
    focusPreflight.ok &&
    focusActualLower.length > 0 &&
    !focusActualLower.includes("finder") &&
    !focusActualLower.includes("skipped");

  return {
    focusPreflight,
    accessibilityPreflight,
    screenCapturePreflight,
    focusPreflightBlocked,
    focusNeedsHandoff,
  };
}

export function deriveProfileRecommendation({
  preflightOk,
  focusPreflight,
  focusPreflightBlocked,
  focusNeedsHandoff,
}: DeriveProfileRecommendationParams): ProfileRecommendation {
  if (preflightOk === false) {
    return {
      profile: "strict",
      reason: "점검 실패 상태에서는 정확(Strict)로만 복구를 권장합니다.",
    };
  }
  if (
    focusPreflight &&
    focusPreflight.ok &&
    (focusPreflight.actual ?? "").toLowerCase().includes("skipped")
  ) {
    return {
      profile: "test",
      reason: "포커스 점검이 비활성화되어 테스트 프로필을 권장합니다.",
    };
  }
  if (focusPreflightBlocked || focusNeedsHandoff) {
    return {
      profile: "test",
      reason: "포커스 전환 불안정으로 테스트 프로필을 권장합니다.",
    };
  }
  return {
    profile: "strict",
    reason: "권한/포커스가 안정적이므로 정확(Strict)을 권장합니다.",
  };
}

export function deriveRecoveryActions({
  preflightOk,
  accessibilityPreflight,
  screenCapturePreflight,
  focusPreflightBlocked,
  focusNeedsHandoff,
  failedAssertions,
  firstFailedArtifactPath,
  lastStatus,
  lastPlanId,
  pendingApproval,
}: DeriveRecoveryActionsParams): RecoveryAction[] {
  const actions: RecoveryAction[] = [];
  const seen = new Set<string>();
  const failureBlob = failedAssertions
    .map((a) => `${a.assertion_key} ${a.evidence ?? ""} ${a.actual}`)
    .join("\n")
    .toLowerCase();
  const hasFailureHint = (hints: string[]) =>
    hints.some((hint) => failureBlob.includes(hint));
  const push = (action: RecoveryAction) => {
    if (seen.has(action.key)) return;
    seen.add(action.key);
    actions.push(action);
  };

  if (preflightOk === false) {
    if (accessibilityPreflight && !accessibilityPreflight.ok) {
      push({
        key: "fix-accessibility",
        label: "접근성 설정 열기",
        description: "접근성 권한 허용 후 다시 점검",
        kind: "preflight_fix",
        fixAction: "open_accessibility_settings",
        assertionKey: "recovery.preflight.open_accessibility_settings",
      });
    }
    if (screenCapturePreflight && !screenCapturePreflight.ok) {
      push({
        key: "fix-screen-capture",
        label: "화면 기록 설정 열기",
        description: "화면/오디오 녹화 권한 허용 후 다시 점검",
        kind: "preflight_fix",
        fixAction: "open_screen_capture_settings",
        assertionKey: "recovery.preflight.open_screen_capture_settings",
      });
    }
  }

  if (focusPreflightBlocked || focusNeedsHandoff) {
    push({
      key: "fix-focus",
      label: "Finder 전면 복구",
      description: "포커스 충돌 복구",
      kind: "preflight_fix",
      fixAction: "activate_finder",
      assertionKey: "recovery.preflight.activate_finder",
    });
    push({
      key: "fix-isolated",
      label: "격리 모드 준비",
      description: "다른 앱 숨김 후 실행 안정화",
      kind: "preflight_fix",
      fixAction: "prepare_isolated_mode",
      assertionKey: "recovery.preflight.prepare_isolated_mode",
    });
  }

  if (
    hasFailureHint([
      "artifact.mail_recipient_present",
      "contract_missing_mail_recipient",
      "mail_recipient=",
      "ambiguous_draft",
      "mail_send_proof",
      "outgoing=",
    ])
  ) {
    push({
      key: "fix-mail-recipient",
      label: "메일 수신자 보강",
      description: "run prompt/기본 수신자로 받는 사람 자동 채우기",
      kind: "preflight_fix",
      fixAction: "mail_fill_default_recipient",
      assertionKey: "recovery.preflight.mail_fill_default_recipient",
    });
    push({
      key: "fix-activate-mail",
      label: "Mail 전면 전환",
      description: "메일 작성창 포커스 복구",
      kind: "preflight_fix",
      fixAction: "activate_mail",
      assertionKey: "recovery.preflight.activate_mail",
    });
    push({
      key: "fix-mail-outgoing-cleanup",
      label: "Mail 초안창 정리",
      description: "누적된 outgoing 창 숨김 처리",
      kind: "preflight_fix",
      fixAction: "mail_cleanup_outgoing_windows",
      assertionKey: "recovery.preflight.mail_cleanup_outgoing_windows",
    });
  }

  if (
    hasFailureHint([
      "artifact.notes_write_confirmed",
      "artifact.notes_note_id_present",
      "contract_missing_notes",
    ])
  ) {
    push({
      key: "fix-activate-notes",
      label: "Notes 전면 전환",
      description: "노트 작성 타깃 포커스 복구",
      kind: "preflight_fix",
      fixAction: "activate_notes",
      assertionKey: "recovery.preflight.activate_notes",
    });
  }

  if (
    hasFailureHint([
      "artifact.textedit_write_confirmed",
      "artifact.textedit_doc_id_present",
      "artifact.textedit_save_confirmed",
      "contract_missing_textedit",
      "contract_textedit_body_empty",
    ])
  ) {
    push({
      key: "fix-activate-textedit",
      label: "TextEdit 전면 전환",
      description: "TextEdit 작성 대상 포커스 복구",
      kind: "preflight_fix",
      fixAction: "activate_textedit",
      assertionKey: "recovery.preflight.activate_textedit",
    });
    push({
      key: "fix-textedit-save",
      label: "TextEdit 저장 실행",
      description: "front document 저장(Cmd+S 대체)",
      kind: "preflight_fix",
      fixAction: "textedit_save_front_document",
      assertionKey: "recovery.preflight.textedit_save_front_document",
    });
  }

  if (firstFailedArtifactPath) {
    push({
      key: `artifact:${firstFailedArtifactPath}`,
      label: `증거 열기 (${artifactPathLabel(firstFailedArtifactPath)})`,
      description: "실패 근거 파일 열기",
      kind: "artifact",
      path: firstFailedArtifactPath,
      assertionKey: "recovery.artifact.open",
    });
  }

  if (lastStatus === "manual_required" && lastPlanId && !pendingApproval) {
    push({
      key: "guided-resume",
      label: "자동 복구 + Resume",
      description: "증거 확인 후 즉시 재개",
      kind: "guided_resume",
      assertionKey: "recovery.guided_resume",
    });
  }

  return actions.slice(0, 4);
}

export function deriveRunScore({
  runSnapshot,
  failedAssertions,
}: DeriveRunScoreParams): RunScore | null {
  return runSnapshot
    ? (() => {
        if (runSnapshot.completionScore) {
          return {
            score: runSnapshot.completionScore.score,
            label: runSnapshot.completionScore.label,
            pass: runSnapshot.completionScore.pass,
          };
        }
        let score = 0;
        if (runSnapshot.plannerComplete) score += 15;
        if (runSnapshot.executionComplete) score += 20;
        if (runSnapshot.businessComplete) score += 30;
        if (runSnapshot.verifyOk) score += 20;
        if (runSnapshot.status === "completed" || runSnapshot.status === "success") {
          score += 15;
        }
        score -= Math.min(10, failedAssertions.length * 2);
        score = Math.max(0, Math.min(100, score));
        const label =
          score >= 90 ? "Excellent" : score >= 75 ? "Good" : score >= 60 ? "Needs tuning" : "Risky";
        return { score, label, pass: score >= 75 };
      })()
    : null;
}

export function deriveRecoveryActionForFailureKey(failureKey: string): RecoveryAction | null {
  const key = failureKey.toLowerCase();
  if (key.includes("mail_recipient")) {
    return {
      key: `topfix:${failureKey}:mail_fill_default_recipient`,
      label: "수신자 보강",
      description: "Mail 받는 사람 자동 보강",
      kind: "preflight_fix",
      fixAction: "mail_fill_default_recipient",
      assertionKey: "recovery.preflight.mail_fill_default_recipient",
    };
  }
  if (
    key.includes("ambiguous_draft") ||
    key.includes("mail_send_proof") ||
    key.includes("outgoing")
  ) {
    return {
      key: `topfix:${failureKey}:mail_cleanup_outgoing_windows`,
      label: "Mail 초안창 정리",
      description: "누적 outgoing 창 숨김 처리",
      kind: "preflight_fix",
      fixAction: "mail_cleanup_outgoing_windows",
      assertionKey: "recovery.preflight.mail_cleanup_outgoing_windows",
    };
  }
  if (key.includes("textedit_save")) {
    return {
      key: `topfix:${failureKey}:textedit_save`,
      label: "TextEdit 저장",
      description: "TextEdit front document 저장 실행",
      kind: "preflight_fix",
      fixAction: "textedit_save_front_document",
      assertionKey: "recovery.preflight.textedit_save_front_document",
    };
  }
  if (key.includes("textedit")) {
    return {
      key: `topfix:${failureKey}:activate_textedit`,
      label: "TextEdit 전면",
      description: "TextEdit 포커스 복구",
      kind: "preflight_fix",
      fixAction: "activate_textedit",
      assertionKey: "recovery.preflight.activate_textedit",
    };
  }
  if (key.includes("notes")) {
    return {
      key: `topfix:${failureKey}:activate_notes`,
      label: "Notes 전면",
      description: "Notes 포커스 복구",
      kind: "preflight_fix",
      fixAction: "activate_notes",
      assertionKey: "recovery.preflight.activate_notes",
    };
  }
  if (key.includes("focus")) {
    return {
      key: `topfix:${failureKey}:activate_finder`,
      label: "Finder 전면",
      description: "Focus handoff 복구",
      kind: "preflight_fix",
      fixAction: "activate_finder",
      assertionKey: "recovery.preflight.activate_finder",
    };
  }
  return null;
}

export function deriveNextActionHint({
  preflightOk,
  accessibilityPreflight,
  screenCapturePreflight,
  focusPreflightBlocked,
  safeExecutionMode,
  executionProfile,
  profileRecommendation,
  pendingApproval,
  lastStatus,
  lastPlanId,
  runSnapshot,
  runPhase,
  failedAssertions,
}: DeriveNextActionHintParams): string {
  if (preflightOk === false) {
    if (focusPreflightBlocked) {
      return "Finder 전면 복구 또는 격리 모드 준비 후 다시 점검하세요.";
    }
    if (accessibilityPreflight && !accessibilityPreflight.ok) {
      return "접근성 설정에서 Codex/Terminal을 허용한 뒤 다시 점검하세요.";
    }
    if (screenCapturePreflight && !screenCapturePreflight.ok) {
      return "화면 기록 설정에서 Codex/Terminal 허용 후 다시 점검하세요.";
    }
    return "실행 전 점검 통과가 필요합니다.";
  }
  if (safeExecutionMode) {
    return "안전 모드 ON: Strict 프로필 고정으로 실행합니다.";
  }
  if (executionProfile !== profileRecommendation.profile) {
    return `권장 프로필은 ${profileLabel(profileRecommendation.profile)}입니다. (${profileRecommendation.reason})`;
  }
  if (pendingApproval) {
    return "승인 버튼(once/always/deny)으로 다음 단계를 선택하세요.";
  }
  if (lastStatus === "manual_required" && lastPlanId) {
    return "체크리스트 3개를 확인한 뒤 Resume으로 다음 단계를 진행하세요.";
  }
  if (runSnapshot?.completionScore && !runSnapshot.completionScore.pass) {
    return `완성도 미달: ${runSnapshot.completionScore.reasons[0] ?? "증거 부족"} 보완 후 재실행하세요.`;
  }
  if (runPhase === "failed") {
    if (failedAssertions.length > 0) {
      const first = failedAssertions[0];
      return `실패 근거: ${first.stage_name}.${first.assertion_key} (expected=${first.expected}, actual=${first.actual})`;
    }
    return "실패 로그를 확인하고 조건을 보강해 재실행하세요.";
  }
  if (runPhase === "completed") {
    return "완료되었습니다. 결과를 확인하고 다음 요청을 진행하세요.";
  }
  if (runPhase === "running" || runPhase === "retrying") {
    return "실행 중입니다. 입력 충돌을 피하고 기다려주세요.";
  }
  return "자연어 요청 또는 프로그램 버튼으로 실행을 시작하세요.";
}

export function deriveShellStatus({
  runSnapshot,
  runPhase,
  results,
  suggestionCount,
  pendingApproval,
  lastStatus,
  lastPlanId,
  showDetailPanel,
  composerMode,
  loading,
  approvalBusy,
  safeExecutionMode,
  pendingDispatch,
  dispatchBlockedReason,
  dispatchBlockedUntilMs,
  dispatchNowMs,
  preflightLoading,
  preflightOk,
  preflightError,
  showAdvancedControls,
}: DeriveShellStatusParams) {
  const currentHud = HUD_META[runPhase];
  const dodItems = runSnapshot
    ? [
        {
          key: "planner",
          label: "Planner 완료",
          done: runSnapshot.plannerComplete,
          detail: runSnapshot.plannerComplete ? "done" : "not done",
        },
        {
          key: "execution",
          label: "Execution 완료",
          done: runSnapshot.executionComplete,
          detail: runSnapshot.executionComplete ? "done" : "not done",
        },
        {
          key: "business",
          label: "Business 완료",
          done: runSnapshot.businessComplete,
          detail: runSnapshot.businessComplete ? "done" : "not done",
        },
        {
          key: "verify",
          label: "의미/검증 통과",
          done: runSnapshot.verifyOk,
          detail: runSnapshot.verifyOk
            ? "ok"
            : `${runSnapshot.verifyIssues.length} issue(s)`,
        },
        {
          key: "final",
          label: "최종 상태 성공",
          done:
            runSnapshot.status === "completed" || runSnapshot.status === "success",
          detail: runSnapshot.status,
        },
      ]
    : [];

  const hasDetailContent =
    results.length > 0 ||
    suggestionCount > 0 ||
    !!pendingApproval ||
    (lastStatus === "manual_required" && !!lastPlanId) ||
    !!runSnapshot;
  const isChatComposerMode = composerMode === "chat";
  const shouldShowDetailPanel = isChatComposerMode ? true : showDetailPanel;
  const shouldRenderDetailPanel =
    shouldShowDetailPanel && (isChatComposerMode || hasDetailContent);
  const checkpointHoldActive =
    runPhase === "manual_required" || runPhase === "approval_required";
  const isExecutionLocked =
    loading ||
    approvalBusy ||
    runPhase === "running" ||
    runPhase === "retrying" ||
    (safeExecutionMode && checkpointHoldActive) ||
    !!pendingDispatch;
  const dispatchRetrySeconds = dispatchBlockedUntilMs
    ? Math.max(0, Math.ceil((dispatchBlockedUntilMs - dispatchNowMs) / 1000))
    : 0;
  const safeCountdownSeconds = pendingDispatch
    ? Math.max(0, Math.ceil((pendingDispatch.executeAtMs - dispatchNowMs) / 1000))
    : 0;
  const executionLockHint = (() => {
    if (dispatchBlockedReason) {
      if (dispatchRetrySeconds > 0) {
        return `${dispatchBlockedReason} · ${dispatchRetrySeconds}초 후 재시도`;
      }
      return dispatchBlockedReason;
    }
    if (pendingDispatch) {
      return `안전 모드 카운트다운 ${safeCountdownSeconds}초`;
    }
    if (!isExecutionLocked) return null;
    if (loading) return "요청 실행 준비 중입니다.";
    if (approvalBusy) return "승인 처리 중입니다.";
    if (runPhase === "running") {
      return "실행 중입니다. 현재 run이 끝나면 새 요청을 보낼 수 있습니다.";
    }
    if (runPhase === "retrying") {
      return "재시도 중입니다. 완료 후 다시 요청하세요.";
    }
    if (safeExecutionMode && runPhase === "manual_required") {
      return "안전 모드: 수동 단계 완료 전에는 새 요청을 잠급니다. 아래 Resume을 진행하세요.";
    }
    if (safeExecutionMode && runPhase === "approval_required") {
      return "안전 모드: 승인 체크포인트 해결 전에는 새 요청을 잠급니다.";
    }
    return "현재 실행 잠금 상태입니다.";
  })();
  const showPreflightPanel =
    showAdvancedControls || preflightLoading || preflightOk === false || !!preflightError;

  return {
    currentHud,
    dodItems,
    hasDetailContent,
    shouldRenderDetailPanel,
    isExecutionLocked,
    safeCountdownSeconds,
    executionLockHint,
    showPreflightPanel,
  };
}
