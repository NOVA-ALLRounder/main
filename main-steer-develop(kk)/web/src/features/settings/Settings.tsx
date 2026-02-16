import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Check, ShieldCheck, Database, Server, RefreshCw, Power } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { motion } from "framer-motion";
import axios from "axios";
import {
    API_BASE_URL,
    getHealth,
    getRuntimeMode,
    getSystemPreflight,
    getVersion,
    setEmergencyStop,
    setRuntimeMode,
    type RuntimeMode,
    type SystemPreflight,
} from "@/lib/api";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

type SystemHealth = {
    missing_deps?: { name?: string; install_cmd?: string }[];
    api_port?: number;
    api_reachable?: boolean;
    llm_enabled?: boolean;
    gmail_credentials_set?: boolean;
    notion_ready?: boolean;
    analyzer_disabled?: boolean;
    background_analysis_disabled?: boolean;
    privacy_salt_set?: boolean;
    os?: string;
    checked_at_utc?: string;
};

export default function Settings() {
    const [n8nRestarting, setN8nRestarting] = useState(false);
    const [manualStatus, setManualStatus] = useState<"error" | null>(null);
    const queryClient = useQueryClient();
    const { data: healthData, isError: healthError, refetch: refetchHealth } = useQuery({
        queryKey: ["systemHealth"],
        queryFn: getHealth,
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
    });
    const { data: versionData } = useQuery({
        queryKey: ["systemVersion"],
        queryFn: getVersion,
        refetchInterval: 60000,
        refetchIntervalInBackground: false,
    });
    const { data: runtimeMode } = useQuery({
        queryKey: ["runtimeMode"],
        queryFn: getRuntimeMode,
        refetchInterval: 5000,
        refetchIntervalInBackground: true,
    });
    const { data: preflightData } = useQuery({
        queryKey: ["systemPreflight"],
        queryFn: getSystemPreflight,
        refetchInterval: 30000,
        refetchIntervalInBackground: false,
    });

    const modeMutation = useMutation({
        mutationFn: (mode: RuntimeMode["mode"]) => setRuntimeMode(mode),
        onSuccess: (data) => {
            queryClient.setQueryData(["runtimeMode"], data);
            queryClient.invalidateQueries({ queryKey: ["systemHealth"] });
        },
    });

    const emergencyMutation = useMutation({
        mutationFn: (enabled: boolean) => setEmergencyStop(enabled),
        onSuccess: (data) => {
            queryClient.setQueryData(["runtimeMode"], data);
            queryClient.invalidateQueries({ queryKey: ["systemHealth"] });
        },
    });
    const health = healthData as SystemHealth | undefined;
    const preflight = preflightData as SystemPreflight | undefined;
    const n8nMissing = Boolean(health?.missing_deps?.some((dep) => dep.name === "n8n"));
    const missingDeps = health?.missing_deps ?? [];
    const n8nStatus: "idle" | "success" | "error" | "failed" = manualStatus ?? (healthError || n8nMissing ? "failed" : "success");

    const checks = [
        { name: "Rust Core API", status: health?.api_reachable ? "Operational" : "Failed", icon: Server },
        { name: "SQLite Database", status: "Connected", icon: Database },
        { name: "n8n Integration", status: n8nStatus === "success" ? "Active" : "Failed", icon: ShieldCheck },
        { name: "LLM Client", status: health?.llm_enabled ? "Active" : "Disabled", icon: Check },
        { name: "Gmail Credentials", status: health?.gmail_credentials_set ? "Ready" : "Missing", icon: Check },
        { name: "Notion Integration", status: health?.notion_ready ? "Ready" : "Missing", icon: Check },
    ];

    const handleN8nRestart = async () => {
        setN8nRestarting(true);
        setManualStatus(null);
        try {
            // Call backend to restart n8n
            await axios.post(`${API_BASE_URL}/chat`, {
                message: "n8n restart"
            });

            // Poll for health status
            setTimeout(async () => {
                await refetchHealth();
                setN8nRestarting(false);
            }, 3000); // Wait 3s for n8n to start

        } catch {
            setManualStatus("error");
            setN8nRestarting(false);
        }
    };

    const containerVariants = {
        hidden: { opacity: 0 },
        visible: {
            opacity: 1,
            transition: { staggerChildren: 0.1 }
        }
    };

    const itemVariants = {
        hidden: { opacity: 0, y: 20 },
        visible: { opacity: 1, y: 0, transition: { duration: 0.4 } }
    };

    return (
        <motion.div
            className="space-y-6"
            initial="hidden"
            animate="visible"
            variants={containerVariants}
        >
            <motion.h2
                className="text-3xl font-bold tracking-tight text-glow"
                variants={itemVariants}
            >
                System Settings
            </motion.h2>

            <div className="grid gap-6 md:grid-cols-2">
                <motion.div variants={itemVariants}>
                    <Card>
                        <CardHeader>
                            <CardTitle>System Health</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            {checks.map((check, idx) => (
                                <motion.div
                                    key={check.name}
                                    className="flex items-center justify-between p-3 rounded-lg bg-white/5 border border-white/5"
                                    initial={{ opacity: 0, x: -20 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    transition={{ delay: idx * 0.1 }}
                                >
                                    <div className="flex items-center gap-3">
                                        <div className={`p-2 rounded-full bg-opacity-10 ${check.status === "Active" || check.status === "Operational" || check.status === "Running" || check.status === "Connected" ? "bg-green-500 text-green-500" : "bg-red-500 text-red-500"}`}>
                                            <check.icon className="w-4 h-4" />
                                        </div>
                                        <span className="font-medium">{check.name}</span>
                                    </div>
                                    <span className={`text-xs font-mono px-2 py-1 rounded-full border ${check.status === "Active" || check.status === "Operational" || check.status === "Running" || check.status === "Connected" ? "text-green-400 bg-green-400/10 border-green-400/20" : "text-red-400 bg-red-400/10 border-red-400/20"}`}>
                                        {check.status}
                                    </span>
                                </motion.div>
                            ))}
                        </CardContent>
                    </Card>
                </motion.div>

                <motion.div variants={itemVariants}>
                    <Card>
                        <CardHeader>
                            <CardTitle>Service Control</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="p-4 rounded-lg bg-white/5 border border-white/5">
                                <div className="flex items-center justify-between mb-3">
                                    <div className="flex items-center gap-2">
                                        <Power className={`w-4 h-4 ${n8nStatus === "success" ? "text-green-400" : "text-orange-400"}`} />
                                        <span className="font-medium">n8n Server</span>
                                    </div>
                                    {n8nStatus === "success" ? (
                                        <span className="text-xs text-green-400">Running</span>
                                    ) : (
                                        <span className="text-xs text-red-400">Failed</span>
                                    )}
                                </div>
                                <motion.button
                                    onClick={handleN8nRestart}
                                    disabled={n8nRestarting}
                                    className="w-full py-2 rounded-lg bg-orange-500/10 text-orange-400 hover:bg-orange-500/20 transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
                                    whileHover={{ scale: 1.02 }}
                                    whileTap={{ scale: 0.98 }}
                                >
                                    <RefreshCw className={`w-4 h-4 ${n8nRestarting ? 'animate-spin' : ''}`} />
                                    {n8nRestarting ? "Restarting..." : "Restart n8n Server"}
                                </motion.button>
                            </div>

                            <div className="p-4 rounded-lg bg-white/5 border border-white/5 space-y-3">
                                <div className="flex items-center justify-between">
                                    <span className="font-medium">Operation Mode</span>
                                    <span className="text-xs font-mono text-white">{runtimeMode?.mode ?? "autopilot"}</span>
                                </div>
                                <div className="grid grid-cols-3 gap-2">
                                    {(["observe", "copilot", "autopilot"] as const).map((mode) => {
                                        const active = (runtimeMode?.mode ?? "autopilot") === mode;
                                        return (
                                            <button
                                                key={mode}
                                                onClick={() => modeMutation.mutate(mode)}
                                                disabled={modeMutation.isPending}
                                                className={`py-2 rounded-md text-xs border transition-colors ${
                                                    active
                                                        ? "bg-primary/20 border-primary/40 text-primary"
                                                        : "bg-white/5 border-white/10 text-gray-300 hover:bg-white/10"
                                                }`}
                                            >
                                                {mode}
                                            </button>
                                        );
                                    })}
                                </div>
                                <div className="flex items-center justify-between pt-1">
                                    <div>
                                        <div className="text-sm font-medium">Emergency Stop</div>
                                        <div className="text-xs text-muted-foreground">Block all automation actions immediately</div>
                                    </div>
                                    <Switch
                                        checked={runtimeMode?.emergency_stop ?? false}
                                        onCheckedChange={(checked) => emergencyMutation.mutate(checked)}
                                        disabled={emergencyMutation.isPending}
                                    />
                                </div>
                                <div className="text-xs text-muted-foreground">
                                    allow_automation: <span className="font-mono text-white">{String(runtimeMode?.allow_automation ?? false)}</span>
                                </div>
                            </div>

                            <div className="pt-4 border-t border-white/10 text-sm text-muted-foreground">
                                <div className="flex justify-between mb-2">
                                    <span>API Port</span>
                                    <span className="font-mono text-white">{health?.api_port ?? 5680}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Operation Mode (health)</span>
                                    <span className="font-mono text-white">{(health as { operation_mode?: string } | undefined)?.operation_mode ?? "unknown"}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Emergency Stop (health)</span>
                                    <span className="font-mono text-white">{String((health as { emergency_stop?: boolean } | undefined)?.emergency_stop ?? false)}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Runtime OS</span>
                                    <span className="font-mono text-white">{health?.os ?? "unknown"}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Analyzer</span>
                                    <span className="font-mono text-white">{health?.analyzer_disabled ? "disabled" : "enabled"}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Background Analysis</span>
                                    <span className="font-mono text-white">{health?.background_analysis_disabled ? "disabled" : "enabled"}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Privacy Salt</span>
                                    <span className="font-mono text-white">{health?.privacy_salt_set ? "set" : "missing"}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Gmail Credentials</span>
                                    <span className="font-mono text-white">{health?.gmail_credentials_set ? "set" : "missing"}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Notion Ready</span>
                                    <span className="font-mono text-white">{health?.notion_ready ? "yes" : "no"}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Core Version</span>
                                    <span className="font-mono text-white">
                                        v{versionData?.core_version ?? "unknown"} ({versionData?.build_profile ?? "unknown"})
                                    </span>
                                </div>
                                <div className="flex justify-between">
                                    <span>Frontend Version</span>
                                    <span className="font-mono text-white">
                                        {import.meta.env.VITE_APP_VERSION ?? "dev"}
                                    </span>
                                </div>
                            </div>

                            <div className="pt-4 border-t border-white/10 text-sm text-muted-foreground">
                                <div className="flex items-center justify-between mb-2">
                                    <span>Preflight</span>
                                    <span className={`font-mono ${preflight?.ok ? "text-green-400" : "text-amber-300"}`}>
                                        {preflight?.ok ? "PASS" : "WARN"}
                                    </span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>STEER_HOME Writable</span>
                                    <span className="font-mono text-white">{String(preflight?.steer_home_writable ?? false)}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Release Writable</span>
                                    <span className="font-mono text-white">{String(preflight?.release_dir_writable ?? false)}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Gmail Credentials</span>
                                    <span className="font-mono text-white">{String(preflight?.gmail_credentials_set ?? false)}</span>
                                </div>
                                <div className="flex justify-between mb-2">
                                    <span>Notion Ready</span>
                                    <span className="font-mono text-white">{String(preflight?.notion_ready ?? false)}</span>
                                </div>
                                {preflight?.notes?.length ? (
                                    <div className="mt-2 rounded-md border border-white/10 bg-white/5 p-2 text-xs">
                                        {preflight.notes.join(" | ")}
                                    </div>
                                ) : null}
                            </div>

                            {!health?.gmail_credentials_set && (
                                <div className="p-3 rounded-lg bg-amber-500/10 border border-amber-500/20 text-xs text-amber-200">
                                    Gmail 연동이 비활성입니다. `core/credentials.json` 파일이 필요합니다.
                                </div>
                            )}
                            {!health?.notion_ready && (
                                <div className="p-3 rounded-lg bg-blue-500/10 border border-blue-500/20 text-xs text-blue-200">
                                    Notion 연동은 `NOTION_API_KEY`, `NOTION_DATABASE_ID` 환경변수 설정 후 활성화됩니다.
                                </div>
                            )}
                        </CardContent>
                    </Card>
                </motion.div>

                <motion.div variants={itemVariants} className="md:col-span-2">
                    <Card>
                        <CardHeader>
                            <CardTitle>Missing Dependencies</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-3">
                            {missingDeps.length === 0 ? (
                                <div className="text-sm text-gray-500">All dependencies are installed.</div>
                            ) : (
                                missingDeps.map((dep) => (
                                    <div
                                        key={dep.name}
                                        className="flex items-center justify-between p-3 rounded-lg bg-white/5 border border-white/5"
                                    >
                                        <div className="flex items-center gap-3">
                                            <div className="p-2 rounded-full bg-red-500/10 text-red-400">
                                                <ShieldCheck className="w-4 h-4" />
                                            </div>
                                            <div>
                                                <div className="font-medium">{dep.name}</div>
                                                <div className="text-xs text-gray-500">Install: {dep.install_cmd}</div>
                                            </div>
                                        </div>
                                    </div>
                                ))
                            )}
                        </CardContent>
                    </Card>
                </motion.div>
            </div>
        </motion.div>
    );
}
