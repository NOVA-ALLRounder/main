import { useCallback, useState } from "react";

import type { Recommendation } from "@/lib/types";
import type { ProvisioningUiState } from "@/features/launcher/support";

export function useRecommendationProvisioningState() {
    const [provisioningUiByRecId, setProvisioningUiByRecId] =
        useState<Record<number, ProvisioningUiState>>({});
    const [watchRecommendationIds, setWatchRecommendationIds] = useState<Set<number>>(new Set());
    const [watchRecommendationCache, setWatchRecommendationCache] =
        useState<Record<number, Recommendation>>({});

    const setProvisioningUiState = useCallback(
        (
            id: number,
            phase: ProvisioningUiState["phase"],
            options?: { opId?: number | null; detail?: string }
        ) => {
            setProvisioningUiByRecId((prev) => ({
                ...prev,
                [id]: {
                    phase,
                    opId: options?.opId ?? prev[id]?.opId ?? null,
                    detail: options?.detail,
                    updatedAt: Date.now(),
                },
            }));
        },
        []
    );

    const clearProvisioningUiState = useCallback((id: number) => {
        setProvisioningUiByRecId((prev) => {
            if (!(id in prev)) return prev;
            const next = { ...prev };
            delete next[id];
            return next;
        });
    }, []);

    const addWatchRecommendation = useCallback((id: number, fallback?: Recommendation | null) => {
        setWatchRecommendationIds((prev) => {
            const next = new Set(prev);
            next.add(id);
            return next;
        });
        if (fallback) {
            setWatchRecommendationCache((prev) => ({ ...prev, [id]: fallback }));
        }
    }, []);

    const removeWatchRecommendation = useCallback((id: number) => {
        setWatchRecommendationIds((prev) => {
            if (!prev.has(id)) return prev;
            const next = new Set(prev);
            next.delete(id);
            return next;
        });
        setWatchRecommendationCache((prev) => {
            if (!(id in prev)) return prev;
            const next = { ...prev };
            delete next[id];
            return next;
        });
        setProvisioningUiByRecId((prev) => {
            if (!(id in prev)) return prev;
            const next = { ...prev };
            delete next[id];
            return next;
        });
    }, []);

    return {
        provisioningUiByRecId,
        watchRecommendationIds,
        watchRecommendationCache,
        setWatchRecommendationCache,
        setProvisioningUiState,
        clearProvisioningUiState,
        addWatchRecommendation,
        removeWatchRecommendation,
    };
}
