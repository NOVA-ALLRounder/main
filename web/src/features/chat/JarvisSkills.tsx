import { useQuery } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { fetchJarvisSkills, type JarvisSkill } from "@/lib/api";
import { Check, X, Zap, Tag, AlertCircle } from "lucide-react";
import { cn } from "@/lib/utils";
import { motion } from "framer-motion";

export default function JarvisSkills() {
    const { data: skills, isLoading } = useQuery({
        queryKey: ["jarvis-skills"],
        queryFn: fetchJarvisSkills,
        refetchInterval: 10000 // Refresh every 10s
    });

    return (
        <Card className="border-primary/20 bg-black/40 backdrop-blur-xl">
            <CardHeader className="pb-3">
                <CardTitle className="text-lg flex items-center gap-2">
                    <Zap className="w-5 h-5 text-primary" />
                    Available Skills
                </CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
                {isLoading ? (
                    <div className="text-sm text-muted-foreground">Loading skills...</div>
                ) : skills && skills.length > 0 ? (
                    skills.map((skill: JarvisSkill) => (
                        <SkillCard key={skill.name} skill={skill} />
                    ))
                ) : (
                    <div className="text-sm text-muted-foreground">No skills available</div>
                )}
            </CardContent>
        </Card>
    );
}

function SkillCard({ skill }: { skill: JarvisSkill }) {
    return (
        <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            whileHover={{ scale: 1.02, transition: { duration: 0.2 } }}
            className={cn(
                "p-3 rounded-lg border transition-all cursor-pointer",
                skill.eligible
                    ? "border-green-500/20 bg-green-500/5 hover:bg-green-500/10 hover:border-green-500/30"
                    : "border-red-500/20 bg-red-500/5 opacity-60 hover:opacity-80"
            )}
        >
            <div className="flex items-start justify-between gap-2 mb-2">
                <div className="flex-1">
                    <div className="flex items-center gap-2 mb-1">
                        <h3 className="font-semibold text-sm">{skill.name}</h3>
                        <span className="text-xs text-muted-foreground">v{skill.version}</span>
                        {skill.eligible ? (
                            <motion.div
                                initial={{ scale: 0 }}
                                animate={{ scale: 1 }}
                                transition={{ type: "spring", stiffness: 500, damping: 30 }}
                            >
                                <Check className="w-4 h-4 text-green-500" />
                            </motion.div>
                        ) : (
                            <motion.div
                                initial={{ scale: 0 }}
                                animate={{ scale: 1 }}
                                transition={{ type: "spring", stiffness: 500, damping: 30 }}
                            >
                                <X className="w-4 h-4 text-red-500" />
                            </motion.div>
                        )}
                    </div>
                    <p className="text-xs text-muted-foreground">{skill.description}</p>
                </div>
            </div>

            {!skill.eligible && skill.reason && (
                <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    className="text-xs text-red-400 mb-2 flex items-start gap-1.5 bg-red-500/10 rounded p-2"
                >
                    <AlertCircle className="w-3 h-3 mt-0.5 shrink-0" />
                    <span>{skill.reason}</span>
                </motion.div>
            )}

            {skill.platform && (
                <div className="text-xs text-muted-foreground mb-2 flex items-center gap-1">
                    <span className="opacity-50">Platform:</span>
                    <span className="font-medium">{skill.platform}</span>
                </div>
            )}

            <div className="flex flex-wrap gap-1 mb-2">
                {skill.actions.slice(0, 5).map((action, idx) => (
                    <motion.span
                        key={action}
                        initial={{ opacity: 0, scale: 0.8 }}
                        animate={{ opacity: 1, scale: 1 }}
                        transition={{ delay: idx * 0.05 }}
                        className="text-xs px-2 py-0.5 rounded-full bg-primary/10 text-primary border border-primary/20"
                    >
                        {action}
                    </motion.span>
                ))}
                {skill.actions.length > 5 && (
                    <span className="text-xs px-2 py-0.5 text-muted-foreground">
                        +{skill.actions.length - 5} more
                    </span>
                )}
            </div>

            {skill.tags.length > 0 && (
                <div className="flex items-center gap-1.5 flex-wrap pt-2 border-t border-white/5">
                    <Tag className="w-3 h-3 text-muted-foreground" />
                    {skill.tags.map((tag) => (
                        <span
                            key={tag}
                            className="text-xs text-muted-foreground/70"
                        >
                            #{tag}
                        </span>
                    ))}
                </div>
            )}
        </motion.div>
    );
}
