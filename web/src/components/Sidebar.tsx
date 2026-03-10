import { Home, PlayCircle, Settings, FileText, MessageCircle } from "lucide-react";
import { useSystemStatus } from "@/lib/hooks";
import { cn } from "@/lib/utils";

const navItems = [
    { icon: Home, label: "Dashboard", id: "dashboard" },
    { icon: PlayCircle, label: "Routines", id: "routines" },
    { icon: FileText, label: "Workflows", id: "workflows" },
    { icon: MessageCircle, label: "Chat", id: "chat" },
    { icon: Settings, label: "Settings", id: "settings" },
];

interface SidebarProps {
    active: string;
    onNavigate: (id: string) => void;
}

export function Sidebar({ active, onNavigate }: SidebarProps) {
    const { isError, isFetching } = useSystemStatus();
    const isOffline = isError;
    const statusLabel = isOffline ? "System Offline" : isFetching ? "System Syncing" : "System Online";
    const statusDotClass = isOffline
        ? "bg-rose-400 shadow-[0_0_10px_#fb7185]"
        : isFetching
          ? "bg-amber-400 shadow-[0_0_10px_#fbbf24]"
          : "bg-green-500 shadow-[0_0_10px_#22c55e]";

    return (
        <aside className="flex shrink-0 flex-col border-b border-white/5 bg-black/40 backdrop-blur-xl lg:h-full lg:w-64 lg:border-b-0 lg:border-r">
            <div className="hidden items-center gap-3 px-6 pb-6 pt-20 lg:flex">
                <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-slate-800 to-slate-950 border border-cyan-300/40 flex items-center justify-center text-[11px] font-bold tracking-wide">
                    <span className="text-cyan-300">A</span>
                    <span className="text-white/90">I</span>
                </div>
                <h1 className="text-xl font-bold tracking-tight text-white/90">
                    <span className="text-cyan-300 font-extrabold">A</span>llv
                    <span className="text-cyan-300 font-extrabold">I</span>a
                </h1>
            </div>

            <nav className="flex gap-2 overflow-x-auto px-4 pb-4 pt-16 sm:pt-20 lg:mt-4 lg:flex-1 lg:block lg:space-y-2 lg:px-4 lg:pb-0 lg:pt-0">
                {navItems.map((item) => (
                    <button
                        key={item.id}
                        onClick={() => onNavigate(item.id)}
                        className={cn(
                            "group flex min-w-fit shrink-0 items-center gap-2 rounded-xl px-3 py-2 text-xs font-medium transition-all duration-200 sm:text-sm lg:w-full lg:min-w-0 lg:gap-3 lg:px-4 lg:py-3",
                            active === item.id
                                ? "bg-primary/20 text-primary shadow-[0_0_20px_rgba(59,130,246,0.3)] border border-primary/20"
                                : "text-muted-foreground hover:bg-white/5 hover:text-white"
                        )}
                    >
                        <item.icon
                            aria-hidden="true"
                            className={cn("w-5 h-5 transition-transform group-hover:scale-110", active === item.id ? "text-primary" : "")}
                        />
                        {item.label}
                    </button>
                ))}
            </nav>

            <div className="px-4 pb-4 lg:hidden">
                <div className="glass inline-flex rounded-xl px-4 py-2">
                    <div className="flex items-center gap-3">
                        <div
                            aria-hidden="true"
                            className={cn("w-2 h-2 rounded-full", statusDotClass)}
                        />
                        <span className="text-xs font-mono text-muted-foreground">
                            {statusLabel}
                        </span>
                    </div>
                </div>
            </div>

            <div className="hidden p-6 lg:block">
                <div className="glass rounded-xl p-4 flex items-center gap-3">
                    <div
                        aria-hidden="true"
                        className={cn("w-2 h-2 rounded-full", statusDotClass)}
                    />
                    <span className="text-xs font-mono text-muted-foreground">
                        {statusLabel}
                    </span>
                </div>
            </div>
        </aside>
    );
}
