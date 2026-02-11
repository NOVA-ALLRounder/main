import { useState, useRef, useEffect } from "react";
import { Send, Bot, User, Sparkles, Copy, CheckCheck, RotateCw, Mic, Info } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { sendJarvisWebMessage } from "@/lib/api";
import { cn } from "@/lib/utils";
import { useMutation } from "@tanstack/react-query";
import { motion, AnimatePresence } from "framer-motion";

interface Message {
    role: "user" | "assistant";
    text: string;
    timestamp: number;
    copied?: boolean;
    error?: boolean;
}

export default function ChatPanel() {
    const [input, setInput] = useState("");
    const [messages, setMessages] = useState<Message[]>([
        {
            role: "assistant",
            text: "안녕하세요! JARVIS입니다. 무엇을 도와드릴까요?\n\n예시:\n• 메모장 열고 안녕이라고 적어줘\n• 이메일 확인해줘\n• 텔레그램으로 메시지 보내줘",
            timestamp: Date.now()
        }
    ]);
    const [showSessionInfo, setShowSessionInfo] = useState(false);
    const messagesEndRef = useRef<HTMLDivElement>(null);

    // Generate session key for this browser session
    const sessionKey = useRef(`web:${Math.random().toString(36).substr(2, 9)}`);

    const mutation = useMutation({
        mutationFn: async (text: string) => {
            // Use new Web Channel API with Intent → Plan → Skills pipeline
            return sendJarvisWebMessage({
                text,
                session_key: sessionKey.current,
                metadata: {}
            });
        },
        onSuccess: (data) => {
            if (data.success) {
                setMessages(prev => [
                    ...prev,
                    {
                        role: "assistant",
                        text: data.message,
                        timestamp: Date.now(),
                        error: false
                    }
                ]);
            } else {
                setMessages(prev => [
                    ...prev,
                    {
                        role: "assistant",
                        text: data.message,
                        timestamp: Date.now(),
                        error: true
                    }
                ]);
            }
        },
        onError: (error) => {
            setMessages(prev => [
                ...prev,
                {
                    role: "assistant",
                    text: `Failed to reach JARVIS. Is the backend running?\n\nError: ${error instanceof Error ? error.message : "Unknown error"}`,
                    timestamp: Date.now(),
                    error: true
                }
            ]);
        }
    });

    const handleSend = () => {
        if (!input.trim() || mutation.isPending) return;

        // Add user message with timestamp
        setMessages(prev => [...prev, {
            role: "user",
            text: input,
            timestamp: Date.now()
        }]);

        // Send to JARVIS
        mutation.mutate(input);

        setInput("");
    };

    const handleCopy = (index: number, text: string) => {
        navigator.clipboard.writeText(text);
        setMessages(prev => prev.map((msg, i) =>
            i === index ? { ...msg, copied: true } : msg
        ));
        setTimeout(() => {
            setMessages(prev => prev.map((msg, i) =>
                i === index ? { ...msg, copied: false } : msg
            ));
        }, 2000);
    };

    const handleRetry = (text: string) => {
        setMessages(prev => [...prev, {
            role: "user",
            text,
            timestamp: Date.now()
        }]);
        mutation.mutate(text);
    };

    const formatTimestamp = (timestamp: number) => {
        const date = new Date(timestamp);
        return date.toLocaleTimeString('ko-KR', {
            hour: '2-digit',
            minute: '2-digit'
        });
    };

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }, [messages]);

    return (
        <Card className="h-full flex flex-col border-primary/20 bg-black/40 backdrop-blur-xl">
            <CardHeader className="py-4 border-b border-white/5">
                <div className="flex items-center justify-between">
                    <CardTitle className="text-lg flex items-center gap-2">
                        <Sparkles className="w-5 h-5 text-primary" />
                        JARVIS Assistant
                    </CardTitle>
                    <motion.button
                        onClick={() => setShowSessionInfo(!showSessionInfo)}
                        className="p-2 rounded-lg hover:bg-white/5 transition-colors cursor-pointer"
                        whileHover={{ scale: 1.05 }}
                        whileTap={{ scale: 0.95 }}
                    >
                        <Info className="w-4 h-4 text-muted-foreground" />
                    </motion.button>
                </div>
                <AnimatePresence>
                    {showSessionInfo && (
                        <motion.div
                            initial={{ opacity: 0, height: 0 }}
                            animate={{ opacity: 1, height: "auto" }}
                            exit={{ opacity: 0, height: 0 }}
                            className="mt-2 text-xs text-muted-foreground bg-white/5 rounded-lg p-2"
                        >
                            Session: {sessionKey.current}
                        </motion.div>
                    )}
                </AnimatePresence>
            </CardHeader>
            <CardContent className="flex-1 flex flex-col p-0 overflow-hidden">
                {/* Messages Area */}
                <div className="flex-1 overflow-y-auto p-4 space-y-4">
                    {messages.map((msg, idx) => (
                        <motion.div
                            key={idx}
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.2 }}
                            className={cn("flex w-full gap-3", msg.role === "user" ? "justify-end" : "justify-start")}
                        >
                            {msg.role === "assistant" && (
                                <div className="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center shrink-0 mt-1">
                                    <Bot className="w-4 h-4 text-primary" />
                                </div>
                            )}
                            <div className="flex flex-col gap-1 max-w-[80%]">
                                <div className={cn(
                                    "rounded-2xl px-4 py-3 text-sm leading-relaxed shadow-md group relative",
                                    msg.role === "user"
                                        ? "bg-gradient-to-br from-primary to-primary/80 text-primary-foreground rounded-tr-none"
                                        : cn(
                                            "bg-white/10 text-foreground rounded-tl-none border backdrop-blur-sm",
                                            msg.error ? "border-red-500/30" : "border-white/5"
                                        )
                                )}>
                                    {msg.text.split('\n').map((line, i) => (
                                        <p key={i} className="min-h-[1.2em]">{line || '\u00A0'}</p>
                                    ))}

                                    {/* Action buttons for assistant messages */}
                                    {msg.role === "assistant" && (
                                        <div className="flex gap-1 mt-2 pt-2 border-t border-white/5">
                                            <motion.button
                                                onClick={() => handleCopy(idx, msg.text)}
                                                className="p-1.5 rounded hover:bg-white/5 transition-colors cursor-pointer"
                                                whileHover={{ scale: 1.1 }}
                                                whileTap={{ scale: 0.9 }}
                                            >
                                                {msg.copied ? (
                                                    <CheckCheck className="w-3 h-3 text-green-500" />
                                                ) : (
                                                    <Copy className="w-3 h-3 text-muted-foreground" />
                                                )}
                                            </motion.button>
                                            {msg.error && (
                                                <motion.button
                                                    onClick={() => {
                                                        const userMsg = messages[idx - 1];
                                                        if (userMsg?.role === "user") {
                                                            handleRetry(userMsg.text);
                                                        }
                                                    }}
                                                    className="p-1.5 rounded hover:bg-white/5 transition-colors cursor-pointer"
                                                    whileHover={{ scale: 1.1 }}
                                                    whileTap={{ scale: 0.9 }}
                                                >
                                                    <RotateCw className="w-3 h-3 text-orange-500" />
                                                </motion.button>
                                            )}
                                        </div>
                                    )}
                                </div>
                                <span className="text-xs text-muted-foreground/50 px-2">
                                    {formatTimestamp(msg.timestamp)}
                                </span>
                            </div>
                            {msg.role === "user" && (
                                <div className="w-8 h-8 rounded-full bg-white/10 flex items-center justify-center shrink-0 mt-1">
                                    <User className="w-4 h-4" />
                                </div>
                            )}
                        </motion.div>
                    ))}

                    {mutation.isPending && (
                        <motion.div
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            className="flex w-full gap-3 justify-start"
                        >
                            <div className="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center shrink-0 mt-1">
                                <Bot className="w-4 h-4 text-primary animate-pulse" />
                            </div>
                            <div className="bg-white/5 border border-white/5 rounded-2xl px-4 py-3 rounded-tl-none flex flex-col gap-2">
                                <div className="flex items-center gap-1.5">
                                    <motion.span
                                        className="w-2 h-2 bg-primary rounded-full"
                                        animate={{ scale: [1, 1.3, 1], opacity: [1, 0.5, 1] }}
                                        transition={{ duration: 1, repeat: Infinity, delay: 0 }}
                                    />
                                    <motion.span
                                        className="w-2 h-2 bg-primary rounded-full"
                                        animate={{ scale: [1, 1.3, 1], opacity: [1, 0.5, 1] }}
                                        transition={{ duration: 1, repeat: Infinity, delay: 0.2 }}
                                    />
                                    <motion.span
                                        className="w-2 h-2 bg-primary rounded-full"
                                        animate={{ scale: [1, 1.3, 1], opacity: [1, 0.5, 1] }}
                                        transition={{ duration: 1, repeat: Infinity, delay: 0.4 }}
                                    />
                                </div>
                                <span className="text-xs text-muted-foreground">Processing...</span>
                            </div>
                        </motion.div>
                    )}
                    <div ref={messagesEndRef} />
                </div>

                {/* Input Area */}
                <div className="p-4 bg-white/5 border-t border-white/5">
                    <div className="flex gap-2">
                        <div className="flex-1 relative">
                            <input
                                value={input}
                                onChange={(e) => setInput(e.target.value)}
                                onKeyDown={(e) => {
                                    if (e.key === "Enter" && !e.shiftKey) {
                                        e.preventDefault();
                                        handleSend();
                                    }
                                }}
                                placeholder="메모장 열고 안녕이라고 적어줘..."
                                className="w-full bg-black/20 border border-white/10 rounded-xl px-4 py-3 pr-12 text-sm focus:outline-none focus:border-primary/50 transition-colors placeholder:text-muted-foreground/50"
                                disabled={mutation.isPending}
                            />
                            <motion.button
                                onClick={() => {
                                    // TODO: Implement voice input
                                    console.log("Voice input not yet implemented");
                                }}
                                className="absolute right-2 top-1/2 -translate-y-1/2 p-2 rounded-lg hover:bg-white/5 transition-colors cursor-pointer"
                                whileHover={{ scale: 1.1 }}
                                whileTap={{ scale: 0.9 }}
                                disabled={mutation.isPending}
                            >
                                <Mic className="w-4 h-4 text-muted-foreground" />
                            </motion.button>
                        </div>
                        <motion.button
                            onClick={handleSend}
                            disabled={!input.trim() || mutation.isPending}
                            className="px-4 py-3 rounded-xl bg-gradient-to-br from-primary to-primary/80 text-primary-foreground flex items-center justify-center gap-2 hover:opacity-90 disabled:opacity-30 disabled:cursor-not-allowed transition-all shadow-lg cursor-pointer"
                            whileHover={{ scale: 1.05 }}
                            whileTap={{ scale: 0.95 }}
                        >
                            <Send className="w-4 h-4" />
                        </motion.button>
                    </div>
                    <div className="flex items-center justify-between mt-2 text-xs text-muted-foreground/50">
                        <span>Powered by GPT-4o-mini</span>
                        <span>{messages.length - 1} messages</span>
                    </div>
                </div>
            </CardContent>
        </Card>
    );
}
