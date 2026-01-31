import { useQuery } from "@tanstack/react-query";
import { fetchSystemStatus, fetchLogs, fetchRoutines, fetchRecommendations, fetchRecommendationMetrics, fetchExecApprovals, fetchExecAllowlist, fetchExecResults, fetchRoutineRuns, fetchLatestQualityScore, fetchConsistencyCheck, fetchSemanticVerification, fetchReleaseGate, fetchVerificationRuns, fetchNlRuns, fetchNlRunMetrics, fetchApprovalPolicies, type ReleaseGateOverrides } from "./api";

export function useSystemStatus() {
    return useQuery({
        queryKey: ["systemStatus"],
        queryFn: fetchSystemStatus,
        refetchInterval: 3000, // Poll every 3 seconds (reduced from 2 to prevent flicker)
        refetchIntervalInBackground: false,
        placeholderData: (previousData) => previousData, // Keep previous data visible during refetch
    });
}

export function useLogs() {
    return useQuery({
        queryKey: ["logs"],
        queryFn: fetchLogs,
        refetchInterval: 5000, // Poll every 5 seconds
    });
}

export function useRoutines() {
    return useQuery({
        queryKey: ["routines"],
        queryFn: fetchRoutines,
        refetchInterval: 10000, // Poll every 10 seconds
    });
}

export function useRecommendations() {
    return useQuery({
        queryKey: ["recommendations"],
        queryFn: fetchRecommendations,
        refetchInterval: 10000,
    });
}

export function useRecommendationMetrics() {
    return useQuery({
        queryKey: ["recommendationMetrics"],
        queryFn: fetchRecommendationMetrics,
        refetchInterval: 15000,
        refetchIntervalInBackground: false,
        placeholderData: (previousData) => previousData,
    });
}

export function useExecApprovals(status: string = "pending") {
    return useQuery({
        queryKey: ["execApprovals", status],
        queryFn: () => fetchExecApprovals(status),
        refetchInterval: 8000,
    });
}

export function useExecAllowlist(limit: number = 100) {
    return useQuery({
        queryKey: ["execAllowlist", limit],
        queryFn: () => fetchExecAllowlist(limit),
        refetchInterval: 15000,
    });
}

export function useExecResults(limit: number = 100, status?: string) {
    return useQuery({
        queryKey: ["execResults", limit, status ?? "all"],
        queryFn: () => fetchExecResults(limit, status),
        refetchInterval: 10000,
    });
}

export function useRoutineRuns(limit: number = 20) {
    return useQuery({
        queryKey: ["routineRuns", limit],
        queryFn: () => fetchRoutineRuns(limit),
        refetchInterval: 15000,
    });
}

export function useQualityScore() {
    return useQuery({
        queryKey: ["qualityScoreLatest"],
        queryFn: fetchLatestQualityScore,
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
        placeholderData: (previousData) => previousData,
    });
}

export function useConsistencyCheck() {
    return useQuery({
        queryKey: ["consistencyCheck"],
        queryFn: fetchConsistencyCheck,
        refetchInterval: 60000,
        refetchIntervalInBackground: false,
    });
}

export function useSemanticVerification() {
    return useQuery({
        queryKey: ["semanticVerification"],
        queryFn: fetchSemanticVerification,
        refetchInterval: 60000,
        refetchIntervalInBackground: false,
    });
}

export function useReleaseGate(overrides?: ReleaseGateOverrides) {
    return useQuery({
        queryKey: ["releaseGate", overrides?.perf_regression_pct ?? "default", overrides?.quality_drop ?? "default"],
        queryFn: () => fetchReleaseGate(overrides),
        refetchInterval: 60000,
        refetchIntervalInBackground: false,
    });
}

export function useVerificationRuns(limit: number = 20) {
    return useQuery({
        queryKey: ["verificationRuns", limit],
        queryFn: () => fetchVerificationRuns(limit),
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
    });
}

export function useNlRuns(limit: number = 20) {
    return useQuery({
        queryKey: ["nlRuns", limit],
        queryFn: () => fetchNlRuns(limit),
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
    });
}

export function useNlRunMetrics(limit: number = 50) {
    return useQuery({
        queryKey: ["nlRunMetrics", limit],
        queryFn: () => fetchNlRunMetrics(limit),
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
    });
}

export function useApprovalPolicies(limit: number = 20) {
    return useQuery({
        queryKey: ["approvalPolicies", limit],
        queryFn: () => fetchApprovalPolicies(limit),
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
    });
}
