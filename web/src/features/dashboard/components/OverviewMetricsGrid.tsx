import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Activity, Cpu, HardDrive, Lightbulb, ShieldCheck, type LucideIcon } from "lucide-react";
import { motion, useReducedMotion } from "framer-motion";
import { cn } from "@/lib/utils";

const cardVariants = {
    hidden: { opacity: 0, y: 20, scale: 0.95 },
    visible: { opacity: 1, y: 0, scale: 1, transition: { duration: 0.4 } },
};

type Props = {
    cpuValue: string;
    memoryUsed: string;
    memoryTotal: string;
    activeRoutinesCount: number;
    pendingRecommendations: number;
    totalRecommendations: number;
    approvalRate: string;
    lastRecTime: string;
    qualityValue: string;
    qualityLabel: string;
    qualityTime: string;
};

export function OverviewMetricsGrid(props: Props) {
    const prefersReducedMotion = useReducedMotion();
    const metrics: Array<{
        title: string;
        eyebrow: string;
        value: string;
        detail: string;
        icon: LucideIcon;
        spanClassName: string;
        cardClassName: string;
        iconWrapClassName: string;
        valueClassName?: string;
        glowClassName: string;
    }> = [
        {
            title: "Quality Score",
            eyebrow: "Readiness",
            value: props.qualityValue,
            detail: `${props.qualityLabel} · refreshed ${props.qualityTime}`,
            icon: ShieldCheck,
            spanClassName: "xl:col-span-5",
            cardClassName: "border-emerald-400/20 bg-emerald-500/5",
            iconWrapClassName: "border-emerald-300/20 bg-emerald-400/10 text-emerald-100",
            valueClassName: "text-emerald-50",
            glowClassName: "bg-emerald-400/12",
        },
        {
            title: "CPU Load",
            eyebrow: "Compute",
            value: `${props.cpuValue}%`,
            detail: "Real-time runtime pressure across the active core.",
            icon: Cpu,
            spanClassName: "xl:col-span-4",
            cardClassName: "border-sky-400/20 bg-sky-500/5",
            iconWrapClassName: "border-sky-300/20 bg-sky-400/10 text-sky-100",
            valueClassName: "text-white",
            glowClassName: "bg-sky-400/12",
        },
        {
            title: "Recommendations",
            eyebrow: "Queue",
            value: String(props.pendingRecommendations),
            detail: `Pending now · total generated ${props.totalRecommendations}`,
            icon: Lightbulb,
            spanClassName: "xl:col-span-3",
            cardClassName: "border-amber-400/20 bg-amber-500/5",
            iconWrapClassName: "border-amber-300/20 bg-amber-400/10 text-amber-100",
            valueClassName: "text-white",
            glowClassName: "bg-amber-400/12",
        },
        {
            title: "Approval Rate",
            eyebrow: "Signal",
            value: `${props.approvalRate}%`,
            detail: `Last recommendation observed at ${props.lastRecTime}`,
            icon: Activity,
            spanClassName: "xl:col-span-3",
            cardClassName: "border-fuchsia-400/20 bg-fuchsia-500/5",
            iconWrapClassName: "border-fuchsia-300/20 bg-fuchsia-400/10 text-fuchsia-100",
            valueClassName: "text-white",
            glowClassName: "bg-fuchsia-400/12",
        },
        {
            title: "Memory Footprint",
            eyebrow: "Capacity",
            value: `${props.memoryUsed} GB`,
            detail: `Used of ${props.memoryTotal} GB available memory`,
            icon: HardDrive,
            spanClassName: "xl:col-span-4",
            cardClassName: "border-white/10 bg-white/5",
            iconWrapClassName: "border-white/10 bg-white/5 text-slate-100",
            valueClassName: "text-white",
            glowClassName: "bg-white/10",
        },
        {
            title: "Active Routines",
            eyebrow: "Automation",
            value: String(props.activeRoutinesCount),
            detail: "Background routines currently scheduled and available to execute.",
            icon: Activity,
            spanClassName: "xl:col-span-5",
            cardClassName: "border-white/10 bg-white/5",
            iconWrapClassName: "border-white/10 bg-white/5 text-slate-100",
            valueClassName: "text-white",
            glowClassName: "bg-cyan-300/10",
        },
    ];

    return (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-12">
            {metrics.map((metric) => {
                const Icon = metric.icon;
                return (
                    <motion.div
                        key={metric.title}
                        variants={prefersReducedMotion ? undefined : cardVariants}
                        whileHover={prefersReducedMotion ? undefined : { scale: 1.015, y: -3 }}
                        transition={
                            prefersReducedMotion
                                ? undefined
                                : { type: "spring", stiffness: 280, damping: 24 }
                        }
                        className={cn(metric.spanClassName)}
                    >
                        <Card className={cn("relative h-full overflow-hidden", metric.cardClassName)}>
                            <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-white/60 to-transparent" />
                            <div className={cn("absolute -right-10 top-0 h-28 w-28 rounded-full blur-3xl", metric.glowClassName)} />
                            <CardHeader className="relative pb-4">
                                <div className="flex items-start justify-between gap-4">
                                    <div className="space-y-3">
                                        <div className="text-[10px] uppercase tracking-[0.28em] text-slate-500">
                                            {metric.eyebrow}
                                        </div>
                                        <CardTitle className="text-base font-medium text-white/90 [text-wrap:balance]">
                                            {metric.title}
                                        </CardTitle>
                                    </div>
                                    <div className={cn("rounded-2xl border p-3", metric.iconWrapClassName)}>
                                        <Icon aria-hidden="true" className="h-4 w-4" />
                                    </div>
                                </div>
                            </CardHeader>
                            <CardContent className="relative space-y-4">
                                <div
                                    className={cn(
                                        "text-4xl font-semibold tracking-[-0.05em]",
                                        metric.valueClassName
                                    )}
                                    style={{ fontVariantNumeric: "tabular-nums" }}
                                >
                                    {metric.value}
                                </div>
                                <p className="max-w-[18rem] text-sm leading-6 text-slate-300">
                                    {metric.detail}
                                </p>
                            </CardContent>
                        </Card>
                    </motion.div>
                );
            })}
        </div>
    );
}
