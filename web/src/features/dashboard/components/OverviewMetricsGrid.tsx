import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Activity, Cpu, HardDrive, Lightbulb, ShieldCheck } from "lucide-react";
import { motion } from "framer-motion";

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
    return (
        <>
            <motion.div variants={cardVariants} whileHover={{ scale: 1.02, y: -2 }} transition={{ type: "spring", stiffness: 300 }}>
                <Card className="border-primary/20 bg-primary/5 h-full">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">CPU Load</CardTitle>
                        <Cpu className="h-4 w-4 text-primary" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold" style={{ fontVariantNumeric: "tabular-nums" }}>
                            {props.cpuValue}%
                        </div>
                        <p className="text-xs text-muted-foreground">Real-time usage</p>
                    </CardContent>
                </Card>
            </motion.div>

            <motion.div variants={cardVariants} whileHover={{ scale: 1.02, y: -2 }} transition={{ type: "spring", stiffness: 300 }}>
                <Card className="h-full">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Memory</CardTitle>
                        <HardDrive className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold" style={{ fontVariantNumeric: "tabular-nums" }}>
                            {props.memoryUsed} GB
                        </div>
                        <p className="text-xs text-muted-foreground">Used of {props.memoryTotal} GB</p>
                    </CardContent>
                </Card>
            </motion.div>

            <motion.div variants={cardVariants} whileHover={{ scale: 1.02, y: -2 }} transition={{ type: "spring", stiffness: 300 }}>
                <Card className="h-full">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Active Routines</CardTitle>
                        <Activity className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold" style={{ fontVariantNumeric: "tabular-nums" }}>
                            {props.activeRoutinesCount}
                        </div>
                        <p className="text-xs text-muted-foreground">Running perfectly</p>
                    </CardContent>
                </Card>
            </motion.div>

            <motion.div variants={cardVariants} whileHover={{ scale: 1.02, y: -2 }} transition={{ type: "spring", stiffness: 300 }}>
                <Card className="h-full">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Recommendations</CardTitle>
                        <Lightbulb className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold" style={{ fontVariantNumeric: "tabular-nums" }}>
                            {props.pendingRecommendations}
                        </div>
                        <p className="text-xs text-muted-foreground">
                            Pending · Total {props.totalRecommendations}
                        </p>
                    </CardContent>
                </Card>
            </motion.div>

            <motion.div variants={cardVariants} whileHover={{ scale: 1.02, y: -2 }} transition={{ type: "spring", stiffness: 300 }}>
                <Card className="h-full">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Approval Rate</CardTitle>
                        <Activity className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold" style={{ fontVariantNumeric: "tabular-nums" }}>
                            {props.approvalRate}%
                        </div>
                        <p className="text-xs text-muted-foreground">Last rec {props.lastRecTime}</p>
                    </CardContent>
                </Card>
            </motion.div>

            <motion.div variants={cardVariants} whileHover={{ scale: 1.02, y: -2 }} transition={{ type: "spring", stiffness: 300 }}>
                <Card className="h-full border-emerald-500/20 bg-emerald-500/5">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Quality Score</CardTitle>
                        <ShieldCheck className="h-4 w-4 text-emerald-400" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold" style={{ fontVariantNumeric: "tabular-nums" }}>
                            {props.qualityValue}
                        </div>
                        <p className="text-xs text-muted-foreground">
                            {props.qualityLabel} · {props.qualityTime}
                        </p>
                    </CardContent>
                </Card>
            </motion.div>
        </>
    );
}
