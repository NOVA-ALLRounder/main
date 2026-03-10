import axios from "axios";

export function isLocalApiOfflineError(error: unknown) {
    return (
        axios.isAxiosError(error) &&
        !error.response &&
        (error.code === "ERR_NETWORK" || error.message === "Network Error")
    );
}

export function retryUnlessOffline(maxRetries: number) {
    return (failureCount: number, error: unknown) => {
        if (isLocalApiOfflineError(error)) {
            return false;
        }
        return failureCount < maxRetries;
    };
}
