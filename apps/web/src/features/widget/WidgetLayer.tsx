import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";
import { X, ExternalLink } from "lucide-react";

interface PinnedItem {
    id: string;
    type: 'text' | 'link' | 'recommendation';
    content: string;
    title?: string;
}

export default function WidgetLayer() {
    const [items, setItems] = useState<PinnedItem[]>([]);
    const isWidgetWindow = getCurrentWindow().label === 'widget';

    useEffect(() => {
        // Validation: Only run in 'widget' window
        const win = getCurrentWindow();
        if (isWidgetWindow) {
            // Listen for Pin Events
            const unlisten = listen<PinnedItem>('pin-data', (event) => {
                const newItem = event.payload;
                setItems(prev => [...prev, { ...newItem, id: Date.now().toString() }]);
                // Auto-show window if hidden (handled in Launcher or here)
                win.show();
            });

            return () => {
                unlisten.then(f => f());
            };
        }
    }, [isWidgetWindow]);

    const removeItem = (id: string) => {
        setItems(prev => prev.filter(item => item.id !== id));
        if (items.length <= 1) {
            getCurrentWindow().hide(); // Hide if empty
        }
    };

    if (!isWidgetWindow) return null;

    return (
        <div className="w-full h-screen bg-transparent p-4 flex flex-col gap-2 overflow-y-auto no-scrollbar pointer-events-auto">
            {/* Header / Drag Handle */}
            <div className="w-full h-6 rounded-full bg-white/10 hover:bg-white/20 cursor-grab active:cursor-grabbing transition-colors mb-2 flex items-center justify-center" data-tauri-drag-region>
                <div className="w-12 h-1 bg-white/30 rounded-full" />
            </div>

            <AnimatePresence>
                {items.map(item => (
                    <motion.div
                        key={item.id}
                        initial={{ opacity: 0, x: -20, scale: 0.9 }}
                        animate={{ opacity: 1, x: 0, scale: 1 }}
                        exit={{ opacity: 0, scale: 0.5, transition: { duration: 0.2 } }}
                        className="bg-[#1e1e1e]/90 backdrop-blur-md border border-white/10 rounded-lg p-3 shadow-xl relative group select-none"
                    >
                        {/* Remove Button */}
                        <button
                            onClick={() => removeItem(item.id)}
                            className="absolute top-1 right-1 p-1 rounded-full text-gray-500 hover:bg-white/10 hover:text-white opacity-0 group-hover:opacity-100 transition-all"
                        >
                            <X className="w-3 h-3" />
                        </button>

                        {/* Title */}
                        {item.title && (
                            <div className="text-xs font-bold text-gray-400 mb-1 flex items-center gap-1">
                                <ExternalLink className="w-3 h-3" />
                                {item.title}
                            </div>
                        )}

                        {/* Content */}
                        <div className="text-sm text-gray-200 break-words leading-relaxed">
                            {item.content}
                        </div>
                    </motion.div>
                ))}
            </AnimatePresence>

            {items.length === 0 && (
                <div className="text-xs text-gray-500 text-center mt-4">
                    Nothing pinned yet.
                </div>
            )}
        </div>
    );
}
