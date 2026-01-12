import { useState, useEffect } from "react";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import {
  Rocket,
  Stethoscope,
  ShoppingBag,
  Building2,
  LayoutDashboard,
  MessageSquare,
  Menu,
  Bell,
  User,
  Send,
  Loader2,
  Calendar,
  ExternalLink,
  AlertTriangle,
  CheckCircle,
  Calculator,
  ChevronRight,
  TrendingUp,
  Activity,
  Settings,
  PlusCircle,
  XCircle,
  CheckSquare,
  Square
} from "lucide-react";
import {
  api,
  type ChatMessage,
  type Recommendation,
  type Competition,
  type RiskAnalysis,
  type CalendarAlert
} from "./services/api";

// --- Components ---

const SidebarItem = ({
  icon: Icon,
  label,
  active,
  onClick,
}: {
  icon: any;
  label: string;
  active?: boolean;
  onClick: () => void;
}) => (
  <button
    onClick={onClick}
    className={`w-full flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors ${active
      ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
      : "text-sidebar-foreground hover:bg-sidebar-accent/50"
      }`}
  >
    <Icon className="w-4 h-4" />
    {label}
  </button>
);

const KPICard = ({
  label,
  value,
  trend,
  status,
}: {
  label: string;
  value: string;
  trend: string;
  status: "good" | "warning" | "info";
}) => {
  const statusColors: any = {
    good: "text-emerald-600 bg-emerald-50",
    warning: "text-amber-600 bg-amber-50",
    info: "text-blue-600 bg-blue-50",
  };

  return (
    <div className="p-6 bg-card rounded-xl border shadow-sm">
      <div className="flex justify-between items-start mb-4">
        <h3 className="text-sm font-medium text-muted-foreground">{label}</h3>
        <span
          className={`text-xs px-2 py-1 rounded-full font-medium ${statusColors[status]}`}
        >
          {trend}
        </span>
      </div>
      <div className="text-2xl font-bold">{value}</div>
    </div>
  );
};

