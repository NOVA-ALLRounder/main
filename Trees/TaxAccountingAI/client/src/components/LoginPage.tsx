import { useState } from "react";
import { Mail, Lock, User, Building2, Loader2 } from "lucide-react";
import { api } from "../services/api";

interface LoginPageProps {
    onSuccess: (user: any, token: string) => void;
}

export const LoginPage = ({ onSuccess }: LoginPageProps) => {
    const [mode, setMode] = useState<"login" | "register">("login");
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
    const [name, setName] = useState("");
    const [company, setCompany] = useState("");
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState("");

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError("");
        setIsLoading(true);

        try {
            let result;
            if (mode === "register") {
                result = await api.register(email, password, name, company);
            } else {
                result = await api.login(email, password);
            }

            if (result.success) {
                localStorage.setItem("token", result.token);
                localStorage.setItem("user", JSON.stringify(result.user));
                onSuccess(result.user, result.token);
            } else {
                setError(result.message);
            }
        } catch (err: any) {
            setError(err.response?.data?.message || "오류가 발생했습니다.");
        } finally {
            setIsLoading(false);
        }
    };

    return (
        <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-blue-50 via-white to-indigo-50">
            <div className="w-full max-w-md">
                {/* Logo */}
                <div className="text-center mb-8">
                    <div className="w-16 h-16 bg-gradient-to-br from-blue-600 to-indigo-600 rounded-2xl flex items-center justify-center text-white text-2xl font-bold mx-auto mb-4 shadow-lg">
                        T
                    </div>
                    <h1 className="text-3xl font-bold text-gray-900">TREES</h1>
                    <p className="text-gray-500 mt-1">AI 세무/재무 비서</p>
                </div>

                {/* Card */}
                <div className="bg-white rounded-2xl shadow-xl border p-8">
                    {/* Tab Switcher */}
                    <div className="flex mb-6 bg-gray-100 rounded-lg p-1">
                        <button
                            onClick={() => setMode("login")}
                            className={`flex-1 py-2.5 rounded-md text-sm font-medium transition-all ${mode === "login"
                                ? "bg-white shadow text-gray-900"
                                : "text-gray-500 hover:text-gray-700"
                                }`}
                        >
                            로그인
                        </button>
                        <button
                            onClick={() => setMode("register")}
                            className={`flex-1 py-2.5 rounded-md text-sm font-medium transition-all ${mode === "register"
                                ? "bg-white shadow text-gray-900"
                                : "text-gray-500 hover:text-gray-700"
                                }`}
                        >
                            회원가입
                        </button>
                    </div>

                    {/* Error Message */}
                    {error && (
                        <div className="mb-4 p-3 bg-red-50 border border-red-200 text-red-600 rounded-lg text-sm">
                            {error}
                        </div>
                    )}

                    {/* Form */}
                    <form onSubmit={handleSubmit} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1.5">
                                이메일
                            </label>
                            <div className="relative">
                                <Mail className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" />
                                <input
                                    type="email"
                                    required
                                    value={email}
                                    onChange={(e) => setEmail(e.target.value)}
                                    className="w-full pl-10 pr-4 py-3 border rounded-lg focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 outline-none transition-all"
                                    placeholder="email@example.com"
                                />
                            </div>
                        </div>

                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1.5">
                                비밀번호
                            </label>
                            <div className="relative">
                                <Lock className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" />
                                <input
                                    type="password"
                                    required
                                    value={password}
                                    onChange={(e) => setPassword(e.target.value)}
                                    className="w-full pl-10 pr-4 py-3 border rounded-lg focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 outline-none transition-all"
                                    placeholder="••••••••"
                                />
                            </div>
                        </div>

                        {mode === "register" && (
                            <>
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 mb-1.5">
                                        이름
                                    </label>
                                    <div className="relative">
                                        <User className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" />
                                        <input
                                            type="text"
                                            required
                                            value={name}
                                            onChange={(e) => setName(e.target.value)}
                                            className="w-full pl-10 pr-4 py-3 border rounded-lg focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 outline-none transition-all"
                                            placeholder="홍길동"
                                        />
                                    </div>
                                </div>

                                <div>
                                    <label className="block text-sm font-medium text-gray-700 mb-1.5">
                                        회사명
                                    </label>
                                    <div className="relative">
                                        <Building2 className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" />
                                        <input
                                            type="text"
                                            required
                                            value={company}
                                            onChange={(e) => setCompany(e.target.value)}
                                            className="w-full pl-10 pr-4 py-3 border rounded-lg focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 outline-none transition-all"
                                            placeholder="(주)회사명"
                                        />
                                    </div>
                                </div>
                            </>
                        )}

                        <button
                            type="submit"
                            disabled={isLoading}
                            className="w-full py-3 bg-gradient-to-r from-blue-600 to-indigo-600 text-white rounded-lg font-semibold hover:from-blue-700 hover:to-indigo-700 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                        >
                            {isLoading ? (
                                <>
                                    <Loader2 className="w-5 h-5 animate-spin" />
                                    처리 중...
                                </>
                            ) : mode === "login" ? (
                                "로그인"
                            ) : (
                                "회원가입"
                            )}
                        </button>
                    </form>

                    {/* Footer */}
                    <p className="mt-6 text-center text-sm text-gray-500">
                        {mode === "login" ? (
                            <>
                                계정이 없으신가요?{" "}
                                <button
                                    onClick={() => setMode("register")}
                                    className="text-blue-600 hover:underline font-medium"
                                >
                                    회원가입
                                </button>
                                <br />
                                <button
                                    onClick={() => {
                                        const resetEmail = prompt("비밀번호를 재설정할 이메일을 입력하세요:");
                                        if (resetEmail) {
                                            alert(`${resetEmail}로 비밀번호 재설정 안내 메일이 발송되었습니다.\n(MVP 데모: 실제로는 발송되지 않습니다)`);
                                        }
                                    }}
                                    className="text-gray-400 hover:text-gray-600 text-xs mt-2"
                                >
                                    비밀번호를 잊으셨나요?
                                </button>
                            </>
                        ) : (
                            <>
                                이미 계정이 있으신가요?{" "}
                                <button
                                    onClick={() => setMode("login")}
                                    className="text-blue-600 hover:underline font-medium"
                                >
                                    로그인
                                </button>
                            </>
                        )}
                    </p>
                </div>

                <p className="mt-6 text-center text-xs text-gray-400">
                    © 2026 TREES - AI Tax & Accounting Assistant
                </p>
            </div>
        </div>
    );
};