// 1. Risk Warning System (AI CFO Mode)
const RiskCard = ({ risk }: { risk: RiskAnalysis }) => {
  const isPlanningMode = risk.title.includes("설계"); // Detect Pre-Founder mode

  const levelColor = isPlanningMode ? 'bg-blue-500' : risk.level === 'critical' ? 'bg-red-500' : risk.level === 'warning' ? 'bg-amber-500' : 'bg-emerald-500';
  const levelBg = isPlanningMode ? 'bg-blue-50 border-blue-100' : risk.level === 'critical' ? 'bg-red-50 border-red-100' : risk.level === 'warning' ? 'bg-amber-50 border-amber-100' : 'bg-emerald-50 border-emerald-100';

  return (
    <div className={`rounded-xl border ${levelBg} flex flex-col overflow-hidden`}>
      {/* 1. Top Section: Risk Score & Money Impact */}
      <div className="p-6 pb-2 flex flex-col md:flex-row gap-6">
        {/* Left: Score & Money */}
        <div className="flex-1 space-y-4">
          <div className="flex items-center gap-2">
            <div className={`w-3 h-3 rounded-full ${levelColor} animate-pulse`} />
            <span className="font-bold text-lg text-primary-foreground/80 md:text-gray-800">
              {isPlanningMode ? "AI 세무 설계 (Planning)" : "AI Tax CFO 진단"}
            </span>
            <span className="text-xs bg-white/50 px-2 py-0.5 rounded border">AI Estimation</span>
          </div>

          <div>
            <h2 className="text-2xl font-bold mb-1">{risk.title}</h2>
            <p className="text-muted-foreground text-sm mb-4">{risk.reason}</p>

            {risk.estimated_penalty > 0 ? (
              <div className="bg-white/60 p-4 rounded-xl border border-black/5 shadow-sm inline-block min-w-[280px]">
                <p className="text-xs text-muted-foreground font-medium mb-1">
                  {isPlanningMode ? "전략적 의사결정 시 절세 효과" : "지금 조치 안 하면 낼 세금 (예상)"}
                </p>
                <div className={`text-3xl font-extrabold flex items-center gap-2 ${isPlanningMode ? 'text-blue-600' : 'text-red-600'}`}>
                  {isPlanningMode ? '+' : '+'}{risk.estimated_penalty.toLocaleString()}원
                  {!isPlanningMode && <AlertTriangle className="w-6 h-6 animate-bounce" />}
                </div>
                <p className="text-xs text-muted-foreground mt-1 font-medium">
                  {isPlanningMode ? "👔 지금 시작하면 이만큼 아낍니다!" : "🚨 3일 내 조치 시 세금 절약 가능"}
                </p>
              </div>
            ) : (
              <div className="bg-emerald-100/50 p-4 rounded-xl border border-emerald-200 inline-block">
                <div className="text-xl font-bold text-emerald-700 flex items-center gap-2">
                  <CheckCircle className="w-6 h-6" />
                  세무 건전성 Safe
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Right: Action Items (The "To-Do" List) */}
        <div className="flex-[1.2] bg-white rounded-xl p-5 border shadow-sm">
          <h3 className="font-bold text-gray-800 mb-3 flex justify-between items-center">
            <span>{isPlanningMode ? "🚀 창업 준비 체크리스트" : "⚡️ 지금 바로 해결하기 (Action Plan)"}</span>
            <span className={`text-xs px-2 py-1 rounded-full ${isPlanningMode ? 'bg-blue-100 text-blue-600' : 'bg-red-100 text-red-600'}`}>
              {risk.action_items.length}건 대기중
            </span>
          </h3>
          <div className="space-y-3">
            {risk.action_items.map((item, i) => (
              <div key={i} className="flex items-center justify-between p-3 rounded-lg border hover:bg-gray-50 transition-colors group cursor-pointer">
                <div className="flex items-center gap-3">
                  <div className="w-5 h-5 rounded border-2 border-gray-300 group-hover:border-primary flex items-center justify-center transition-colors">
                    {/* Checkbox simulation */}
                  </div>
                  <div>
                    <div className="text-sm font-bold text-gray-800">{item.task}</div>
                    {item.amount > 0 && (
                      <div className="text-xs text-muted-foreground">
                        비용/효과: <span className="font-medium text-gray-700">{item.amount.toLocaleString()}원</span>
                      </div>
                    )}
                  </div>
                </div>
                {item.risk_reduction > 0 && (
                  <div className="text-xs font-bold text-emerald-600 bg-emerald-50 px-2 py-1 rounded">
                    위험도 -{item.risk_reduction}
                  </div>
                )}
                {isPlanningMode && (
                  <div className="text-xs font-bold text-blue-600 bg-blue-50 px-2 py-1 rounded">
                    {item.deadline}
                  </div>
                )}
              </div>
            ))}
          </div>
          {risk.action_items.length === 0 && <p className="text-sm text-gray-400 text-center py-4">조치할 항목이 없습니다.</p>}
        </div>
      </div>

      {/* 2. Bottom Section: Missing Proof Radar (For Existing Biz) */}
      {!isPlanningMode && risk.missing_proofs && risk.missing_proofs.length > 0 && (
        <div className="mx-6 mb-6 mt-2 bg-white/50 rounded-xl border overflow-hidden">
          <div className="px-4 py-2 border-b bg-gray-50/50 flex justify-between items-center">
            <h4 className="text-sm font-bold text-gray-700 flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-red-500" />
              증빙 누락 의심 거래 (Missing Proof Radar)
            </h4>
            <span className="text-xs text-muted-foreground">자동 수집된 카드 내역 중 증빙 미매칭 항목</span>
          </div>
          <div className="divide-y">
            {risk.missing_proofs.map((proof, i) => (
              <div key={i} className="px-4 py-3 flex justify-between items-center text-sm hover:bg-white transition-colors">
                <div className="flex gap-4">
                  <span className="text-gray-500 font-mono text-xs w-20">{proof.date}</span>
                  <span className="font-medium text-gray-800">{proof.merchant}</span>
                </div>
                <div className="flex gap-4 items-center">
                  <span className="font-bold">{proof.amount.toLocaleString()}원</span>
                  <span className="text-xs bg-red-100 text-red-600 px-2 py-0.5 rounded">{proof.type}</span>
                  <button className="text-xs border px-2 py-1 rounded hover:bg-gray-100">소명하기</button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

// 2. Tax Simulator
const TaxSimulator = () => {
  const [toggles, setToggles] = useState({ salary_increase: false, vehicle_expense: false, rnd_credit: false });
  const [result, setResult] = useState<any>(null);

  useEffect(() => {
    // Initial sim
    handleSimulate();
  }, [toggles]);

  const handleSimulate = async () => {
    try {
      const res = await api.simulateTax(toggles);
      setResult(res);
    } catch (e) { console.error(e); }
  };

  const toggle = (key: keyof typeof toggles) => {
    setToggles(prev => ({ ...prev, [key]: !prev[key] }));
  };

  return (
    <div className="p-6 bg-card rounded-xl border shadow-sm h-full flex flex-col">
      <div className="flex justify-between items-center mb-4">
        <h3 className="font-bold flex items-center gap-2">
          <Calculator className="w-5 h-5 text-primary" />
          절세 시뮬레이터
        </h3>
        <span className="text-xs bg-sidebar-accent px-2 py-1 rounded">What-If 분석</span>
      </div>

      <div className="space-y-3 mb-6 flex-1">
        <div
          onClick={() => toggle('salary_increase')}
          className={`p-3 rounded-lg border cursor-pointer transition-colors flex items-center gap-3 ${toggles.salary_increase ? 'bg-primary/5 border-primary' : 'hover:bg-muted/50'}`}
        >
          <div className={`w-5 h-5 rounded border flex items-center justify-center ${toggles.salary_increase ? 'bg-primary border-primary text-white' : 'border-gray-400'}`}>
            {toggles.salary_increase && <CheckCircle className="w-3.5 h-3.5" />}
          </div>
          <span className="text-sm font-medium">대표자 급여 인상 (월 150만원↑)</span>
        </div>

        <div
          onClick={() => toggle('vehicle_expense')}
          className={`p-3 rounded-lg border cursor-pointer transition-colors flex items-center gap-3 ${toggles.vehicle_expense ? 'bg-primary/5 border-primary' : 'hover:bg-muted/50'}`}
        >
          <div className={`w-5 h-5 rounded border flex items-center justify-center ${toggles.vehicle_expense ? 'bg-primary border-primary text-white' : 'border-gray-400'}`}>
            {toggles.vehicle_expense && <CheckCircle className="w-3.5 h-3.5" />}
          </div>
          <span className="text-sm font-medium">업무용 승용차 비용 처리</span>
        </div>

        <div
          onClick={() => toggle('rnd_credit')}
          className={`p-3 rounded-lg border cursor-pointer transition-colors flex items-center gap-3 ${toggles.rnd_credit ? 'bg-primary/5 border-primary' : 'hover:bg-muted/50'}`}
        >
          <div className={`w-5 h-5 rounded border flex items-center justify-center ${toggles.rnd_credit ? 'bg-primary border-primary text-white' : 'border-gray-400'}`}>
            {toggles.rnd_credit && <CheckCircle className="w-3.5 h-3.5" />}
          </div>
          <span className="text-sm font-medium">R&D 세액 공제 적용</span>
        </div>
      </div>

      <div className="bg-muted/30 p-4 rounded-lg border-t border-dashed border-gray-200">
        <p className="text-xs text-muted-foreground mb-1">예상 절세 효과</p>
        <div className="text-2xl font-bold text-primary">
          {result?.total_saving.toLocaleString()}원
          <span className="text-sm font-normal text-muted-foreground ml-1">절약</span>
        </div>
      </div>
    </div>
  );
};

// 3. Tax Calendar
const TaxCalendar = ({ alerts }: { alerts: CalendarAlert[] }) => (
  <div className="p-6 bg-card rounded-xl border shadow-sm h-full flex flex-col">
    <h3 className="font-bold flex items-center gap-2 mb-4">
      <Calendar className="w-5 h-5 text-primary" />
      세무 일정 (30일 이내)
    </h3>
    <div className="space-y-4 flex-1 overflow-y-auto pr-1">
      {alerts.map((alert, i) => (
        <div key={i} className="flex gap-4 items-center">
          <div className={`w-12 h-12 rounded-lg flex flex-col items-center justify-center text-xs font-bold ${alert.d_day <= 7 ? 'bg-red-100 text-red-600' : 'bg-secondary text-secondary-foreground'}`}>
            <span>D-{alert.d_day}</span>
          </div>
          <div className="flex-1">
            <h4 className="font-medium text-sm">{alert.title}</h4>
            <p className="text-xs text-muted-foreground">{alert.date} 마감</p>
          </div>
          {alert.type === 'mandatory' && <span className="text-[10px] bg-red-100 text-red-600 px-1.5 py-0.5 rounded">필수</span>}
        </div>
      ))}
    </div>
    <button className="w-full mt-4 py-2 text-xs font-medium text-muted-foreground hover:bg-muted rounded-md transition-colors flex items-center justify-center gap-1">
      전체 일정 보기 <ChevronRight className="w-3 h-3" />
    </button>
  </div>
);

const PredictionCard = ({ rec }: { rec: Recommendation }) => (
  <div className="p-6 bg-card rounded-xl border shadow-sm hover:shadow-md transition-shadow">
    <div className="flex justify-between items-start mb-2">
      <div className="flex items-center gap-2">
        <Calendar className="w-5 h-5 text-primary" />
        <h3 className="font-bold text-lg">{rec.title}</h3>
      </div>
      <span className="text-xs bg-sidebar-accent px-2 py-1 rounded">2026 예측</span>
    </div>
    <div className="mt-4 space-y-2">
      <p className="text-sm text-muted-foreground">예상 공고일</p>
      <p className="text-xl font-semibold text-primary">{rec.predicted_date}</p>
      <p className="text-xs text-muted-foreground">구간: {rec.range}</p>
    </div>
    <div className="mt-4 pt-4 border-t text-xs text-muted-foreground">
      근거: {rec.reason} ({rec.confidence})
    </div>
  </div>
);

const CompetitionCard = ({ comp }: { comp: Competition }) => (
  <div className="p-5 bg-card rounded-xl border shadow-sm flex flex-col gap-3">
    <div className="flex justify-between items-start">
      <div className="flex gap-2 items-center">
        <span className={`text-xs font-bold px-2 py-1 rounded ${comp.platform === 'Kaggle' ? 'bg-sky-100 text-sky-700' : 'bg-indigo-100 text-indigo-700'}`}>
          {comp.platform}
        </span>
        <h3 className="font-semibold text-base">{comp.title}</h3>
      </div>
      <a href={comp.link} target="_blank" rel="noreferrer" className="text-muted-foreground hover:text-primary">
        <ExternalLink className="w-4 h-4" />
      </a>
    </div>
    <p className="text-sm text-muted-foreground line-clamp-2">{comp.description}</p>
    <div className="flex flex-wrap gap-1 mt-auto">
      {comp.tags.map(tag => (
        <span key={tag} className="text-[10px] bg-secondary px-1.5 py-0.5 rounded text-secondary-foreground">
          #{tag}
        </span>
      ))}
    </div>
    <div className="text-xs font-medium mt-1 text-red-500">
      마감: {comp.deadline}
    </div>
  </div>
);

// 5. Data Scraping Simulation
const LoadingScraper = ({ onComplete }: { onComplete: () => void }) => {
  const steps = [
    "국세청 홈택스 연결 중...",
    "전자세금계산서 내역 수집 중...",
    "신용카드 매입 내역 분석 중...",
    "금융결제원 계좌 조회 중...",
    "AI 세무 리스크 분석 중..."
  ];
  const [currentStep, setCurrentStep] = useState(0);

  useEffect(() => {
    if (currentStep < steps.length) {
      const timeout = setTimeout(() => {
        setCurrentStep(prev => prev + 1);
      }, 800 + Math.random() * 500); // Random delay between 0.8s ~ 1.3s
      return () => clearTimeout(timeout);
    } else {
      setTimeout(onComplete, 500);
    }
  }, [currentStep]);

  return (
    <div className="min-h-screen flex flex-col items-center justify-center bg-background">
      <div className="w-64 space-y-4">
        <div className="mx-auto w-16 h-16 border-4 border-primary border-t-transparent rounded-full animate-spin" />
        <div className="text-center font-medium text-lg animate-pulse">
          {currentStep < steps.length ? steps[currentStep] : "완료!"}
        </div>
        <div className="w-full bg-gray-200 rounded-full h-2 overflow-hidden">
          <div
            className="bg-primary h-full transition-all duration-500 ease-out"
            style={{ width: `${Math.min((currentStep / steps.length) * 100, 100)}%` }}
          />
        </div>
        <p className="text-xs text-center text-muted-foreground">
          공인인증서 보안 모듈이 작동 중입니다.
        </p>
      </div>
    </div>
  );
};

// 6. Onboarding with Company Type Selector
const Onboarding = ({ onStart }: { onStart: (name: string, company: string, bizNum: string, type: string) => void }) => {
  const [step, setStep] = useState<"type" | "info">("type");
  const [selectedType, setSelectedType] = useState<string>("general");

  const [name, setName] = useState("");
  const [company, setCompany] = useState("");
  const [bizNum, setBizNum] = useState("");
  const [isScraping, setIsScraping] = useState(false);

  const types = [
    { id: "startup", label: "스타트업 / R&D", icon: Rocket, desc: "투자 유치, 런웨이 관리, R&D 세액공제" },
    { id: "hospital", label: "병의원 / 약국", icon: Stethoscope, desc: "보험 청구, 비급여 관리, 의약품 재고" },
    { id: "commerce", label: "쇼핑몰 / 커머스", icon: ShoppingBag, desc: "ROAS 분석, 재고 회전, 정산 관리" },
    { id: "general", label: "일반 법인/개인", icon: Building2, desc: "표준 재무/세무/인사 관리" },
  ];

  const handleTypeSelect = (id: string) => {
    setSelectedType(id);
    setStep("info");
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (name && company && bizNum) {
      setIsScraping(true);
    }
  };

  if (isScraping) {
    return <LoadingScraper onComplete={() => onStart(name, company, bizNum, selectedType)} />;
  }

  if (step === "type") {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-50 p-4">
        <div className="max-w-4xl w-full">
          <div className="text-center mb-10">
            <h1 className="text-3xl font-bold text-gray-900 mb-2">어떤 비즈니스를 운영 중이신가요?</h1>
            <p className="text-gray-500">업종에 딱 맞는 'AI CFO'를 배정해 드립니다.</p>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            {types.map((t) => (
              <button
                key={t.id}
                onClick={() => handleTypeSelect(t.id)}
                className="flex flex-col items-center p-6 bg-white rounded-xl border-2 border-transparent hover:border-primary hover:shadow-lg transition-all group text-center h-64 justify-center"
              >
                <div className="w-16 h-16 bg-gray-100 rounded-full flex items-center justify-center mb-4 group-hover:bg-primary/10 group-hover:text-primary transition-colors">
                  <t.icon className="w-8 h-8 text-gray-600 group-hover:text-primary" />
                </div>
                <h3 className="font-bold text-lg mb-2 text-gray-800">{t.label}</h3>
                <p className="text-sm text-gray-400 leading-relaxed">{t.desc}</p>
              </button>
            ))}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50">
      <div className="max-w-md w-full p-8 bg-white rounded-2xl shadow-lg border">
        <div className="text-center mb-8">
          <button onClick={() => setStep("type")} className="text-sm text-gray-400 hover:text-gray-600 mb-4">← 유형 다시 선택하기</button>
          <div className="w-12 h-12 bg-primary rounded-xl flex items-center justify-center text-white text-xl font-bold mx-auto mb-4">
            {selectedType === 'startup' ? <Rocket className="w-6 h-6" /> : selectedType === 'hospital' ? <Stethoscope className="w-6 h-6" /> : selectedType === 'commerce' ? <ShoppingBag className="w-6 h-6" /> : <Building2 className="w-6 h-6" />}
          </div>
          <h1 className="text-2xl font-bold text-gray-900">기본 정보 입력</h1>
          <p className="text-gray-500 mt-2">맞춤형 대시보드를 생성합니다.</p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-5">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">대표자 성함</label>
            <input
              type="text"
              required
              className="w-full px-4 py-3 rounded-lg border focus:ring-2 focus:ring-primary/20 outline-none transition-all"
              placeholder="예: 김세무"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">회사명</label>
            <input
              type="text"
              required
              className="w-full px-4 py-3 rounded-lg border focus:ring-2 focus:ring-primary/20 outline-none transition-all"
              placeholder="예: (주)TaxAI"
              value={company}
              onChange={(e) => setCompany(e.target.value)}
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">사업자등록번호</label>
            <input
              type="text"
              required
              maxLength={12}
              className="w-full px-4 py-3 rounded-lg border focus:ring-2 focus:ring-primary/20 outline-none transition-all font-mono tracking-widest"
              placeholder="000-00-00000"
              value={bizNum}
              onChange={(e) => setBizNum(e.target.value)}
            />
          </div>

          <button
            type="submit"
            className="w-full py-3 bg-primary text-white rounded-lg font-bold text-lg hover:bg-primary/90 transition-transform active:scale-95 shadow-md flex items-center justify-center gap-2"
          >
            OS 설정 완료 <ChevronRight className="w-5 h-5" />
          </button>
        </form>
      </div>
    </div>
  );
};

// --- Main App ---

export default function App() {
  const [activeTab, setActiveTab] = useState<"dashboard" | "chat" | "competitions">("dashboard");
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);

  // User State (Auth)
  const [user, setUser] = useState<{ name: string, company: string, bizNum: string, type: string } | null>(null);

  // Data States
  const [dashboardData, setDashboardData] = useState<any>(null);
  const [recommendations, setRecommendations] = useState<Recommendation[]>([]);
  const [competitions, setCompetitions] = useState<Competition[]>([]);

  // Advanced SaaS States
  const [risk, setRisk] = useState<RiskAnalysis | null>(null);
  const [calendarAlerts, setCalendarAlerts] = useState<CalendarAlert[]>([]);

  // Chat States
  const [messages, setMessages] = useState<ChatMessage[]>([
    { role: "assistant", content: "안녕하세요! CEO님의 전담 AI CFO입니다. 무엇을 도와드릴까요?" },
  ]);
  const [input, setInput] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  // Initial Fetch
  useEffect(() => {
    if (user) {
      // Pass bizNum as seed and type for domain specific logic
      api.getDashboard(user.bizNum, user.type).then(setDashboardData).catch(console.error);
      api.getRecommendations().then(setRecommendations).catch(console.error);
      api.getCompetitions().then(setCompetitions).catch(console.error);
      // Advanced
      api.getTaxRisk(user.bizNum, user.type).then(setRisk).catch(console.error);
      api.getCalendarAlerts().then(res => setCalendarAlerts(res.alerts)).catch(console.error);
    }
  }, [user]);

  const handleSend = async () => {
    if (!input.trim()) return;
    const userMsg: ChatMessage = { role: "user", content: input };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");
    setIsLoading(true);

    try {
      const res = await api.chat(userMsg.content, messages);
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: res.response, context: res.context },
      ]);
    } catch (error) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: "죄송합니다. 오류가 발생했습니다." },
      ]);
    } finally {
      setIsLoading(false);
    }
  };

  // If no user, show Onboarding
  if (!user) {
    return <Onboarding onStart={(name, company, bizNum, type) => setUser({ name, company, bizNum, type })} />;
  }

  return (
    <div className="min-h-screen bg-background flex text-foreground font-sans">
      {/* Sidebar */}
      <aside
        className={`${isSidebarOpen ? "w-64" : "w-0"
          } bg-sidebar border-r border-sidebar-border transition-all duration-300 overflow-hidden flex flex-col fixed h-full z-10 md:relative`}
      >
        <div className="p-6 border-b border-sidebar-border">
          <div className="flex items-center gap-2 font-bold text-xl text-sidebar-primary">
            <div className="w-8 h-8 bg-sidebar-primary rounded-lg flex items-center justify-center text-white">
              T
            </div>
            TaxAI OS
          </div>
          <div className="mt-2 text-xs font-medium text-gray-500 bg-gray-100 px-2 py-1 rounded inline-block">
            {user.type === 'startup' ? 'Startup Ed.' : user.type === 'hospital' ? 'Medi-Tech Ed.' : user.type === 'commerce' ? 'Commerce Ed.' : 'Standard Ed.'}
          </div>
        </div>

        <nav className="flex-1 p-4 space-y-6 overflow-y-auto">
          {/* Section 1: CFO Core */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 px-3 tracking-wider">CFO CORE</div>
            <div className="space-y-1">
              <SidebarItem icon={LayoutDashboard} label="재무 대시보드" active={activeTab === "dashboard"} onClick={() => setActiveTab("dashboard")} />
              <SidebarItem icon={AlertTriangle} label="세무 리스크" onClick={() => { }} />
              <SidebarItem icon={Calculator} label="회계/증빙 관리" onClick={() => { }} />
            </div>
          </div>

          {/* Section 2: Domain MCPs */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 px-3 tracking-wider flex justify-between items-center">
              <span>DOMAIN EXTENSIONS</span>
              <span className="text-[10px] bg-primary/10 text-primary px-1.5 py-0.5 rounded">ON</span>
            </div>
            <div className="space-y-1">
              {user.type === 'startup' && (
                <>
                  <SidebarItem icon={Rocket} label="R&D / 정부지원" active={activeTab === "competitions"} onClick={() => setActiveTab("competitions")} />
                  <SidebarItem icon={TrendingUp} label="Runway / Burn Rate" onClick={() => { }} />
                </>
              )}
              {user.type === 'hospital' && (
                <>
                  <SidebarItem icon={Stethoscope} label="보험 청구 심사" onClick={() => { }} />
                  <SidebarItem icon={Activity} label="진료과별 손익" onClick={() => { }} />
                </>
              )}
              {user.type === 'commerce' && (
                <>
                  <SidebarItem icon={ShoppingBag} label="ROAS / 마케팅" onClick={() => { }} />
                  <SidebarItem icon={Building2} label="재고 / 정산" onClick={() => { }} />
                </>
              )}
              <SidebarItem icon={MessageSquare} label="AI 자문 (Domain Specific)" active={activeTab === "chat"} onClick={() => setActiveTab("chat")} />
            </div>
          </div>
        </nav>

        <div className="p-4 border-t border-sidebar-border">
          <div className="flex items-center gap-3 px-3 py-2">
            <div className="w-8 h-8 rounded-full bg-sidebar-accent flex items-center justify-center">
              <User className="w-4 h-4" />
            </div>
            <div className="text-sm">
              <div className="font-medium">{user.name} 대표</div>
              <div className="text-xs text-muted-foreground">{user.company}</div>
            </div>
          </div>
        </div>
      </aside>

      {/* Main Content */}
      <main className="flex-1 flex flex-col min-w-0 bg-background/50">
        {/* Header */}
        <header className="h-16 border-b bg-background/80 backdrop-blur-sm sticky top-0 z-10 px-6 flex items-center justify-between">
          <button onClick={() => setIsSidebarOpen(!isSidebarOpen)} className="p-2 hover:bg-accent rounded-md md:hidden"><Menu className="w-5 h-5" /></button>
          <h1 className="font-semibold text-lg opacity-80 pl-4 md:pl-0">
            {activeTab === 'dashboard' ? `${user.type.toUpperCase()} Dashboard` : activeTab === 'chat' ? 'AI CFO Chat' : 'Domain Intelligence'}
          </h1>
          <div className="flex items-center gap-4">
            <button className="p-2 hover:bg-accent rounded-full relative">
              <Bell className="w-5 h-5 text-muted-foreground" />
              {calendarAlerts.length > 0 && <span className="absolute top-2 right-2 w-2 h-2 bg-red-500 rounded-full" />}
            </button>
          </div>
        </header>

        <div className="flex-1 overflow-auto p-6">

          {/* DASHBOARD VIEW */}
          {activeTab === "dashboard" && dashboardData && (
            <div className="max-w-6xl mx-auto space-y-6">

              <div className="md:hidden mb-4">
                <h2 className="text-2xl font-bold">환영합니다, {user.name} 대표님! 👋</h2>
                <p className="text-sm text-muted-foreground">{user.company}의 재무 현황입니다.</p>
              </div>

              {/* 1. Risk Warning Card */}
              {risk && <RiskCard risk={risk} />}

              {/* 2. KPI Grid */}
              <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                {dashboardData.kpi?.map((k: any, i: number) => (
                  <KPICard key={i} {...k} />
                ))}
              </div>

              {/* 3. Main Actionable Area */}
              <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 h-auto lg:h-[420px]">
                {/* Simulator */}
                <div className="lg:col-span-1 h-full">
                  <TaxSimulator />
                </div>

                {/* Chart (Existing) */}
                <div className="lg:col-span-1 p-6 bg-card rounded-xl border shadow-sm flex flex-col h-full">
                  <h3 className="font-semibold mb-4 text-sm text-gray-500 uppercase tracking-widest">Financial Trend</h3>
                  <div className="flex-1 w-full min-h-0">
                    <ResponsiveContainer width="100%" height="100%">
                      <LineChart data={dashboardData.chart}>
                        <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                        <XAxis dataKey="name" axisLine={false} tickLine={false} tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }} dy={10} />
                        <YAxis axisLine={false} tickLine={false} tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }} />
                        <Tooltip />
                        <Line type="monotone" dataKey="income" stroke="hsl(var(--primary))" strokeWidth={2} dot={false} name="수입" />
                        <Line type="monotone" dataKey="expense" stroke="#ef4444" strokeWidth={2} dot={false} name="지출" />
                      </LineChart>
                    </ResponsiveContainer>
                  </div>
                </div>

                {/* Calendar */}
                <div className="lg:col-span-1 h-full">
                  <TaxCalendar alerts={calendarAlerts} />
                </div>
              </div>

              {/* 4. Prediction Section */}
              <div className="p-6 bg-card rounded-xl border shadow-sm">
                <h3 className="font-semibold text-lg flex items-center gap-2 mb-4">
                  <Calendar className="w-5 h-5" />
                  2026년 지원사업 예측 (AI)
                </h3>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  {recommendations.map((rec, i) => (
                    <PredictionCard key={i} rec={rec} />
                  ))}
                </div>
              </div>

              {/* 4. Domain Specific Widgets */}
              {user.type === 'startup' && (
                <div className="p-6 bg-blue-50/50 rounded-xl border border-blue-100 mt-6">
                  <h3 className="font-bold flex items-center gap-2 mb-4 text-blue-800">
                    <Rocket className="w-5 h-5" /> Startup Growth Engine
                  </h3>
                  <div className="grid grid-cols-2 gap-4">
                    <div className="bg-white p-4 rounded-lg border shadow-sm">
                      <div className="text-xs text-gray-500 mb-1">다음 투자 라운드까지</div>
                      <div className="text-xl font-bold text-gray-900">5.2개월 (Runway)</div>
                    </div>
                    <div className="bg-white p-4 rounded-lg border shadow-sm">
                      <div className="text-xs text-gray-500 mb-1">정부지원금 매칭 가능성</div>
                      <div className="text-xl font-bold text-gray-900">85% (High)</div>
                    </div>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* CHAT VIEW */}
          {activeTab === "chat" && (
            <div className="max-w-4xl mx-auto h-[calc(100vh-8rem)] flex flex-col bg-card rounded-xl border shadow-sm overflow-hidden">
              <div className="p-4 border-b bg-background/50 backdrop-blur">
                <h2 className="font-semibold flex items-center gap-2">
                  <MessageSquare className="w-5 h-5 text-primary" />
                  AI {user.type === 'hospital' ? '병원경영' : user.type === 'startup' ? '스타트업' : '세무'} 어드바이저
                </h2>
              </div>

              <div className="flex-1 overflow-y-auto p-4 space-y-4">
                {messages.map((m, i) => (
                  <div
                    key={i}
                    className={`flex ${m.role === "user" ? "justify-end" : "justify-start"
                      }`}
                  >
                    <div
                      className={`max-w-[80%] rounded-2xl px-4 py-3 ${m.role === "user"
                        ? "bg-primary text-primary-foreground"
                        : "bg-muted"
                        }`}
                    >
                      <p className="whitespace-pre-wrap text-sm leading-relaxed">{m.content}</p>

                      {/* Source Context Display */}
                      {m.context && m.context.length > 0 && (
                        <div className="mt-3 pt-3 border-t border-black/10 text-xs space-y-1">
                          <p className="font-semibold opacity-70">참고 자료 (RAG):</p>
                          {m.context.map((ctx, idx) => (
                            <div key={idx} className="bg-black/5 p-2 rounded">
                              <span className="font-bold">[{ctx.source}] </span>
                              {ctx.content.slice(0, 80)}...
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                ))}
                {isLoading && (
                  <div className="flex justify-start">
                    <div className="bg-secondary rounded-2xl px-4 py-3 flex items-center gap-2">
                      <Loader2 className="w-4 h-4 animate-spin" />
                      생각하는 중...
                    </div>
                  </div>
                )}
              </div>

              <div className="p-4 border-t bg-background">
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    handleSend();
                  }}
                  className="flex gap-2"
                >
                  <input
                    className="flex-1 bg-muted/50 border-0 focus:ring-2 ring-primary/20 rounded-lg px-4 py-3 outline-none transition-all"
                    placeholder="예: 법인세율이 어떻게 되나요? 2026년 예창패 언제 뜰까요?"
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                  />
                  <button
                    type="submit"
                    disabled={isLoading || !input.trim()}
                    className="bg-primary text-primary-foreground px-4 rounded-lg hover:bg-primary/90 disabled:opacity-50 transition-colors"
                  >
                    <Send className="w-5 h-5" />
                  </button>
                </form>
              </div>
            </div>
          )}

          {/* COMPETITIONS VIEW */}
          {activeTab === "competitions" && (
            <div className="max-w-6xl mx-auto space-y-6">
              <div>
                <h1 className="text-2xl font-bold mb-2">실전 대회 추천 🏆</h1>
                <p className="text-muted-foreground">Kaggle, Dacon 등에서 현재 진행 중인 금융/예측 대회를 실시간으로 추천해드립니다.</p>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {competitions.map((comp, i) => (
                  <CompetitionCard key={i} comp={comp} />
                ))}
              </div>
            </div>
          )}

        </div>
      </main>
    </div>
  );
}
