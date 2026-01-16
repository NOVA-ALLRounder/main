import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import {
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
  PieChart,
  Pie,
  Cell,
} from "recharts";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
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
  Check,
  LogOut,
  Building,
  FileText,
  X,
  Coins
} from "lucide-react";
import {
  api,
  type ChatMessage,
  type Recommendation,
  type Competition,
  type RiskAnalysis,
  type CalendarAlert,
  type MemoryRecord,
  type GraphNode,
  type GraphEdge
} from "./services/api";
import { LoginPage } from "./components/LoginPage";

// --- Components ---

type DecisionRecord = {
  id: string;
  title: string;
  summary: string;
  status: "accepted" | "rejected";
  createdAt: string;
  reasons: string[];
  impact: string;
  outcomeStatus: "pending" | "positive" | "negative" | "neutral";
  outcomeMemo?: string;
  priorityScore?: number;
  runwayMonths?: number;
  riskScore?: number;
  riskLevel?: "safe" | "warning" | "critical";
  drivers?: string[] | { label: string; score: number }[];
  rejectionReason?: string;
  relatedTab?: string;
  actionKey?: string;
};

type PresetDataset = {
  id: string;
  industry: "startup" | "hospital" | "commerce" | "saas" | "manufacturing" | "education" | "franchise";
  name: string;
  persona: string;
  summary: string;
  badge?: string;
  meta: {
    cash: number;
    monthlyRevenue: number;
    monthlyExpense: number;
    breakdown?: Record<string, number>;
  };
  history: { month: string; revenue: number; expense: number }[];
};

const buildPresetHistory = (
  baseRevenue: number,
  baseExpense: number,
  trend: number,
  seasonality: number
) => {
  const now = new Date();
  return Array.from({ length: 12 }, (_, idx) => {
    const offset = 11 - idx;
    const date = new Date(now.getFullYear(), now.getMonth() - offset, 1);
    const month = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
    const seasonal = Math.sin((idx / 12) * Math.PI * 2) * seasonality;
    const trendFactor = 1 + trend * (idx / 11);
    const revenue = Math.max(0, Math.round(baseRevenue * trendFactor * (1 + seasonal)));
    const expense = Math.max(0, Math.round(baseExpense * (1 + seasonal * 0.6) * (1 + trend * 0.4 * (idx / 11))));
    return { month, revenue, expense };
  });
};

const PRESET_DATASETS: PresetDataset[] = [
  { id: "startup-funded", industry: "startup", name: "Startup/Series A 확장", persona: "Series A 확장 단계", summary: "공격적 채용/마케팅 집행", badge: "공격", meta: { cash: 1500000000, monthlyRevenue: 70000000, monthlyExpense: 170000000, breakdown: { 인건비: 80000000, 마케팅: 35000000, 서버: 12000000, RnD: 25000000, 기타: 18000000 } }, history: buildPresetHistory(70000000, 170000000, 0.3, 0.1) },
  { id: "startup-pre", industry: "startup", name: "Startup/예비창업자", persona: "예비창업자", summary: "보수적 운영, 비용 최소화", badge: "보수", meta: { cash: 30000000, monthlyRevenue: 1500000, monthlyExpense: 4500000, breakdown: { 인건비: 1500000, 개발: 1200000, 운영비: 800000, 기타: 1000000 } }, history: buildPresetHistory(1500000, 4500000, 0.05, 0.03) },
  { id: "startup-self", industry: "startup", name: "Startup/자기자본 1억", persona: "자기자본 1억 스타트업", summary: "현실적 성장, 유료 전환 단계", badge: "현실", meta: { cash: 100000000, monthlyRevenue: 12000000, monthlyExpense: 18000000, breakdown: { 인건비: 8000000, 마케팅: 3000000, 서버: 2000000, 기타: 5000000 } }, history: buildPresetHistory(12000000, 18000000, 0.12, 0.06) },
  { id: "hospital-clinic", industry: "hospital", name: "Hospital/동네의원 안정", persona: "동네의원", summary: "현실적 운영, 보험청구 안정", badge: "현실", meta: { cash: 260000000, monthlyRevenue: 110000000, monthlyExpense: 85000000, breakdown: { 인건비: 36000000, 약품: 16000000, 임차료: 12000000, 장비: 9000000, 기타: 12000000 } }, history: buildPresetHistory(110000000, 85000000, 0.06, 0.04) },
  { id: "hospital-dental", industry: "hospital", name: "Hospital/치과 성장", persona: "치과 성장 단계", summary: "공격적 확장, 비급여 강화", badge: "공격", meta: { cash: 220000000, monthlyRevenue: 150000000, monthlyExpense: 130000000, breakdown: { 인건비: 48000000, 재료비: 22000000, 임차료: 13000000, 마케팅: 18000000, 기타: 29000000 } }, history: buildPresetHistory(150000000, 130000000, 0.12, 0.05) },
  { id: "hospital-delay", industry: "hospital", name: "Hospital/요양급여 지연", persona: "요양급여 지연 리스크", summary: "보수적 운영, 현금흐름 압박", badge: "보수", meta: { cash: 180000000, monthlyRevenue: 70000000, monthlyExpense: 90000000, breakdown: { 인건비: 38000000, 약품: 15000000, 임차료: 10000000, 운영비: 14000000, 기타: 13000000 } }, history: buildPresetHistory(70000000, 90000000, 0.03, 0.05) },
  { id: "commerce-marketplace", industry: "commerce", name: "E-commerce/마켓플레이스 셀러", persona: "오픈마켓 셀러", summary: "보수적 운영, 낮은 마진", badge: "보수", meta: { cash: 100000000, monthlyRevenue: 140000000, monthlyExpense: 135000000, breakdown: { 매입: 65000000, 광고: 22000000, 물류: 20000000, 인건비: 13000000, 기타: 15000000 } }, history: buildPresetHistory(140000000, 135000000, 0.04, 0.06) },
  { id: "commerce-d2c", industry: "commerce", name: "E-commerce/D2C 성장", persona: "D2C 브랜드 성장", summary: "공격적 성장, 광고 확대", badge: "공격", meta: { cash: 320000000, monthlyRevenue: 280000000, monthlyExpense: 240000000, breakdown: { 매입: 105000000, 광고: 55000000, 물류: 30000000, 인건비: 22000000, 기타: 28000000 } }, history: buildPresetHistory(280000000, 240000000, 0.14, 0.07) },
  { id: "commerce-seasonal", industry: "commerce", name: "E-commerce/시즌 편차", persona: "시즌 편차 큰 쇼핑몰", summary: "현실적 운영, 시즌 변동 큼", badge: "현실", meta: { cash: 120000000, monthlyRevenue: 90000000, monthlyExpense: 100000000, breakdown: { 매입: 45000000, 광고: 20000000, 물류: 15000000, 인건비: 10000000, 기타: 10000000 } }, history: buildPresetHistory(90000000, 100000000, 0.06, 0.12) },
  { id: "saas-growth", industry: "saas", name: "SaaS/ARR 성장", persona: "PLG 성장 SaaS", summary: "공격적 성장, 인프라 확장", badge: "공격", meta: { cash: 2400000000, monthlyRevenue: 150000000, monthlyExpense: 260000000, breakdown: { 인건비: 120000000, 마케팅: 50000000, 인프라: 25000000, 고객성공: 20000000, 기타: 45000000 } }, history: buildPresetHistory(150000000, 260000000, 0.22, 0.04) },
  { id: "saas-stable", industry: "saas", name: "SaaS/중견 안정", persona: "중견 SaaS", summary: "현실적 운영, 수익 안정", badge: "현실", meta: { cash: 800000000, monthlyRevenue: 120000000, monthlyExpense: 110000000, breakdown: { 인건비: 55000000, 마케팅: 15000000, 인프라: 12000000, 고객성공: 12000000, 기타: 16000000 } }, history: buildPresetHistory(120000000, 110000000, 0.1, 0.03) },
  { id: "saas-bootstrap", industry: "saas", name: "SaaS/부트스트랩", persona: "부트스트랩 SaaS", summary: "보수적 운영, 비용 절감", badge: "보수", meta: { cash: 120000000, monthlyRevenue: 35000000, monthlyExpense: 45000000, breakdown: { 인건비: 22000000, 인프라: 6000000, 마케팅: 5000000, 운영: 5000000, 기타: 7000000 } }, history: buildPresetHistory(35000000, 45000000, 0.08, 0.03) },
  { id: "manufacturing-capex", industry: "manufacturing", name: "제조/설비투자", persona: "설비투자 확장", summary: "공격적 투자, 고정비 증가", badge: "공격", meta: { cash: 1200000000, monthlyRevenue: 500000000, monthlyExpense: 520000000, breakdown: { 원재료: 250000000, 인건비: 90000000, 설비리스: 70000000, 물류: 40000000, 기타: 70000000 } }, history: buildPresetHistory(500000000, 520000000, 0.15, 0.06) },
  { id: "manufacturing-stable", industry: "manufacturing", name: "제조/수주 안정", persona: "수주 안정 제조업", summary: "현실적 운영, 마진 안정", badge: "현실", meta: { cash: 900000000, monthlyRevenue: 450000000, monthlyExpense: 400000000, breakdown: { 원재료: 210000000, 인건비: 80000000, 설비유지: 35000000, 물류: 35000000, 기타: 40000000 } }, history: buildPresetHistory(450000000, 400000000, 0.08, 0.04) },
  { id: "manufacturing-cost", industry: "manufacturing", name: "제조/원가 압박", persona: "원가 압박 제조업", summary: "보수적 운영, 원가 상승", badge: "보수", meta: { cash: 600000000, monthlyRevenue: 320000000, monthlyExpense: 340000000, breakdown: { 원재료: 190000000, 인건비: 70000000, 설비유지: 25000000, 물류: 25000000, 기타: 30000000 } }, history: buildPresetHistory(320000000, 340000000, 0.03, 0.05) },
  { id: "education-online", industry: "education", name: "교육/온라인 확장", persona: "온라인 교육 성장", summary: "공격적 성장, 마케팅 확대", badge: "공격", meta: { cash: 450000000, monthlyRevenue: 180000000, monthlyExpense: 210000000, breakdown: { 콘텐츠: 70000000, 마케팅: 50000000, 인건비: 40000000, 플랫폼: 20000000, 기타: 30000000 } }, history: buildPresetHistory(180000000, 210000000, 0.18, 0.07) },
  { id: "education-offline", industry: "education", name: "교육/오프라인 학원", persona: "오프라인 학원", summary: "현실적 운영, 안정적 수강", badge: "현실", meta: { cash: 250000000, monthlyRevenue: 120000000, monthlyExpense: 105000000, breakdown: { 인건비: 45000000, 임차료: 20000000, 마케팅: 15000000, 운영비: 10000000, 기타: 15000000 } }, history: buildPresetHistory(120000000, 105000000, 0.06, 0.09) },
  { id: "education-small", industry: "education", name: "교육/소형 학원", persona: "소형 학원", summary: "보수적 운영, 비용 절감", badge: "보수", meta: { cash: 80000000, monthlyRevenue: 40000000, monthlyExpense: 48000000, breakdown: { 인건비: 20000000, 임차료: 10000000, 마케팅: 5000000, 운영비: 5000000, 기타: 8000000 } }, history: buildPresetHistory(40000000, 48000000, 0.04, 0.1) },
  { id: "franchise-growth", industry: "franchise", name: "프랜차이즈/신규점포 확장", persona: "다점포 확장", summary: "공격적 확장, 고정비 증가", badge: "공격", meta: { cash: 700000000, monthlyRevenue: 260000000, monthlyExpense: 300000000, breakdown: { 원재료: 120000000, 인건비: 70000000, 임차료: 40000000, 마케팅: 30000000, 기타: 40000000 } }, history: buildPresetHistory(260000000, 300000000, 0.12, 0.08) },
  { id: "franchise-core", industry: "franchise", name: "프랜차이즈/1~2호점", persona: "초기 프랜차이즈", summary: "현실적 운영, 매출 안정", badge: "현실", meta: { cash: 300000000, monthlyRevenue: 150000000, monthlyExpense: 140000000, breakdown: { 원재료: 65000000, 인건비: 35000000, 임차료: 20000000, 마케팅: 10000000, 기타: 10000000 } }, history: buildPresetHistory(150000000, 140000000, 0.07, 0.07) },
  { id: "franchise-stagnant", industry: "franchise", name: "프랜차이즈/매출 정체", persona: "매출 정체 점포", summary: "보수적 운영, 매출 정체", badge: "보수", meta: { cash: 200000000, monthlyRevenue: 110000000, monthlyExpense: 125000000, breakdown: { 원재료: 55000000, 인건비: 30000000, 임차료: 20000000, 마케팅: 8000000, 기타: 12000000 } }, history: buildPresetHistory(110000000, 125000000, 0.02, 0.06) },
];

const PRESET_GROUPS = [
  { id: "startup", label: "스타트업", desc: "투자/예비/자기자본", items: PRESET_DATASETS.filter((item) => item.industry === "startup") },
  { id: "hospital", label: "병의원", desc: "의원/치과/요양급여", items: PRESET_DATASETS.filter((item) => item.industry === "hospital") },
  { id: "commerce", label: "이커머스", desc: "마켓플레이스/D2C/시즌", items: PRESET_DATASETS.filter((item) => item.industry === "commerce") },
  { id: "saas", label: "IT SaaS", desc: "PLG/중견/부트스트랩", items: PRESET_DATASETS.filter((item) => item.industry === "saas") },
  { id: "manufacturing", label: "제조", desc: "설비투자/수주안정/원가압박", items: PRESET_DATASETS.filter((item) => item.industry === "manufacturing") },
  { id: "education", label: "교육", desc: "온라인/오프라인/소형", items: PRESET_DATASETS.filter((item) => item.industry === "education") },
  { id: "franchise", label: "프랜차이즈", desc: "확장/초기/정체", items: PRESET_DATASETS.filter((item) => item.industry === "franchise") },
];

const DashboardEmptyState = ({ status, onSetup }: { status: "none" | "partial"; onSetup: () => void }) => {
  const title = status === "partial" ? "재무 데이터가 아직 부족합니다." : "재무 데이터가 아직 없습니다.";
  const desc = status === "partial" ? "현금, 매출, 지출 중 2개 이상을 입력하면 대시보드가 활성화됩니다." : "설정에서 프리셋 데이터셋을 적용하면 대시보드가 활성화됩니다.";
  return (
    <div className="max-w-6xl mx-auto space-y-6">
      <div className="p-6 rounded-xl border bg-muted/40 flex flex-col md:flex-row md:items-center md:justify-between gap-4">
        <div>
          <h3 className="text-base font-semibold">{title}</h3>
          <p className="text-sm text-muted-foreground mt-1">{desc}</p>
        </div>
        <button onClick={onSetup} className="text-sm px-4 py-2 rounded-full bg-primary text-primary-foreground hover:bg-primary/90 transition-colors">데이터 입력하러 가기</button>
      </div>
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        {[1, 2, 3, 4].map((i) => <div key={i} className="h-28 rounded-xl border bg-muted/30" />)}
      </div>
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2 h-60 rounded-xl border bg-muted/30" />
        <div className="lg:col-span-1 h-60 rounded-xl border bg-muted/30" />
      </div>
    </div>
  );
};

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

// --- 0. Revenue Calculator Wizard ---
const RevenueCalculator = ({ onComplete, onCancel }: { onComplete: (val: number) => void, onCancel: () => void }) => {
  const [step, setStep] = useState(1);
  const [model, setModel] = useState<"sales" | "subscription" | "service">("sales");

  // Inputs
  const [price, setPrice] = useState("");
  const [volume, setVolume] = useState(""); // units/mo or users or projects/yr

  // Result
  const [calculated, setCalculated] = useState(0);

  const handleCalculate = () => {
    const p = parseInt(price.replace(/,/g, "")) || 0;
    const v = parseInt(volume.replace(/,/g, "")) || 0;
    let total = 0;

    if (model === "sales") {
      // Price * Units/mo * 12
      total = p * v * 12;
    } else if (model === "subscription") {
      // Simple ARR estimate: (Price * Users at End) * 0.7 (avg active) * 12? 
      // Let's do: Price * Expected Users (Year End) * 6 (avg ramp up)
      total = p * v * 8; // Bit more optimistic
    } else {
      // Project Fee * Projects/yr
      total = p * v;
    }
    setCalculated(total);
    setStep(3);
  };

  if (step === 1) {
    return (
      <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 animate-in fade-in duration-200">
        <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6">
          <div className="flex justify-between items-center mb-6">
            <h3 className="text-xl font-bold">비즈니스 모델이 무엇인가요?</h3>
            <button onClick={onCancel}><XCircle className="w-6 h-6 text-gray-400 hover:text-gray-600" /></button>
          </div>
          <div className="space-y-3">
            {[
              { id: "sales", label: "판매형 (Sales)", desc: "제품/상품을 1회성으로 판매" },
              { id: "subscription", label: "구독형 (Subscription)", desc: "서비스를 월/년 단위 이용료로 제공" },
              { id: "service", label: "용역/프로젝트 (Service)", desc: "건별 계약으로 용역 or 개발 제공" },
            ].map(m => (
              <button
                key={m.id}
                onClick={() => { setModel(m.id as any); setStep(2); }}
                className="w-full p-4 rounded-lg border-2 border-gray-100 hover:border-primary hover:bg-primary/5 flex flex-col items-start transition-all"
              >
                <div className="font-bold text-gray-800">{m.label}</div>
                <div className="text-sm text-gray-500">{m.desc}</div>
              </button>
            ))}
          </div>
        </div>
      </div>
    );
  }

  if (step === 2) {
    return (
      <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 animate-in fade-in duration-200">
        <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6">
          <div className="flex justify-between items-center mb-6">
            <h3 className="text-xl font-bold">핵심 지표 입력</h3>
            <button onClick={() => setStep(1)} className="text-sm text-gray-500 hover:text-gray-800">뒤로</button>
          </div>

          <div className="space-y-5">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                {model === "sales" ? "평균 객단가 (1개당 판매가)" : model === "subscription" ? "월 구독료 (1인당)" : "평균 프로젝트/건당 계약금"}
              </label>
              <div className="relative">
                <input
                  type="text"
                  autoFocus
                  className="w-full pl-3 pr-8 py-2 border rounded-lg focus:ring-2 focus:ring-primary/20 outline-none text-right font-medium"
                  value={price}
                  onChange={e => setPrice(Number(e.target.value.replace(/[^0-9]/g, "")).toLocaleString())}
                  placeholder="0"
                />
                <span className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 text-sm">원</span>
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                {model === "sales" ? "월 예상 판매량 (개)" : model === "subscription" ? "연말 기준 예상 유료 구독자 수 (명)" : "연간 예상 수주 건수 (건)"}
              </label>
              <input
                type="text"
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-primary/20 outline-none text-right font-medium"
                value={volume}
                onChange={e => setVolume(Number(e.target.value.replace(/[^0-9]/g, "")).toLocaleString())}
                placeholder="0"
              />
            </div>

            <button
              onClick={handleCalculate}
              disabled={!price || !volume}
              className="w-full py-3 bg-primary text-white rounded-lg font-bold disabled:opacity-50 disabled:cursor-not-allowed"
            >
              예상 매출 계산하기
            </button>
          </div>
        </div>
      </div>
    );
  }

  // Step 3: Result
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 animate-in fade-in duration-200">
      <div className="bg-white rounded-xl shadow-xl w-full max-w-sm p-6 text-center">
        <h3 className="text-lg font-medium text-gray-600 mb-2">AI가 계산한 1년차 예상 매출</h3>
        <div className="text-4xl font-extrabold text-blue-600 mb-6">
          {calculated.toLocaleString()}원
        </div>

        <p className="text-sm text-gray-500 mb-6 bg-gray-50 p-3 rounded-lg">
          {model === "sales" ? "월 판매량 유지 시 연간 합계입니다."
            : model === "subscription" ? "램프업(성장) 기간을 고려하여 보정된 값입니다."
              : "건당 단가 × 연간 건수 합계입니다."}
        </p>

        <div className="flex gap-2">
          <button
            onClick={() => setStep(2)}
            className="flex-1 py-3 border rounded-lg font-medium hover:bg-gray-50 text-gray-600"
          >
            다시 계산
          </button>
          <button
            onClick={() => onComplete(calculated)}
            className="flex-1 py-3 bg-blue-600 text-white rounded-lg font-bold hover:bg-blue-700"
          >
            이 값으로 설정
          </button>
        </div>
      </div>
    </div>
  );
};



// 1. Risk Warning System (AI CFO Mode)
const RiskCard = ({ risk }: { risk: RiskAnalysis }) => {
  const isPlanningMode = risk.title.includes("설계"); // Detect Pre-Founder mode
  const [checkedItems, setCheckedItems] = useState<Set<number>>(new Set());
  const [expandedItem, setExpandedItem] = useState<number | null>(null); // For accordion view
  const [explainModal, setExplainModal] = useState<{ proof: any; index: number } | null>(null);
  const [explainText, setExplainText] = useState('');
  const [explainedItems, setExplainedItems] = useState<Set<number>>(new Set());

  const toggleCheck = (idx: number, e: React.MouseEvent) => {
    e.stopPropagation(); // Prevent toggling accordion when checking
    const next = new Set(checkedItems);
    if (next.has(idx)) {
      next.delete(idx);
    } else {
      next.add(idx);
      // Feedback for user
      const item = risk.action_items[idx];
      const saved = Math.round(risk.estimated_penalty / risk.action_items.length);
      alert(`'${item.task}' 항목을 확인했습니다.\n예상 리스크 감소액: ${saved.toLocaleString()}원`);
    }
    setCheckedItems(next);
  };

  // Calculate secured savings
  const totalRiskAmount = risk.estimated_penalty;
  const securedAmount = Math.round(totalRiskAmount * (checkedItems.size / risk.action_items.length));
  const remainingRisk = totalRiskAmount - securedAmount;

  const toggleExpand = (idx: number) => {
    setExpandedItem(expandedItem === idx ? null : idx);
  };

  const levelColor = isPlanningMode ? 'bg-blue-500' : risk.level === 'critical' ? 'bg-red-500' : risk.level === 'warning' ? 'bg-amber-500' : 'bg-emerald-500';
  const levelBg = isPlanningMode ? 'bg-blue-50 border-blue-100' : risk.level === 'critical' ? 'bg-red-50 border-red-100' : risk.level === 'warning' ? 'bg-amber-50 border-amber-100' : 'bg-emerald-50 border-emerald-100';

  return (
    <div className={`rounded-xl border ${levelBg} flex flex-col overflow-hidden`}>
      {/* 1. Top Section: Risk Score & Money Impact */}
      <div className="p-6 pb-2 flex flex-col gap-6">
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
                  {isPlanningMode ? '+' : '+'}{remainingRisk.toLocaleString()}원
                  {!isPlanningMode && <AlertTriangle className="w-6 h-6 animate-bounce" />}
                </div>
                {checkedItems.size > 0 && (
                  <div className="mt-2 text-sm font-medium text-emerald-600 bg-emerald-50 px-2 py-1 rounded">
                    🛡️ {securedAmount.toLocaleString()}원 방어 성공!
                  </div>
                )}
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
              <div
                key={i}
                onClick={() => toggleExpand(i)}
                className={`flex flex-col p-3 rounded-lg border transition-all cursor-pointer ${expandedItem === i ? 'bg-blue-50/30 ring-1 ring-blue-100' : 'hover:bg-gray-50'}`}
              >
                <div className="flex items-center justify-between w-full group-hover:pl-1 transition-all">
                  <div className="flex items-center gap-3">
                    <div
                      onClick={(e) => toggleCheck(i, e)}
                      className={`w-6 h-6 rounded border-2 flex items-center justify-center transition-all cursor-pointer shadow-sm hover:scale-110 ${checkedItems.has(i) ? 'bg-primary border-primary' : 'border-gray-300 bg-white group-hover:border-primary'}`}
                    >
                      {checkedItems.has(i) && <Check className="w-4 h-4 text-white" />}
                    </div>
                    <div>
                      <div className="text-base font-bold text-gray-800 flex items-center gap-2">
                        {item.task}
                        <ChevronRight className={`w-4 h-4 text-gray-400 transition-transform ${expandedItem === i ? 'rotate-90' : ''}`} />
                      </div>

                      {item.amount > 0 && !expandedItem && (
                        <div className="text-xs text-muted-foreground">
                          비용/효과: <span className="font-medium text-gray-700">{item.amount.toLocaleString()}원</span>
                        </div>
                      )}
                    </div>
                  </div>

                  {expandedItem !== i && (
                    <div className="flex gap-2">
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
                  )}
                </div>

                {/* Expanded Details */}
                {expandedItem === i && (
                  <div className="mt-3 ml-8 text-sm animate-in fade-in slide-in-from-top-1 duration-200">
                    <p className="text-gray-600 leading-relaxed mb-3">
                      {item.description?.split("**").map((part, idx) =>
                        idx % 2 === 1 ? <span key={idx} className="font-bold text-blue-800 bg-blue-50 px-1 rounded">{part}</span> : part
                      )}
                    </p>

                    {item.references && (
                      <div className="space-y-1">
                        <p className="text-xs text-muted-foreground font-semibold">관련 정보 및 근거:</p>
                        {item.references.map((ref, idx) => (
                          <a key={idx} href={ref.url} target="_blank" rel="noreferrer" className="flex items-center gap-1 text-xs text-blue-500 hover:underline hover:text-blue-700">
                            <ExternalLink className="w-3 h-3" /> {ref.title}
                          </a>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
          {risk.action_items.length === 0 && <p className="text-sm text-gray-400 text-center py-4">조치할 항목이 없습니다.</p>}

          {/* Reference Library (Collected) */}
          {risk.action_items.some(i => i.references?.length) && (
            <div className="mt-6 pt-4 border-t border-dashed">
              <h4 className="text-xs font-bold text-gray-400 uppercase tracking-wider mb-2">Reference Library</h4>
              <div className="flex flex-wrap gap-2">
                {Array.from(new Set(risk.action_items.flatMap(i => i.references || []).map(r => JSON.stringify(r)))).map((rStr, i) => {
                  const r = JSON.parse(rStr);
                  return (
                    <a key={i} href={r.url} target="_blank" rel="noreferrer" className="px-2 py-1 bg-gray-100 hover:bg-gray-200 text-xs text-gray-600 rounded flex items-center gap-1 transition-colors">
                      <ExternalLink className="w-3 h-3" /> {r.title}
                    </a>
                  );
                })}
              </div>
            </div>
          )}
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
                  {explainedItems.has(i) ? (
                    <span className="text-xs bg-green-100 text-green-600 px-2 py-1 rounded">✓ 소명완료</span>
                  ) : (
                    <button
                      onClick={() => { setExplainModal({ proof, index: i }); setExplainText(''); }}
                      className="text-xs border px-2 py-1 rounded hover:bg-gray-100"
                    >
                      소명하기
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 소명하기 모달 */}
      {explainModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 animate-in fade-in">
          <div className="bg-white rounded-xl p-6 w-full max-w-md mx-4 shadow-2xl animate-in zoom-in-95">
            <h3 className="font-bold text-lg mb-4 flex items-center gap-2">
              📝 증빙 소명하기
            </h3>
            <div className="bg-gray-50 p-3 rounded-lg mb-4 text-sm">
              <p><strong>거래처:</strong> {explainModal.proof.merchant}</p>
              <p><strong>금액:</strong> {explainModal.proof.amount.toLocaleString()}원</p>
              <p><strong>구분:</strong> {explainModal.proof.type}</p>
            </div>
            <div className="space-y-3">
              <div>
                <label className="block text-sm font-medium mb-1">소명 내용</label>
                <textarea
                  value={explainText}
                  onChange={(e) => setExplainText(e.target.value)}
                  className="w-full border rounded-lg p-2 text-sm h-24 resize-none"
                  placeholder="해당 거래에 대한 소명 내용을 입력해주세요..."
                />
              </div>
              <div>
                <label className="block text-sm font-medium mb-1">증빙자료 첨부</label>
                <div className="border-2 border-dashed rounded-lg p-4 text-center text-sm text-gray-500 hover:bg-gray-50 cursor-pointer">
                  <input type="file" className="hidden" id="proof-file" />
                  <label htmlFor="proof-file" className="cursor-pointer">
                    📎 파일 선택 또는 드래그
                  </label>
                </div>
              </div>
            </div>
            <div className="flex gap-2 mt-6">
              <button
                onClick={() => setExplainModal(null)}
                className="flex-1 py-2 border rounded-lg text-sm hover:bg-gray-50"
              >
                취소
              </button>
              <button
                onClick={() => {
                  setExplainedItems(prev => new Set([...prev, explainModal.index]));
                  setExplainModal(null);
                  alert('소명이 제출되었습니다.');
                }}
                className="flex-1 py-2 bg-primary text-white rounded-lg text-sm hover:bg-primary/90"
              >
                소명 제출
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

// 2. Tax Simulator with business-specific items (한국 세법 기준)
const TaxSimulator = ({ businessType = 'startup' }: { businessType?: string }) => {
  // 업종별 절세 항목 정의 (법령 근거 포함)
  const simulatorItems: Record<string, Array<{ key: string; label: string; desc: string; rate: number; legal: string }>> = {
    startup: [
      { key: 'salary_increase', label: '대표자 급여 인상', desc: '월 400~600만원 적정 급여 책정', rate: 0.05, legal: '소득세법 제20조' },
      { key: 'vehicle_expense', label: '업무용 승용차 비용', desc: '연 1,500만원 한도 (운행일지 필수)', rate: 0.03, legal: '법인세법 시행령 제50조' },
      { key: 'rnd_credit', label: 'R&D 세액공제', desc: '연구개발비 25% 공제 (중소기업)', rate: 0.25, legal: '조특법 제10조' },
      { key: 'startup_deduction', label: '창업중소기업 세액감면', desc: '5년간 50~100% 감면', rate: 0.50, legal: '조특법 제6조' },
      { key: 'employment_credit', label: '고용증대 세액공제', desc: '청년 1,100만원/일반 700만원', rate: 0.02, legal: '조특법 제29조의7' },
    ],
    hospital: [
      { key: 'equipment_depreciation', label: '의료장비 가속상각', desc: 'MRI/CT 등 내용연수 단축', rate: 0.08, legal: '법인세법 시행령 제26조' },
      { key: 'staff_training', label: '직원 교육훈련비 공제', desc: '인건비의 10% 세액공제', rate: 0.10, legal: '조특법 제7조' },
      { key: 'medical_consumables', label: '의약품/소모품 비용', desc: '매입세액 공제 적용', rate: 0.05, legal: '부가가치세법 제38조' },
      { key: 'building_maintenance', label: '시설 유지보수비', desc: '수익적 지출 비용처리', rate: 0.04, legal: '법인세법 제23조' },
      { key: 'insurance_optimization', label: '4대보험 최적화', desc: '적정 신고 및 지원금 활용', rate: 0.03, legal: '고용보험법' },
    ],
    commerce: [
      { key: 'inventory_valuation', label: '재고자산 평가방법 변경', desc: '선입선출법→후입선출법', rate: 0.04, legal: '법인세법 제42조' },
      { key: 'ad_expense', label: '광고선전비 비용처리', desc: '매출액의 일정비율 한도', rate: 0.06, legal: '법인세법 시행령 제45조' },
      { key: 'logistics_subsidy', label: '물류비 세액공제', desc: '스마트 물류센터 투자', rate: 0.03, legal: '조특법 제25조' },
      { key: 'platform_fee', label: '마켓플레이스 수수료', desc: '판매수수료 전액 비용처리', rate: 0.05, legal: '법인세법 제19조' },
      { key: 'export_credit', label: '수출 세액공제', desc: '해외 판매 시 환급', rate: 0.04, legal: '조특법 제22조' },
    ],
  };

  const items = simulatorItems[businessType] || simulatorItems.startup;
  const initialToggles = items.reduce((acc, item) => ({ ...acc, [item.key]: false }), {});

  const [toggles, setToggles] = useState<Record<string, boolean>>(initialToggles);
  const [result, setResult] = useState<any>({ total_saving: 0, details: [], message: '' });

  useEffect(() => {
    handleSimulate();
  }, [toggles]);

  const handleSimulate = async () => {
    try {
      const res = await api.simulateTax(toggles);
      setResult(res);
    } catch (e) {
      console.error(e);
      setResult({ total_saving: 0, details: [], message: 'API 오류' });
    }
  };

  const toggle = (key: string) => {
    setToggles(prev => ({ ...prev, [key]: !prev[key] }));
  };

  return (
    <div className="p-6 bg-card rounded-xl border shadow-sm h-full flex flex-col">
      <div className="flex justify-between items-center mb-4">
        <h3 className="font-bold flex items-center gap-2">
          <Calculator className="w-5 h-5 text-primary" />
          절세 시뮬레이터
        </h3>
        <span className="text-xs bg-sidebar-accent px-2 py-1 rounded">
          {businessType === 'hospital' ? '병원' : businessType === 'commerce' ? '커머스' : '스타트업'}
        </span>
      </div>

      <div className="space-y-3 mb-6 flex-1">
        {items.map((item) => {
          // Find dynamic detail from result if available
          const dynamicDetail = result?.details?.find((d: any) => d.key === item.key);

          // Let's use a separate state for expansion if we want independent control, 
          // but for now, let's show detail if checked (simulated)

          return (
            <div
              key={item.key}
              className={`rounded-lg border transition-all ${toggles[item.key] ? 'bg-primary/5 border-primary shadow-sm' : 'hover:bg-muted/50'}`}
            >
              <div
                onClick={() => toggle(item.key)}
                className="p-3 cursor-pointer flex items-center gap-3"
              >
                <div className={`w-5 h-5 rounded border flex items-center justify-center flex-shrink-0 transition-colors ${toggles[item.key] ? 'bg-primary border-primary text-white' : 'border-gray-400'}`}>
                  {toggles[item.key] && <CheckCircle className="w-3.5 h-3.5" />}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium">{item.label}</span>
                    <span className="text-[10px] bg-blue-50 text-blue-600 px-1.5 py-0.5 rounded hidden sm:inline">
                      {Math.round(item.rate * 100)}% 공제
                    </span>
                  </div>
                  <p className="text-xs text-muted-foreground">{item.desc}</p>
                </div>
                <ChevronRight className={`w-4 h-4 text-gray-400 transition-transform ${toggles[item.key] ? 'rotate-90' : ''}`} />
              </div>

              {/* Expanded Detail View */}
              {toggles[item.key] && (
                <div className="px-3 pb-3 pl-11">
                  <div className="bg-white/50 p-3 rounded text-xs text-gray-600 space-y-2 border border-blue-100">
                    <p className="font-semibold text-blue-700 mb-1">💡 절세 솔루션</p>
                    <p>{dynamicDetail?.description || "세액 공제 요건을 검토 중입니다..."}</p>

                    {dynamicDetail?.references && (
                      <div className="mt-2 pt-2 border-t border-gray-100">
                        <span className="text-[10px] text-gray-400 font-bold block mb-1">관련 법령/가이드</span>
                        <div className="flex flex-wrap gap-2">
                          {dynamicDetail.references.map((ref: any, idx: number) => (
                            <a
                              key={idx}
                              href={ref.url}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="flex items-center gap-1 bg-gray-50 px-2 py-1 rounded border hover:bg-gray-100 text-[10px] text-gray-500"
                              onClick={(e) => e.stopPropagation()}
                            >
                              <FileText className="w-3 h-3" />
                              {ref.title}
                            </a>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>

      <div className="bg-muted/30 p-4 rounded-lg border-t border-dashed border-gray-200">
        <p className="text-xs text-muted-foreground mb-1">예상 절세 효과</p>
        <div className="text-2xl font-bold text-primary">
          {(result?.total_saving ?? 0).toLocaleString()}원
          <span className="text-sm font-normal text-muted-foreground ml-1">절약</span>
        </div>
      </div>
    </div>
  );
};

// 3. Tax Calendar with click-to-detail
const TaxCalendar = ({ alerts }: { alerts: CalendarAlert[] }) => {
  const [selectedAlert, setSelectedAlert] = useState<CalendarAlert | null>(null);

  // 세무일정 상세 정보
  const alertDetails: Record<string, { desc: string; checklist: string[]; docs: string[] }> = {
    '2기 확정 부가세 신고': {
      desc: '2기(7-12월) 부가가치세 확정 신고 및 납부',
      checklist: ['매출/매입 세금계산서 정리', '신용카드 매출전표 확인', '현금영수증 발행내역 검토'],
      docs: ['부가가치세 신고서', '매출처별 세금계산서 합계표', '매입처별 세금계산서 합계표']
    },
    '1월분 원천세 신고/납부': {
      desc: '1월분 급여 관련 원천징수세액 신고 및 납부',
      checklist: ['급여대장 정리', '일용직 지급명세서 작성', '퇴직소득 원천징수 확인'],
      docs: ['원천징수이행상황신고서', '지급명세서']
    },
    '법인세 신고': {
      desc: '전년도 법인소득에 대한 법인세 신고 (12월 결산법인)',
      checklist: ['재무제표 확정', '세무조정 검토', '이연법인세 계산'],
      docs: ['법인세 과세표준 및 세액신고서', '재무상태표', '손익계산서']
    },
    '종합소득세 신고': {
      desc: '전년도 종합소득에 대한 소득세 신고',
      checklist: ['소득금액 합산', '필요경비 정리', '세액공제 항목 확인'],
      docs: ['종합소득세 신고서', '소득공제 증빙서류']
    }
  };

  return (
    <div className="p-6 bg-card rounded-xl border shadow-sm h-full flex flex-col">
      <h3 className="font-bold flex items-center gap-2 mb-4">
        <Calendar className="w-5 h-5 text-primary" />
        세무 일정 (30일 이내)
      </h3>

      {selectedAlert ? (
        // 상세보기 모드
        <div className="flex-1 animate-in fade-in">
          <button
            onClick={() => setSelectedAlert(null)}
            className="text-xs text-primary hover:underline mb-3 flex items-center gap-1"
          >
            ← 목록으로
          </button>
          <div className="bg-primary/5 p-4 rounded-lg border border-primary/20">
            <h4 className="font-bold text-lg mb-2">{selectedAlert.title}</h4>
            <p className="text-sm text-muted-foreground mb-3">
              {alertDetails[selectedAlert.title]?.desc || '세무 일정 상세 정보'}
            </p>
            <div className="text-xs space-y-2">
              <p><strong>📅 마감:</strong> {selectedAlert.date}</p>
              <p><strong>⏰ D-Day:</strong> <span className={selectedAlert.d_day <= 7 ? 'text-red-600 font-bold' : ''}>{selectedAlert.d_day}일 남음</span></p>
            </div>
          </div>

          <div className="mt-4">
            <h5 className="font-semibold text-sm mb-2">✅ 준비사항 체크리스트</h5>
            <div className="space-y-1">
              {(alertDetails[selectedAlert.title]?.checklist || ['관련 서류 준비', '세무 담당자 확인']).map((item, i) => (
                <label key={i} className="flex items-center gap-2 text-xs cursor-pointer hover:bg-muted/50 p-1 rounded">
                  <input type="checkbox" className="rounded" />
                  {item}
                </label>
              ))}
            </div>
          </div>

          <div className="mt-4">
            <h5 className="font-semibold text-sm mb-2">📄 필요 서류</h5>
            <div className="flex flex-wrap gap-1">
              {(alertDetails[selectedAlert.title]?.docs || ['신고서']).map((doc, i) => (
                <span key={i} className="text-[10px] bg-muted px-2 py-1 rounded">{doc}</span>
              ))}
            </div>
          </div>
        </div>
      ) : (
        // 목록 모드
        <>
          <div className="space-y-4 flex-1 overflow-y-auto pr-1">
            {alerts.map((alert, i) => (
              <div
                key={i}
                onClick={() => setSelectedAlert(alert)}
                className="flex gap-4 items-center cursor-pointer hover:bg-muted/50 p-2 -m-2 rounded-lg transition-colors"
              >
                <div className={`w-12 h-12 rounded-lg flex flex-col items-center justify-center text-xs font-bold ${alert.d_day <= 7 ? 'bg-red-100 text-red-600' : 'bg-secondary text-secondary-foreground'}`}>
                  <span>D-{alert.d_day}</span>
                </div>
                <div className="flex-1">
                  <h4 className="font-medium text-sm">{alert.title}</h4>
                  <p className="text-xs text-muted-foreground">{alert.date} 마감</p>
                </div>
                {alert.type === 'mandatory' && <span className="text-[10px] bg-red-100 text-red-600 px-1.5 py-0.5 rounded">필수</span>}
                <ChevronRight className="w-4 h-4 text-muted-foreground" />
              </div>
            ))}
          </div>
          <button className="w-full mt-4 py-2 text-xs font-medium text-muted-foreground hover:bg-muted rounded-md transition-colors flex items-center justify-center gap-1">
            전체 일정 보기 <ChevronRight className="w-3 h-3" />
          </button>
        </>
      )}
    </div>
  );
};

// 4. Financial Analysis Component
const FinancialAnalysis = ({ revenue = 150000000, industry = 'startup' }: { revenue?: number; industry?: string }) => {
  const [analysis, setAnalysis] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [quickRatios, setQuickRatios] = useState<Record<string, any> | null>(null);
  const [quickHealth, setQuickHealth] = useState<any | null>(null);
  const [quickLoading, setQuickLoading] = useState(false);

  useEffect(() => {
    fetchAnalysis();
  }, [revenue, industry]);

  useEffect(() => {
    fetchQuickStats();
  }, [revenue, industry]);

  const fetchAnalysis = async () => {
    setLoading(true);
    try {
      const res = await api.getFinancialAnalysis(revenue, industry);
      setAnalysis(res);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const fetchQuickStats = async () => {
    setQuickLoading(true);
    try {
      const [ratiosRes, healthRes] = await Promise.all([
        api.getFinancialRatios(revenue, industry),
        api.getFinancialHealth(revenue, industry)
      ]);
      setQuickRatios(ratiosRes?.ratios || null);
      setQuickHealth(healthRes || null);
    } catch (e) {
      console.error(e);
    } finally {
      setQuickLoading(false);
    }
  };



  const getGradeColor = (grade: string) => {
    if (grade.startsWith('A')) return 'text-green-600 bg-green-100';
    if (grade.startsWith('B')) return 'text-blue-600 bg-blue-100';
    if (grade === 'C') return 'text-yellow-600 bg-yellow-100';
    return 'text-red-600 bg-red-100';
  };

  if (loading) return <div className="p-6 bg-card rounded-xl border shadow-sm">로딩 중...</div>;

  return (
    <div className="p-6 bg-card rounded-xl border shadow-sm">
      <div className="flex justify-between items-center mb-4">
        <h3 className="font-bold flex items-center gap-2">
          <TrendingUp className="w-5 h-5 text-primary" />
          재무 분석
        </h3>
        {analysis?.health_score && (
          <span className={`text-lg font-bold px-3 py-1 rounded-full ${getGradeColor(analysis.health_score.grade)}`}>
            {analysis.health_score.grade}
          </span>
        )}
      </div>

      {analysis?.health_score && (
        <div className="mb-4 p-3 rounded-lg bg-muted/30">
          <div className="flex justify-between text-sm mb-1">
            <span>재무 건전성</span>
            <span className="font-bold">{analysis.health_score.total_score}/100점</span>
          </div>
          <div className="w-full bg-gray-200 rounded-full h-2">
            <div
              className={`h-2 rounded-full transition-all ${analysis.health_score.total_score >= 70 ? 'bg-green-500' : analysis.health_score.total_score >= 50 ? 'bg-yellow-500' : 'bg-red-500'}`}
              style={{ width: `${analysis.health_score.total_score}%` }}
            />
          </div>
          <p className="text-xs text-muted-foreground mt-1">{analysis.health_score.grade_desc}</p>
        </div>
      )}

      <div className="grid grid-cols-2 gap-2 mb-4">
        {analysis?.ratios && Object.entries(analysis.ratios).slice(0, 6).map(([key, ratio]: [string, any]) => (
          <div key={key} className="p-2 rounded bg-muted/20 text-center">
            <p className="text-[10px] text-muted-foreground truncate">{ratio.name}</p>
            <p className="font-bold text-sm">{ratio.value}{ratio.unit}</p>
          </div>
        ))}
      </div>

      {analysis?.recommendations && analysis.recommendations.length > 0 && (
        <div className="border-t pt-3">
          <p className="text-xs font-semibold mb-2">💡 개선 권고</p>
          {analysis.recommendations.slice(0, 2).map((rec: any, i: number) => (
            <div key={i} className={`text-xs p-2 rounded mb-1 ${rec.priority === 'high' ? 'bg-red-50 text-red-700' : rec.priority === 'medium' ? 'bg-yellow-50 text-yellow-700' : 'bg-blue-50 text-blue-700'}`}>
              {rec.issue}
            </div>
          ))}
        </div>
      )}

      <div className="border-t pt-3 mt-3">
        <div className="flex items-center justify-between mb-2">
          <p className="text-xs font-semibold">⚡ 빠른 지표 (Health/Ratios API)</p>
          <button
            onClick={fetchQuickStats}
            disabled={quickLoading}
            className="text-[10px] px-2 py-1 rounded border hover:bg-muted disabled:opacity-60"
          >
            {quickLoading ? "갱신중..." : "새로고침"}
          </button>
        </div>
        {quickLoading && (
          <div className="text-xs text-muted-foreground mb-2">불러오는 중...</div>
        )}
        {!quickLoading && !quickHealth && !quickRatios && (
          <div className="text-xs text-muted-foreground mb-2">빠른 지표를 불러오지 못했습니다.</div>
        )}
        {quickHealth && (
          <div className="text-xs text-muted-foreground mb-2">
            재무 건전성: <span className="font-semibold text-gray-800">{quickHealth.total_score || 0}/100</span> ({quickHealth.grade || '-'})
          </div>
        )}
        {quickRatios && (
          <div className="grid grid-cols-3 gap-2">
            {Object.entries(quickRatios).slice(0, 3).map(([key, ratio]: [string, any]) => (
              <div key={key} className="p-2 rounded bg-muted/10 text-center">
                <p className="text-[10px] text-muted-foreground truncate">{ratio.name}</p>
                <p className="font-bold text-xs">{ratio.value}{ratio.unit}</p>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

// 5. Business Lookup Component
const BusinessLookup = () => {
  const [bizNum, setBizNum] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [result, setResult] = useState<any>(null);
  const [loading, setLoading] = useState(false);



  const handleLookup = async () => {
    if (bizNum.replace(/-/g, '').length !== 10) {
      alert('사업자등록번호 10자리를 입력하세요');
      return;
    }
    setLoading(true);
    try {
      // Pass apiKey if user provided it
      const res = await api.lookupBusiness(bizNum, apiKey);
      setResult(res);
    } catch (e) {
      console.error(e);
      setResult({ success: false, message: 'API 오류' });
    } finally {
      setLoading(false);
    }
  };

  const formatBizNum = (value: string) => {
    const nums = value.replace(/[^0-9]/g, '').slice(0, 10);
    if (nums.length <= 3) return nums;
    if (nums.length <= 5) return `${nums.slice(0, 3)}-${nums.slice(3)}`;
    return `${nums.slice(0, 3)}-${nums.slice(3, 5)}-${nums.slice(5)}`;
  };

  return (
    <div className="p-6 bg-card rounded-xl border shadow-sm">
      <h3 className="font-bold flex items-center gap-2 mb-4">
        <Building className="w-5 h-5 text-primary" />
        사업자 정보 조회
      </h3>

      <div className="flex gap-2 mb-4">
        <input
          type="text"
          value={bizNum}
          onChange={(e) => setBizNum(formatBizNum(e.target.value))}
          placeholder="000-00-00000"
          className="flex-1 px-3 py-2 border rounded-lg text-sm"
        />
        <button
          onClick={handleLookup}
          disabled={loading}
          className="px-4 py-2 bg-primary text-white rounded-lg text-sm font-medium hover:bg-primary/90 disabled:opacity-50"
        >
          {loading ? '조회중...' : '조회'}
        </button>
      </div>

      {result && (
        <div className={`p-4 rounded-lg ${result.success ? 'bg-green-50 border border-green-200' : 'bg-red-50 border border-red-200'}`}>
          {result.success ? (
            <div className="space-y-2 text-sm">
              <div className="flex justify-between">
                <span className="text-muted-foreground">사업자번호</span>
                <span className="font-medium">{result.data.b_no}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">상태</span>
                <span className={`font-medium ${result.data.b_stt === '계속사업자' ? 'text-green-600' : 'text-red-600'}`}>
                  {result.data.b_stt}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">과세유형</span>
                <span className="font-medium">{result.data.tax_type}</span>
              </div>
              {result.data.company_name && (
                <div className="flex justify-between">
                  <span className="text-muted-foreground">상호</span>
                  <span className="font-medium">{result.data.company_name}</span>
                </div>
              )}
              {result.data.industry && (
                <div className="flex justify-between">
                  <span className="text-muted-foreground">업종</span>
                  <span className="font-medium">{result.data.industry}</span>
                </div>
              )}
            </div>
          ) : (
            <p className="text-red-600 text-sm">{result.message}</p>
          )}
          {result.message?.includes('MOCK') && (
            <div className="mt-3 pt-3 border-t border-red-200">
              <p className="text-xs text-red-600 mb-2 font-medium">
                ⚠️ 테스트 데이터입니다. 실제 조회를 위해 API 키가 필요합니다.
              </p>
              <div className="flex gap-2">
                <input
                  type="password"
                  placeholder="공공데이터포털 API Key 입력"
                  className="flex-1 px-2 py-1 text-xs border rounded bg-white"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                />
                <button
                  onClick={handleLookup}
                  className="px-2 py-1 bg-gray-800 text-white text-xs rounded hover:bg-black"
                >
                  재조회
                </button>
              </div>
              <a href="https://www.data.go.kr/data/15081808/openapi.do" target="_blank" rel="noreferrer" className="text-[10px] text-blue-500 hover:underline mt-1 block">
                키 발급받기 (공공데이터포털) &rarr;
              </a>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

const PredictionCard = ({ rec }: { rec: Recommendation }) => {
  const [isExpanded, setIsExpanded] = useState(false);

  return (
    <div
      onClick={() => setIsExpanded(!isExpanded)}
      className={`p-6 bg-card rounded-xl border shadow-sm hover:shadow-md transition-all cursor-pointer ${isExpanded ? 'ring-2 ring-primary/20' : ''}`}
    >
      <div className="flex justify-between items-start mb-2">
        <div className="flex items-center gap-2">
          <Calendar className="w-5 h-5 text-primary" />
          <h3 className="font-bold text-lg">{rec.title}</h3>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs bg-sidebar-accent px-2 py-1 rounded">2026 예측</span>
          <ChevronRight className={`w-4 h-4 text-gray-400 transition-transform ${isExpanded ? 'rotate-90' : ''}`} />
        </div>
      </div>

      <div className="mt-4 space-y-2">
        <p className="text-sm text-muted-foreground">예상 공고일</p>
        <p className="text-xl font-semibold text-primary">{rec.predicted_date}</p>
        <p className="text-xs text-muted-foreground">구간: {rec.range}</p>
      </div>

      <div className="mt-4 pt-4 border-t text-xs text-muted-foreground">
        <p>근거: {rec.reason} ({rec.confidence})</p>
      </div>

      {/* Expanded Details */}
      {isExpanded && (
        <div className="mt-4 pt-4 border-t border-dashed space-y-3 animate-in fade-in slide-in-from-top-1">
          {rec.funding_limit && (
            <div>
              <span className="block font-semibold text-gray-700 mb-1">💰 지원 규모</span>
              <p className="text-sm">{rec.funding_limit}</p>
            </div>
          )}
          {rec.eligibility && (
            <div>
              <span className="block font-semibold text-gray-700 mb-1">📋 신청 자격</span>
              <p className="text-sm text-gray-600">{rec.eligibility}</p>
            </div>
          )}
          {rec.strategy && (
            <div className="bg-blue-50 p-3 rounded-lg border border-blue-100">
              <span className="block font-semibold text-blue-700 mb-1">💡 선정 전략</span>
              <p className="text-sm text-gray-700 leading-relaxed">{rec.strategy}</p>
            </div>
          )}
          {rec.link && (
            <div className="pt-2 text-right">
              <a
                href={rec.link}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center text-xs font-bold text-primary hover:underline bg-primary/5 px-2 py-1 rounded"
                onClick={(e) => e.stopPropagation()}
              >
                공고 상세 보기 <ExternalLink className="w-3 h-3 ml-1" />
              </a>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

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
  const [isComplete, setIsComplete] = useState(false);

  useEffect(() => {
    if (currentStep < steps.length) {
      const timeout = setTimeout(() => {
        setCurrentStep(prev => prev + 1);
      }, 800 + Math.random() * 500);
      return () => clearTimeout(timeout);
    } else if (!isComplete) {
      setIsComplete(true);
    }
  }, [currentStep, isComplete, steps.length]);

  useEffect(() => {
    if (isComplete) {
      const timeout = setTimeout(() => {
        onComplete();
      }, 500);
      return () => clearTimeout(timeout);
    }
  }, [isComplete, onComplete]);

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
const Onboarding = ({ onStart }: { onStart: (name: string, company: string, bizNum: string, type: string, targetRevenue?: number) => void }) => {
  const [step, setStep] = useState<"type" | "info">("type");
  const [selectedType, setSelectedType] = useState<string>("general");

  const [name, setName] = useState("");
  const [company, setCompany] = useState("");
  const [bizNum, setBizNum] = useState("");
  const [isPreFounder, setIsPreFounder] = useState(false);
  const [targetRevenue, setTargetRevenue] = useState("0");
  const [isScraping, setIsScraping] = useState(false);
  const [showCalculator, setShowCalculator] = useState(false);

  const handleRevenueChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    // Basic number formatting
    const raw = e.target.value.replace(/[^0-9]/g, '');
    const num = parseInt(raw, 10);
    if (!isNaN(num)) {
      setTargetRevenue(num.toLocaleString());
    } else {
      setTargetRevenue("");
    }
  };

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
    if (name && company && (bizNum || isPreFounder)) {
      setIsScraping(true);
    }
  };

  const handleComplete = useCallback(() => {
    const revenueNum = parseInt(targetRevenue.replace(/,/g, ''), 10) || 150000000;
    onStart(name, company, isPreFounder ? "PRE_FOUNDER" : bizNum, selectedType, isPreFounder ? revenueNum : undefined);
  }, [onStart, name, company, isPreFounder, bizNum, selectedType, targetRevenue]);

  if (isScraping) {
    return <LoadingScraper onComplete={handleComplete} />;
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
    <>
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
              <div className="space-y-2">
                <input
                  type="text"
                  required={!isPreFounder}
                  disabled={isPreFounder}
                  maxLength={12}
                  className={`w-full px-4 py-3 rounded-lg border focus:ring-2 focus:ring-primary/20 outline-none transition-all font-mono tracking-widest ${isPreFounder ? 'bg-gray-100 text-gray-400 cursor-not-allowed' : ''}`}
                  placeholder="000-00-00000"
                  value={bizNum}
                  onChange={(e) => setBizNum(e.target.value)}
                />
                <label className="flex items-center gap-2 cursor-pointer select-none">
                  <input
                    type="checkbox"
                    className="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary"
                    checked={isPreFounder}
                    onChange={(e) => {
                      setIsPreFounder(e.target.checked);
                      if (e.target.checked) setBizNum("");
                    }}
                  />
                  <span className="text-sm text-gray-500">아직 사업자등록번호가 없습니다 (예비창업자)</span>
                </label>

                {isPreFounder && (
                  <div className="mt-4 p-4 bg-blue-50/50 rounded-lg border border-blue-100 animate-in fade-in slide-in-from-top-2 duration-300">
                    <label className="block text-sm font-medium text-blue-900 mb-3">
                      1년차 예상 매출
                    </label>

                    {targetRevenue === "0" || targetRevenue === "" ? (
                      <button
                        type="button"
                        onClick={() => setShowCalculator(true)}
                        className="w-full py-4 bg-white border-2 border-blue-200 border-dashed rounded-lg text-blue-600 font-bold hover:bg-blue-50 hover:border-blue-400 transition-all flex flex-col items-center gap-1"
                      >
                        <Calculator className="w-5 h-5 mb-1" />
                        AI 매출 추정 마법사 실행
                        <span className="text-xs font-normal text-blue-400">간단한 질문으로 목표 매출을 계산합니다</span>
                      </button>
                    ) : (
                      <div className="relative group">
                        <div className="absolute top-0 right-0 -mt-2 -mr-2 flex gap-1">
                          <button type="button" onClick={() => setShowCalculator(true)} className="bg-white border text-xs px-2 py-1 rounded shadow-sm hover:bg-gray-50 text-gray-600">재계산</button>
                        </div>
                        <input
                          type="text"
                          className="w-full pl-4 pr-10 py-3 rounded border border-blue-200 focus:ring-2 focus:ring-blue-500/20 outline-none text-right font-extrabold text-xl text-blue-800 tracking-tight"
                          value={targetRevenue}
                          onChange={handleRevenueChange}
                        />
                        <span className="absolute right-4 top-1/2 -translate-y-1/2 text-blue-500 font-medium">원</span>
                      </div>
                    )}

                    <p className="text-[11px] text-blue-600/60 mt-2 text-center">
                      * 이 금액을 목표로 세무 시뮬레이션을 진행합니다.
                    </p>
                  </div>
                )}
              </div>
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

      {showCalculator && (
        <RevenueCalculator
          onCancel={() => setShowCalculator(false)}
          onComplete={(val) => {
            setTargetRevenue(val.toLocaleString());
            setShowCalculator(false);
          }}
        />
      )}
    </>
  );
};

// --- Main App ---

function App() {
  const [activeTab, setActiveTab] = useState<string>("home");
  const [dashboardTab, setDashboardTab] = useState<"home" | "accounting" | "management">("home");
  const [isSidebarOpen, setIsSidebarOpen] = useState(true); // Keep existing isSidebarOpen
  const [isLoading, setIsLoading] = useState(false); // New general isLoading
  const [isQuestionOpen, setIsQuestionOpen] = useState(false);
  const [isSilent, setIsSilent] = useState(false);
  const [isDetailOpen, setIsDetailOpen] = useState(false);
  const [showDecisionDetails, setShowDecisionDetails] = useState(false);
  const [showDecisionSimulation, setShowDecisionSimulation] = useState(false);

  // -- Tax/Accounting Feature States --
  const [vatRevenue, setVatRevenue] = useState(0);
  const [vatPurchase, setVatPurchase] = useState(0);


  const [deductionChecklist, setDeductionChecklist] = useState([
    { id: 1, label: "사업용 신용카드 등록", checked: false, tip: "홈택스에 등록된 카드만 공제 가능" },
    { id: 2, label: "임차료 전자세금계산서 수취", checked: false, tip: "건물주에게 요청 필수" },
    { id: 3, label: "차량 운행일지 작성", checked: false, tip: "업무용 승용차 비용 인정 요건" },
    { id: 4, label: "접대비 증빙 보관", checked: false, tip: "20만원 이상은 법인카드 필수" },
    { id: 5, label: "통신비 사업자 전환", checked: false, tip: "대표자 명의 -> 사업자 명의 변경" },
  ]);

  const toggleDeduction = (id: number) => {
    setDeductionChecklist(prev => prev.map(item => item.id === id ? { ...item, checked: !item.checked } : item));
  };

  // -- Management Feature States --
  const [runwayCash, setRunwayCash] = useState(150000000);
  const [runwayBurn, setRunwayBurn] = useState(12500000);
  const estimatedMonths = runwayBurn > 0 ? (runwayCash / runwayBurn).toFixed(1) : "∞";

  const [financeDatasets, setFinanceDatasets] = useState<Record<string, { cash: number; monthlyRevenue: number; monthlyExpense: number; breakdown?: Record<string, number> }>>({});
  const [activeDatasetName, setActiveDatasetName] = useState<string>("");
  const [presetPreview, setPresetPreview] = useState<PresetDataset | null>(null);

  const activeDatasetMetrics = useMemo(() => {
    const dataset = activeDatasetName ? financeDatasets[activeDatasetName] : null;
    return {
      cash: Number(dataset?.cash || 0),
      monthlyRevenue: Number(dataset?.monthlyRevenue || 0),
      monthlyExpense: Number(dataset?.monthlyExpense || 0),
    };
  }, [activeDatasetName, financeDatasets]);

  const dataStatus = useMemo(() => {
    if (!activeDatasetName) return "none";
    const filled = [activeDatasetMetrics.cash, activeDatasetMetrics.monthlyRevenue, activeDatasetMetrics.monthlyExpense].filter((value) => Number.isFinite(value) && value > 0).length;
    if (filled >= 2) return "ready";
    return "partial";
  }, [activeDatasetName, activeDatasetMetrics.cash, activeDatasetMetrics.monthlyRevenue, activeDatasetMetrics.monthlyExpense]);

  const localDashboardData = useMemo(() => {
    if (dataStatus !== "ready") return null;
    if (!activeDatasetName) return null;
    const dataset = financeDatasets[activeDatasetName];
    if (!dataset) return null;
    const monthlyRevenue = dataset.monthlyRevenue || 0;
    const monthlyExpense = dataset.monthlyExpense || 0;
    const cash = dataset.cash || 0;
    const yearlyRevenue = monthlyRevenue * 12;
    const yearlyProfit = (monthlyRevenue - monthlyExpense) * 12;
    return {
      stats: [
        { title: "예상 매출 (12M)", value: `₩${yearlyRevenue.toLocaleString()}`, change: "프리셋", trend: "neutral", desc: "데이터셋 기준" },
        { title: "예상 순이익", value: `₩${yearlyProfit.toLocaleString()}`, change: yearlyProfit >= 0 ? "+" : "-", trend: yearlyProfit >= 0 ? "up" : "down", desc: "수익-지출 기준" },
        { title: "현재 현금성 자산", value: `₩${cash.toLocaleString()}`, change: "프리셋", trend: "neutral", desc: "현금 입력값" },
        { title: "평균 Burn Rate", value: `₩${monthlyExpense.toLocaleString()}`, change: "프리셋", trend: "neutral", desc: "월 평균 지출" },
      ],
      chart: Array.from({ length: 12 }, (_, i) => ({ name: `${i + 1}월`, income: monthlyRevenue, expense: monthlyExpense })),
    };
  }, [financeDatasets, activeDatasetName, dataStatus]);

  const formatDelta = (value: number, unit = "") => {
    const rounded = Math.round(value);
    const prefix = rounded > 0 ? "+" : "";
    return `${prefix}${rounded.toLocaleString()}${unit}`;
  };

  const applyPresetDataset = useCallback((preset: PresetDataset) => {
    setFinanceDatasets((prev) => ({ ...prev, [preset.name]: { ...preset.meta } }));
    setActiveDatasetName(preset.name);
    setRunwayCash(preset.meta.cash);
    setRunwayBurn(preset.meta.monthlyExpense);
  }, []);

  const [showCompetitorCompare, setShowCompetitorCompare] = useState(false);
  // -----------------------------------
  // -----------------------------------

  // User State (Auth)


  // Data States
  const [dashboardData, setDashboardData] = useState<any>(null);

  // Effective dashboard data: use preset data when available, otherwise use API data
  const effectiveDashboardData = useMemo(() => {
    if (dataStatus === "ready" && activeDatasetName && localDashboardData) {
      return localDashboardData;
    }
    return dashboardData;
  }, [dataStatus, activeDatasetName, localDashboardData, dashboardData]);

  const [recommendations, setRecommendations] = useState<Recommendation[]>([]);
  const [competitions, setCompetitions] = useState<Competition[]>([]);

  // Advanced SaaS States
  const [risk, setRisk] = useState<RiskAnalysis | null>(null);
  const [calendarAlerts, setCalendarAlerts] = useState<CalendarAlert[]>([]);

  // Tax Simulation States
  const [taxSimForm, setTaxSimForm] = useState<Record<string, boolean>>({
    salary_increase: false,
    equipment_depreciation: false,
    rd_deduction: false,
    investment_credit: false,
  });
  const [taxSimResult, setTaxSimResult] = useState<{
    total_saving: number;
    details: Array<{ item: string; amount: number }>;
    message: string;
  } | null>(null);
  const [isSimulating, setIsSimulating] = useState(false);

  // NTS Document States
  const [ntsFile, setNtsFile] = useState<File | null>(null);
  const [ntsPassword, setNtsPassword] = useState("");
  const [ntsResult, setNtsResult] = useState<any>(null);
  const [isUploadingNts, setIsUploadingNts] = useState(false);

  // Subsidies State
  const [subsidies, setSubsidies] = useState<Array<{
    title: string;
    org: string;
    start_date: string;
    end_date: string;
    link: string;
    tags: string[];
  }>>([]);

  // Business Lookup State
  const [bizLookupResult, setBizLookupResult] = useState<any>(null);
  const [isLookingUp, setIsLookingUp] = useState(false);

  // --- Handler Functions (after state declarations) ---

  // Tax Simulation Handler
  const handleTaxSimulation = useCallback(async () => {
    if (isSimulating) return;
    setIsSimulating(true);
    try {
      const result = await api.simulateTax(taxSimForm);
      setTaxSimResult(result);
      showToast(`예상 절세액: ${result.total_saving.toLocaleString()}원`, "success");
    } catch (err) {
      console.error("Tax simulation failed:", err);
      showToast("세금 시뮬레이션 실패", "error");
    } finally {
      setIsSimulating(false);
    }
  }, [taxSimForm, isSimulating]);

  // NTS Document Upload Handler
  const handleNtsUpload = useCallback(async () => {
    if (!ntsFile || isUploadingNts) return;
    setIsUploadingNts(true);
    try {
      const result = await api.uploadNTSDocument(ntsFile, ntsPassword);
      setNtsResult(result);
      showToast("국세청 문서 분석 완료", "success");
    } catch (err) {
      console.error("NTS upload failed:", err);
      showToast("국세청 문서 업로드 실패", "error");
    } finally {
      setIsUploadingNts(false);
    }
  }, [ntsFile, ntsPassword, isUploadingNts]);

  // Business Lookup Handler
  const handleBizLookup = useCallback(async (bizNum: string) => {
    if (isLookingUp || !bizNum) return;
    setIsLookingUp(true);
    setBizLookupResult(null);
    try {
      const result = await api.lookupBusiness(bizNum);
      setBizLookupResult(result);
      if (result?.valid) {
        showToast("유효한 사업자등록번호입니다", "success");
      } else {
        showToast("유효하지 않은 사업자등록번호입니다", "error");
      }
    } catch (err) {
      console.error("Business lookup failed:", err);
      showToast("사업자조회 실패", "error");
    } finally {
      setIsLookingUp(false);
    }
  }, [isLookingUp]);


  // Chat States
  const [messages, setMessages] = useState<ChatMessage[]>([
    { role: "assistant", content: "AI CFO 질문 패널입니다. 결정을 위한 근거가 필요하면 말씀해 주세요." },
  ]);
  const loadingStartRef = useRef<number | null>(null);
  const [decisionHistory, setDecisionHistory] = useState<DecisionRecord[]>([]);
  const decisionCardRef = useRef<HTMLDivElement | null>(null);
  const accountingRef = useRef<HTMLDivElement | null>(null);
  const managementRef = useRef<HTMLDivElement | null>(null);
  const dashboardRef = useRef<HTMLDivElement | null>(null);
  const [highlightKey, setHighlightKey] = useState<"dashboard" | "accounting" | "management" | "home" | null>(null);
  const [showRejectionModal, setShowRejectionModal] = useState(false);
  const [rejectionInput, setRejectionInput] = useState("");
  const [showOutcomeModal, setShowOutcomeModal] = useState(false);
  const [outcomeTarget, setOutcomeTarget] = useState<DecisionRecord | null>(null);
  const [outcomeStatus, setOutcomeStatus] = useState<"positive" | "negative" | "neutral">("neutral");
  const [memoryHighlightId, setMemoryHighlightId] = useState<string | null>(null);
  const [memoryFilterKey, setMemoryFilterKey] = useState<string | null>(null);
  const [showSimilarOnly, setShowSimilarOnly] = useState(false);
  const [similarityWeights, setSimilarityWeights] = useState({
    priority: 1,
    runway: 1,
    risk: 1,
  });
  const [tonePreference, setTonePreference] = useState<"direct" | "neutral" | "soft">("neutral");
  const [warningThreshold, setWarningThreshold] = useState(2);
  const [summaryLength, setSummaryLength] = useState<"short" | "normal" | "detailed">("normal");
  const [showSimilarOutcomes, setShowSimilarOutcomes] = useState(true);
  const [memoryGraph, setMemoryGraph] = useState<{ nodes: GraphNode[]; edges: GraphEdge[] } | null>(null);
  const [graphPositions, setGraphPositions] = useState<Record<string, { x: number; y: number }>>({});
  const [draggingNodeId, setDraggingNodeId] = useState<string | null>(null);
  const [selectedGraphNode, setSelectedGraphNode] = useState<GraphNode | null>(null);
  const [hoveredEdgeKey, setHoveredEdgeKey] = useState<string | null>(null);
  const [selectedEdgeKey, setSelectedEdgeKey] = useState<string | null>(null);
  const [layoutSaved, setLayoutSaved] = useState(false);
  const [outcomeFilter, setOutcomeFilter] = useState<"all" | "positive" | "neutral" | "negative" | "pending">("all");
  const [showCalibrationModal, setShowCalibrationModal] = useState(false);
  const [calibrationStep, setCalibrationStep] = useState(1);
  const [isCalibrating, setIsCalibrating] = useState(false);
  const [user, setUser] = useState<{
    name: string;
    company: string;
    bizNum: string;
    type: string;
    targetRevenue?: number;
    activeMCPs: string[];
    rfiData: any;
  } | null>(null);
  const [profileDraft, setProfileDraft] = useState<{
    name: string;
    company: string;
    bizNum: string;
    type: string;
    targetRevenue: string;
  }>({
    name: "",
    company: "",
    bizNum: "",
    type: "general",
    targetRevenue: ""
  });

  const [input, setInput] = useState("");
  const [outcomeMemo, setOutcomeMemo] = useState("");
  const [showNotifications, setShowNotifications] = useState(false);

  // Toast State
  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' | 'info' } | null>(null);

  const showToast = (message: string, type: 'success' | 'error' | 'info' = 'info') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  // Loading States
  const [loadingElapsed, setLoadingElapsed] = useState(0);

  // Progressive loading message based on elapsed time
  useEffect(() => {
    if (!isLoading) {
      setLoadingElapsed(0);
      return;
    }
    const interval = setInterval(() => {
      setLoadingElapsed(prev => prev + 1);
    }, 1000);
    return () => clearInterval(interval);
  }, [isLoading]);

  const getLoadingMessage = () => {
    if (loadingElapsed < 2) return "생각하는 중...";
    if (loadingElapsed < 5) return "세무 자료 검색 중...";
    if (loadingElapsed < 10) return "AI가 분석하고 있어요...";
    return "조금만 기다려 주세요...";
  };
  const [isSavingProfile, setIsSavingProfile] = useState(false);
  const [isSavingRFI, setIsSavingRFI] = useState(false);
  const [isSavingSettings, setIsSavingSettings] = useState(false);
  const [isSavingCfoSettings, setIsSavingCfoSettings] = useState(false);
  const [apiHealth, setApiHealth] = useState<{ status: string } | null>(null);
  const [apiHealthCheckedAt, setApiHealthCheckedAt] = useState<string | null>(null);
  const [apiHealthLoading, setApiHealthLoading] = useState(false);
  const [apiHealthError, setApiHealthError] = useState<string | null>(null);
  const [ntsDocTypes, setNtsDocTypes] = useState<Array<{ code: string; name: string; supported: boolean }>>([]);
  const [ntsDocTypesLoading, setNtsDocTypesLoading] = useState(false);

  const fetchApiHealth = useCallback(async () => {
    setApiHealthLoading(true);
    setApiHealthError(null);
    try {
      const res = await api.getHealth();
      setApiHealth(res);
      setApiHealthCheckedAt(new Date().toLocaleString());
    } catch (e) {
      console.error(e);
      setApiHealth(null);
      setApiHealthError("연결 실패");
    } finally {
      setApiHealthLoading(false);
    }
  }, []);

  const fetchNtsDocTypes = useCallback(async () => {
    setNtsDocTypesLoading(true);
    try {
      const res = await api.getNTSDocumentTypes();
      setNtsDocTypes(res.types || []);
    } catch (e) {
      console.error(e);
      setNtsDocTypes([]);
    } finally {
      setNtsDocTypesLoading(false);
    }
  }, []);

  const mapMemoryToDecision = useCallback((record: MemoryRecord): DecisionRecord => {
    return {
      id: record.id,
      title: record.title,
      summary: record.summary,
      status: record.status === 'accepted' ? 'accepted' : 'rejected',
      createdAt: record.created_at,
      reasons: record.reasons || [],
      impact: record.impact || '',
      priorityScore: record.priority_score || 50,
      runwayMonths: record.runway_months || 0,
      riskScore: record.risk_score || 0,
      outcomeStatus: record.outcome_status || "pending",
      outcomeMemo: record.outcome_memo || "",
      rejectionReason: record.rejection_reason
    };
  }, []);
  // Initial Fetch
  useEffect(() => {
    if (user) {
      const safeMCPs = user.activeMCPs || [];
      const teamSize = parseInt(user.rfiData?.teamSize) || 0;
      const budget = parseInt(String(user.rfiData?.budget).replace(/[^0-9]/g, '')) || 0;
      api.getDashboard(user.bizNum, safeMCPs, user.targetRevenue, teamSize, budget)
        .then(setDashboardData)
        .catch(err => {
          console.error("Dashboard fetch failed:", err);
          // Fallback data to prevent infinite loading
          setDashboardData({
            stats: [
              { title: "예상 매출 (2026)", value: "₩0", change: "0%", trend: "neutral", desc: "데이터 수신 실패" },
              { title: "예상 순이익", value: "₩0", change: "0%", trend: "neutral", desc: "재무제표 확인 필요" },
              { title: "현재 현금성 자산", value: "₩0", change: "0%", trend: "neutral", desc: "계좌 연동 필요" },
              { title: "평균 Burn Rate", value: "₩0", change: "0%", trend: "neutral", desc: "지출 내역 없음" }
            ],
            chart: []
          });
          showToast("서버 연결에 실패하여 데모 모드로 전환됩니다.", "error");
        });
      api.getRecommendations(user.type || 'startup').then(setRecommendations).catch(console.error);
      api.getCompetitions().then(setCompetitions).catch(console.error);
      // Advanced
      api.getTaxRisk(user.bizNum, safeMCPs).then(setRisk).catch(console.error);
      api.getCalendarAlerts().then(res => setCalendarAlerts(res.alerts)).catch(console.error);
      api.getSubsidies().then(setSubsidies).catch(console.error);
    }
  }, [user]);

  useEffect(() => {
    fetchApiHealth();
    fetchNtsDocTypes();
  }, [fetchApiHealth, fetchNtsDocTypes]);

  useEffect(() => {
    if (!user) return;
    setProfileDraft({
      name: user.name || "",
      company: user.company || "",
      bizNum: user.bizNum || "",
      type: user.type || "general",
      targetRevenue: user.targetRevenue ? String(user.targetRevenue) : ""
    });
  }, [user]);

  useEffect(() => {
    if (!user?.bizNum) return;
    const storageKey = `taxai_memory_${user.bizNum || user.company}`;
    api.getMemory(user.bizNum)
      .then((res) => {
        const mapped = (res.records || []).map(mapMemoryToDecision);
        setDecisionHistory(mapped);
        localStorage.setItem(storageKey, JSON.stringify(mapped));
        if (mapped.length === 0) {
          setShowCalibrationModal(true);
        }
      })
      .catch((err) => {
        console.error("Failed to load decision history:", err);
        const saved = localStorage.getItem(storageKey);
        if (saved) {
          try {
            const parsed = JSON.parse(saved) as DecisionRecord[];
            if (Array.isArray(parsed)) {
              setDecisionHistory(parsed);
              if (parsed.length === 0) setShowCalibrationModal(true);
            } else {
              setShowCalibrationModal(true);
            }
          } catch (error) {
            console.error("Failed to load local decision history:", error);
            setShowCalibrationModal(true);
          }
        } else {
          setShowCalibrationModal(true);
        }
      });
  }, [user, mapMemoryToDecision]);

  useEffect(() => {
    if (!user) return;
    const storageKey = `taxai_memory_${user.bizNum || user.company}`;
    localStorage.setItem(storageKey, JSON.stringify(decisionHistory));
  }, [decisionHistory, user]);

  useEffect(() => {
    if (!highlightKey) return;
    const timer = window.setTimeout(() => setHighlightKey(null), 2500);
    return () => window.clearTimeout(timer);
  }, [highlightKey]);

  useEffect(() => {
    if (!user) return;
    const storageKey = `taxai_similarity_weights_${user.bizNum || user.company}`;
    const saved = localStorage.getItem(storageKey);
    if (saved) {
      try {
        const parsed = JSON.parse(saved) as { priority: number; runway: number; risk: number };
        setSimilarityWeights((prev) => ({ ...prev, ...parsed }));
      } catch (err) {
        console.error("Failed to load similarity weights:", err);
      }
    }
  }, [user]);

  useEffect(() => {
    if (!user) return;
    const storageKey = `taxai_similarity_weights_${user.bizNum || user.company}`;
    localStorage.setItem(storageKey, JSON.stringify(similarityWeights));
  }, [similarityWeights, user]);

  useEffect(() => {
    if (!user) return;
    const storageKey = `taxai_warning_prefs_${user.bizNum || user.company}`;
    const saved = localStorage.getItem(storageKey);
    if (saved) {
      try {
        const parsed = JSON.parse(saved) as { tone: "direct" | "neutral" | "soft"; threshold: number; summaryLength?: "short" | "normal" | "detailed" };
        if (parsed.tone) setTonePreference(parsed.tone);
        if (parsed.threshold) setWarningThreshold(parsed.threshold);
        if (parsed.summaryLength) setSummaryLength(parsed.summaryLength);
      } catch (err) {
        console.error("Failed to load warning preferences:", err);
      }
    }
  }, [user]);

  useEffect(() => {
    if (!user) return;
    const storageKey = `taxai_warning_prefs_${user.bizNum || user.company}`;
    localStorage.setItem(storageKey, JSON.stringify({
      tone: tonePreference,
      threshold: warningThreshold,
      summaryLength,
    }));
  }, [tonePreference, warningThreshold, summaryLength, user]);

  useEffect(() => {
    if (!user?.bizNum) return;
    api.getPreferences(user.bizNum).then((res) => {
      const record = res.record;
      if (!record) return;
      if (record.similarity_weights) {
        setSimilarityWeights((prev) => ({ ...prev, ...record.similarity_weights }));
      }
      if (record.warning_prefs) {
        if (record.warning_prefs.tone) setTonePreference(record.warning_prefs.tone);
        if (record.warning_prefs.threshold) setWarningThreshold(record.warning_prefs.threshold);
        if (record.warning_prefs.summaryLength) setSummaryLength(record.warning_prefs.summaryLength);
      }
      if (record.graph_positions) {
        setGraphPositions(record.graph_positions);
      }
    }).catch((err) => {
      console.error("Failed to load preferences from server:", err);
    });
  }, [user]);

  useEffect(() => {
    if (!user?.bizNum) return;
    api.getMemoryGraph(user.bizNum).then((res) => {
      setMemoryGraph(res);
    }).catch((err) => {
      console.error("Failed to load memory graph:", err);
    });
  }, [user]);

  useEffect(() => {
    if (!user?.bizNum) return;
    const payload = {
      biz_num: user.bizNum,
      similarity_weights: similarityWeights,
      warning_prefs: {
        tone: tonePreference,
        threshold: warningThreshold,
        summaryLength,
      },
    };
    api.savePreferences(payload).catch((err) => {
      console.error("Failed to save preferences:", err);
    });
  }, [similarityWeights, tonePreference, warningThreshold, summaryLength, user]);

  const saveGraphPositions = () => {
    if (!user?.bizNum) return;
    const payload = {
      biz_num: user.bizNum,
      similarity_weights: similarityWeights,
      warning_prefs: {
        tone: tonePreference,
        threshold: warningThreshold,
        summaryLength,
      },
      graph_positions: graphPositions,
    };
    api.savePreferences(payload).catch((err) => {
      console.error("Failed to save graph positions:", err);
    });
    setLayoutSaved(true);
  };

  const saveCfoSettings = async () => {
    if (!user?.bizNum) return;
    setIsSavingCfoSettings(true);
    try {
      const payload = {
        biz_num: user.bizNum,
        similarity_weights: similarityWeights,
        warning_prefs: {
          tone: tonePreference,
          threshold: warningThreshold,
          summaryLength,
        },
      };
      await api.savePreferences(payload);
      showToast("AI CFO 설정이 저장되었습니다.", "success");
    } catch (err) {
      console.error("Failed to save CFO settings:", err);
      showToast("AI CFO 설정 저장에 실패했습니다.", "error");
    } finally {
      setIsSavingCfoSettings(false);
    }
  };

  const handleProfileSave = async () => {
    if (!authUser?.email) {
      showToast("로그인 정보가 없습니다.", "error");
      return;
    }
    setIsSavingProfile(true);
    const trimmedName = profileDraft.name.trim();
    const trimmedCompany = profileDraft.company.trim();
    const rawBizNum = profileDraft.bizNum.trim();
    const digitsBizNum = rawBizNum.replace(/[^0-9]/g, "");
    if (digitsBizNum && digitsBizNum.length !== 10) {
      showToast("사업자번호는 10자리여야 합니다.", "error");
      setIsSavingProfile(false);
      return;
    }
    const formattedBizNum = digitsBizNum
      ? `${digitsBizNum.slice(0, 3)}-${digitsBizNum.slice(3, 5)}-${digitsBizNum.slice(5)}`
      : "";
    const trimmedType = profileDraft.type.trim() || "general";
    const parsedTargetRevenue = profileDraft.targetRevenue
      ? parseInt(profileDraft.targetRevenue.replace(/[^0-9]/g, ""), 10)
      : undefined;

    try {
      const result = await api.updateProfile({
        email: authUser.email,
        name: trimmedName || undefined,
        company: trimmedCompany || undefined,
        biz_num: formattedBizNum || undefined,
        type: trimmedType || undefined,
        target_revenue: parsedTargetRevenue,
      });
      if (result.success) {
        localStorage.setItem("user", JSON.stringify(result.user));
        setAuthUser(result.user);
        setUser((prev) => {
          if (!prev) return prev;
          const existingMCPs = prev.activeMCPs || [];
          const nextMCPs = existingMCPs.includes(trimmedType)
            ? existingMCPs
            : [...existingMCPs, trimmedType];
          return {
            ...prev,
            name: trimmedName || prev.name,
            company: trimmedCompany || prev.company,
            bizNum: formattedBizNum || prev.bizNum,
            type: trimmedType || prev.type,
            targetRevenue: parsedTargetRevenue,
            activeMCPs: nextMCPs,
          };
        });
        showToast("회사 정보가 저장되었습니다.", "success");
      } else {
        showToast(result.message || "회사 정보 저장 실패", "error");
      }
    } catch (err) {
      console.error("Failed to update profile:", err);
      showToast("회사 정보 저장 중 오류가 발생했습니다.", "error");
    } finally {
      setIsSavingProfile(false);
    }
  };

  useEffect(() => {
    if (!layoutSaved) return;
    const timer = window.setTimeout(() => setLayoutSaved(false), 2000);
    return () => window.clearTimeout(timer);
  }, [layoutSaved]);

  useEffect(() => {
    if (summaryLength === "short") {
      setShowSimilarOutcomes(false);
    } else {
      setShowSimilarOutcomes(true);
    }
  }, [summaryLength]);

  useEffect(() => {
    if (!memoryHighlightId || activeTab !== "memory") return;
    const timer = window.setTimeout(() => {
      const el = document.getElementById(`memory-${memoryHighlightId}`);
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "start" });
      }
    }, 100);
    return () => window.clearTimeout(timer);
  }, [memoryHighlightId, activeTab]);

  useEffect(() => {
    if (!memoryHighlightId) return;
    const timer = window.setTimeout(() => setMemoryHighlightId(null), 4000);
    return () => window.clearTimeout(timer);
  }, [memoryHighlightId]);

  const parseStatValue = (raw?: string) => {
    if (!raw) return null;
    const numeric = raw.replace(/[^0-9.-]/g, "");
    const parsed = Number(numeric);
    return Number.isFinite(parsed) ? parsed : null;
  };

  const decisionContext = useMemo(() => {
    const runway = Number(estimatedMonths);
    const runwayText = Number.isFinite(runway) ? `${runway}개월` : "∞";
    const riskTitle = risk?.title || "리스크 분석 중";
    const riskLevel = risk?.level;
    const penalty = risk?.estimated_penalty || 0;
    const keyStat = effectiveDashboardData?.stats?.[0];
    const cashStat = (effectiveDashboardData?.stats || []).find((stat: any) =>
      /현금|잔액|cash/i.test(stat.title || "")
    );
    const cashValue = parseStatValue(cashStat?.value);

    let actionKey = "stability";
    let summary = "이번 주는 재무 안정성 유지가 우선입니다.";
    let relatedTab: "home" | "accounting" | "management" = "home";
    if (riskLevel === "critical") {
      actionKey = "tax-risk";
      summary = "이번 주는 세무 리스크 대응을 최우선으로 진행하세요.";
      relatedTab = "accounting";
    } else if (Number.isFinite(runway) && runway < 6) {
      actionKey = "hiring-freeze";
      summary = "이번 달 채용은 보류하는 것이 합리적입니다.";
      relatedTab = "management";
    } else if (riskLevel === "warning") {
      actionKey = "cost-control";
      summary = "단기 현금 흐름을 기준으로 비용 통제를 강화하세요.";
      relatedTab = "management";
    }

    const reasons = [
      risk ? `세무 리스크: ${riskTitle}` : null,
      Number.isFinite(runway) ? `Runway ${runwayText}` : null,
      keyStat ? `${keyStat.title}: ${keyStat.value}` : null,
      cashValue !== null ? `현금성 자산: ${cashValue.toLocaleString()}원` : null,
    ].filter(Boolean) as string[];

    const impact = penalty > 0
      ? `추정 세액 리스크 ${penalty.toLocaleString()}원`
      : Number.isFinite(runway)
        ? `현재 Burn 기준 ${runwayText} 내 현금 압박 가능성`
        : "현금 흐름 변동 가능성 ↑";

    const scoreFromRisk = riskLevel === "critical" ? 70 : riskLevel === "warning" ? 45 : 15;
    const riskScore = risk?.score ?? scoreFromRisk;
    const scoreFromRunway = Number.isFinite(runway) ? (runway < 3 ? 55 : runway < 6 ? 30 : 10) : 15;
    const scoreFromPenalty = penalty > 0 ? Math.min(30, Math.floor(penalty / 10000000)) : 0;
    const scoreFromCash = cashValue !== null && cashValue < 50000000 ? 15 : 0;
    const priorityScore = Math.min(100, scoreFromRisk + scoreFromRunway + scoreFromPenalty + scoreFromCash);
    const drivers = [
      { label: "리스크", score: scoreFromRisk },
      { label: "런웨이", score: scoreFromRunway },
      { label: "세액", score: scoreFromPenalty },
      { label: "현금", score: scoreFromCash },
    ].filter((driver) => driver.score > 0);

    return {
      id: `decision-${actionKey}`,
      summary,
      reasons: reasons.length > 0 ? reasons.slice(0, 3) : ["재무 지표 안정성 점검 필요"],
      impact,
      actionKey,
      priorityScore,
      runwayMonths: Number.isFinite(runway) ? runway : undefined,
      riskScore,
      drivers,
      relatedTab,
    };
  }, [risk, effectiveDashboardData, estimatedMonths]);

  const similarOutcomes = useMemo(() => {
    const sameId = decisionHistory.filter((record) => record.id === decisionContext.id);
    const sameAction = decisionHistory.filter((record) =>
      record.id !== decisionContext.id && record.id.includes(decisionContext.actionKey)
    );
    return [...sameId, ...sameAction]
      .filter((record) => record.outcomeStatus !== "pending")
      .slice(0, 3);
  }, [decisionHistory, decisionContext.id, decisionContext.actionKey]);

  const hasNegativeOutcome = useMemo(() => {
    return similarOutcomes.some((record) => record.outcomeStatus === "negative");
  }, [similarOutcomes]);

  const negativeOutcomeTarget = useMemo(() => {
    return similarOutcomes.find((record) => record.outcomeStatus === "negative") || null;
  }, [similarOutcomes]);



  const getSimilarityScore = useCallback((record: DecisionRecord) => {
    let score = 0;
    if (record.id.includes(decisionContext.actionKey)) score += 2;
    if (record.priorityScore !== undefined) {
      const diff = Math.abs(record.priorityScore - decisionContext.priorityScore);
      if (diff <= 10) score += 2 * similarityWeights.priority;
      else if (diff <= 25) score += 1 * similarityWeights.priority;
    }
    if (record.runwayMonths !== undefined && decisionContext.runwayMonths !== undefined) {
      const diff = Math.abs(record.runwayMonths - decisionContext.runwayMonths);
      if (diff <= 1) score += 2 * similarityWeights.runway;
      else if (diff <= 3) score += 1 * similarityWeights.runway;
    }
    if (record.riskScore !== undefined && decisionContext.riskScore !== undefined) {
      const diff = Math.abs(record.riskScore - decisionContext.riskScore);
      if (diff <= 10) score += 2 * similarityWeights.risk;
      else if (diff <= 25) score += 1 * similarityWeights.risk;
    }
    const reasonText = record.reasons.join(" ");
    const currentText = decisionContext.reasons.join(" ");
    const keywords = ["Runway", "세무", "현금"];
    const overlap = keywords.filter((keyword) => reasonText.includes(keyword) && currentText.includes(keyword)).length;
    score += overlap;
    return score;
  }, [
    decisionContext.actionKey,
    decisionContext.priorityScore,
    decisionContext.runwayMonths,
    decisionContext.riskScore,
    decisionContext.reasons,
    similarityWeights.priority,
    similarityWeights.runway,
    similarityWeights.risk,
  ]);

  const getSimilarityReason = useCallback((record: DecisionRecord) => {
    const reasons = [];
    if (record.id.includes(decisionContext.actionKey)) reasons.push("유사 결정");
    if (record.reasons.some((reason) => reason.includes("Runway"))) reasons.push("런웨이");
    if (record.reasons.some((reason) => reason.includes("세무"))) reasons.push("세무 리스크");
    if (record.reasons.some((reason) => reason.includes("현금"))) reasons.push("현금");
    return reasons.length > 0 ? reasons.join(" · ") : "참고 결정";
  }, [decisionContext.actionKey]);

  const filteredMemory = useMemo(() => {
    return decisionHistory.filter((record) => {
      if (!memoryFilterKey) return true;
      if (!record.id.includes(memoryFilterKey)) return false;
      if (!showSimilarOnly) return true;
      return getSimilarityScore(record) >= 2;
    });
  }, [decisionHistory, memoryFilterKey, showSimilarOnly, getSimilarityScore]);

  const filteredMemoryWithOutcome = useMemo(() => {
    if (outcomeFilter === "all") return filteredMemory;
    return filteredMemory.filter((record) => record.outcomeStatus === outcomeFilter);
  }, [filteredMemory, outcomeFilter]);

  const bestSimilarId = useMemo(() => {
    if (!showSimilarOnly || filteredMemoryWithOutcome.length === 0) return null;
    const sorted = [...filteredMemoryWithOutcome].sort((a, b) => getSimilarityScore(b) - getSimilarityScore(a));
    return sorted[0]?.id || null;
  }, [filteredMemoryWithOutcome, showSimilarOnly, getSimilarityScore]);

  const graphLayout = useMemo(() => {
    if (!memoryGraph || memoryGraph.nodes.length === 0) return null;
    const nodes = memoryGraph.nodes;
    const radius = 110;
    const center = 150;
    const positions = nodes.map((node, idx) => {
      const angle = (2 * Math.PI * idx) / nodes.length;
      return {
        id: node.id,
        x: center + radius * Math.cos(angle),
        y: center + radius * Math.sin(angle),
      };
    });
    const positionMap = positions.reduce<Record<string, { x: number; y: number }>>((acc, item) => {
      acc[item.id] = { x: item.x, y: item.y };
      return acc;
    }, {});
    return { positions, positionMap };
  }, [memoryGraph]);

  useEffect(() => {
    if (!graphLayout) return;
    setGraphPositions(graphLayout.positionMap);
  }, [graphLayout]);

  useEffect(() => {
    if (!user?.bizNum) return;
    const storageKey = `taxai_graph_positions_${user.bizNum}`;
    const saved = localStorage.getItem(storageKey);
    if (saved) {
      try {
        const parsed = JSON.parse(saved) as Record<string, { x: number; y: number }>;
        setGraphPositions(parsed);
      } catch (err) {
        console.error("Failed to load graph positions:", err);
      }
    }
  }, [user]);

  useEffect(() => {
    if (!user?.bizNum) return;
    const storageKey = `taxai_graph_positions_${user.bizNum}`;
    localStorage.setItem(storageKey, JSON.stringify(graphPositions));
  }, [graphPositions, user]);

  const warningTone = useMemo(() => {
    if (!hasNegativeOutcome) return "none";
    if (!negativeOutcomeTarget) return "none";
    const score = getSimilarityScore(negativeOutcomeTarget);
    if (score >= warningThreshold + 1) return "strong";
    if (score >= warningThreshold) return "soft";
    return "none";
  }, [hasNegativeOutcome, negativeOutcomeTarget, getSimilarityScore, warningThreshold]);

  const warningMessage = useMemo(() => {
    if (warningTone === "none") return "";
    if (tonePreference === "direct") {
      return warningTone === "strong"
        ? "과거 동일 판단에서 부정 결과가 발생했습니다. 이 결정은 재검토가 필요합니다."
        : "유사 판단에서 부정 결과가 있었습니다. 신중히 접근하세요.";
    }
    if (tonePreference === "soft") {
      return warningTone === "strong"
        ? "이전에 비슷한 판단에서 어려움이 있었습니다. 다시 살펴보는 것을 권장합니다."
        : "유사한 판단에서 아쉬운 결과가 있었습니다. 참고해 주세요.";
    }
    return warningTone === "strong"
      ? "과거 동일 판단에서 부정 결과가 있었습니다. 재검토 권장."
      : "유사 판단에서 부정 결과가 있었습니다. 유의하세요.";
  }, [warningTone, tonePreference]);

  // mapMemoryToDecision is already defined earlier in the component

  const mapDecisionToMemory = (record: DecisionRecord, bizNum: string): MemoryRecord => ({
    id: record.id,
    biz_num: bizNum,
    title: record.title,
    summary: record.summary,
    status: record.status,
    created_at: record.createdAt,
    reasons: record.reasons,
    impact: record.impact,
    outcome_status: record.outcomeStatus,
    outcome_memo: record.outcomeMemo,
    priority_score: record.priorityScore,
    runway_months: record.runwayMonths,
    risk_score: record.riskScore,
    rejection_reason: record.rejectionReason,
  });

  const recordDecision = (status: "accepted" | "rejected", rejectionReason?: string) => {
    const now = new Date().toISOString();
    // Unique ID generation to allow history accumulation
    const uniqueId = `${decisionContext.id}-${Date.now()}`;

    const next: DecisionRecord = {
      id: uniqueId,
      title: decisionContext.summary,
      summary: decisionContext.impact,
      status,
      createdAt: now,
      reasons: decisionContext.reasons,
      impact: decisionContext.impact,
      outcomeStatus: "pending",
      priorityScore: decisionContext.priorityScore,
      runwayMonths: decisionContext.runwayMonths,
      riskScore: decisionContext.riskScore,
      rejectionReason,
    };

    setDecisionHistory((prev) => [next, ...prev]);

    if (user?.bizNum) {
      api.saveDecisionMemory(mapDecisionToMemory(next, user.bizNum)).catch((err) => {
        console.error("Failed to save decision memory:", err);
      });
    }
  };

  const buildLocalAnswer = (prompt: string) => {
    const buildConclusion = (summary: string) => {
      if (tonePreference === "direct") {
        return `결론: ${summary} 즉시 대응이 필요합니다.`;
      }
      if (tonePreference === "soft") {
        return `결론: ${summary} 참고 부탁드립니다.`;
      }
      return `결론: ${summary}`;
    };

    const currentBurn = runwayBurn;
    const currentCash = runwayCash;
    const nextBurn = Math.round(currentBurn * 1.15);
    const nextRunway = nextBurn > 0 ? (currentCash / nextBurn).toFixed(1) : "∞";
    const simulationText = `시뮬레이션(간단): 월 Burn이 15% 증가하면 Runway가 약 ${nextRunway}개월로 감소합니다.`;

    if (/시뮬|simulation/i.test(prompt)) {
      return `${buildConclusion(decisionContext.summary)}\n\n${simulationText}\n\n현재 Burn: ${currentBurn.toLocaleString()}원 → 예상 Burn: ${nextBurn.toLocaleString()}원`;
    }
    if (/근거|이유|why/i.test(prompt)) {
      const reasons = summaryLength === "short"
        ? decisionContext.reasons.slice(0, 1)
        : decisionContext.reasons;
      return `${buildConclusion(decisionContext.summary)}\n\n근거:\n- ${reasons.join("\n- ")}`;
    }
    if (/자세히|detail/i.test(prompt)) {
      return `요약: ${decisionContext.summary}\n\n근거:\n- ${decisionContext.reasons.join("\n- ")}\n\n영향: ${decisionContext.impact}`;
    }
    if (summaryLength === "detailed") {
      return `${buildConclusion(decisionContext.summary)}\n\n근거:\n- ${decisionContext.reasons.join("\n- ")}\n\n영향: ${decisionContext.impact}`;
    }
    if (summaryLength === "short") {
      return `${buildConclusion(decisionContext.summary)}`;
    }
    return `${buildConclusion(decisionContext.summary)}\n근거: ${decisionContext.reasons[0] || "추가 지표를 확인 중입니다."}`;
  };

  const openDecisionAction = (action: "details" | "simulation") => {
    const targetTab = action === "simulation" ? "management" : decisionContext.relatedTab;
    setActiveTab("home");
    setIsDetailOpen(true);
    setHighlightKey(null);
    if (action === "details") {
      setShowDecisionDetails(true);
      setDashboardTab(targetTab);
      setHighlightKey(targetTab);
    }
    if (action === "simulation") {
      setShowDecisionSimulation(true);
      setDashboardTab(targetTab);
      setHighlightKey(targetTab);
    }
    setTimeout(() => {
      if (targetTab === "accounting") {
        accountingRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
      } else if (targetTab === "management") {
        managementRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
      } else {
        dashboardRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
      }
    }, 100);
  };

  const openRelatedScreen = () => {
    setActiveTab("home");
    setIsDetailOpen(true);
    setHighlightKey(null);
    setDashboardTab(decisionContext.relatedTab);
    setHighlightKey(decisionContext.relatedTab);
    setTimeout(() => {
      if (decisionContext.relatedTab === "accounting") {
        accountingRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
      } else if (decisionContext.relatedTab === "management") {
        managementRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
      } else {
        dashboardRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
      }
    }, 100);
  };

  const handleSend = async (overrideText?: string) => {
    if (isLoading) return;
    const messageText = (overrideText ?? input).trim();
    if (!messageText) return;
    const userMsg: ChatMessage = { role: "user", content: messageText };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");
    setIsLoading(true);
    loadingStartRef.current = performance.now();

    const prompt = userMsg.content.trim();
    if (/자세히|근거|이유|시뮬|simulation|detail/i.test(prompt)) {
      if (/근거|이유/i.test(prompt)) {
        openDecisionAction("details");
      }
      if (/시뮬|simulation/i.test(prompt)) {
        openDecisionAction("simulation");
      }
      const localReply: ChatMessage = {
        role: "assistant",
        content: buildLocalAnswer(prompt),
      };
      setMessages((prev) => [...prev, localReply]);
      setIsLoading(false);
      return;
    }

    try {
      const history = messages.map(({ role, content }) => ({ role, content }));

      // Add empty assistant message first for streaming
      setMessages((prev) => [...prev, { role: "assistant", content: "" }]);

      // Use streaming API
      api.chatStream(
        userMsg.content,
        history,
        user?.bizNum,
        (chunk: string) => {
          // Update the last message with new chunk
          setMessages((prev) => {
            const updated = [...prev];
            const lastMsg = updated[updated.length - 1];
            if (lastMsg && lastMsg.role === "assistant") {
              updated[updated.length - 1] = {
                ...lastMsg,
                content: lastMsg.content + chunk
              };
            }
            return updated;
          });
        },
        () => {
          // Done streaming
          const elapsed = loadingStartRef.current ? performance.now() - loadingStartRef.current : 0;
          setMessages((prev) => {
            const updated = [...prev];
            const lastMsg = updated[updated.length - 1];
            if (lastMsg && lastMsg.role === "assistant") {
              updated[updated.length - 1] = {
                ...lastMsg,
                latencyMs: Math.round(elapsed)
              };
            }
            return updated;
          });
          setIsLoading(false);
        }
      );
    } catch (error) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: "죄송합니다. 오류가 발생했습니다." },
      ]);
      setIsLoading(false);
    }
  };

  // Auth state
  const [isAuthenticated, setIsAuthenticated] = useState<boolean>(() => {
    return !!localStorage.getItem("token");
  });
  const [authUser, setAuthUser] = useState<any>(() => {
    const stored = localStorage.getItem("user");
    return stored ? JSON.parse(stored) : null;
  });

  const handleAuthSuccess = (authUserData: any, _token: string) => {
    setAuthUser(authUserData);
    setIsAuthenticated(true);
    // If onboarding already completed, set user state
    if (authUserData.onboarding_completed && authUserData.biz_num) {
      setUser({
        name: authUserData.name,
        company: authUserData.company,
        bizNum: authUserData.biz_num,
        type: authUserData.type || "general",
        activeMCPs: [authUserData.type || "general"],
        rfiData: {}
      });
    }
  };

  // Restore user state on mount if authUser exists
  useEffect(() => {
    if (authUser && authUser.onboarding_completed && authUser.biz_num && !user) {
      setUser({
        name: authUser.name,
        company: authUser.company,
        bizNum: authUser.biz_num,
        type: authUser.type || "general",
        activeMCPs: [authUser.type || "general"],
        rfiData: {}
      });
    }
  }, [authUser, user]);

  const handleLogout = () => {
    localStorage.removeItem("token");
    localStorage.removeItem("user");
    setIsAuthenticated(false);
    setAuthUser(null);
    setUser(null);
  };

  // If not authenticated, show LoginPage
  if (!isAuthenticated) {
    return <LoginPage onSuccess={handleAuthSuccess} />;
  }

  // Handler for onboarding completion
  const handleOnboardingComplete = async (name: string, company: string, bizNum: string, type: string, targetRevenue?: number) => {
    // Save to backend
    if (authUser?.email) {
      try {
        const result = await api.completeOnboarding(authUser.email, bizNum, type, targetRevenue);
        if (result.success) {
          localStorage.setItem("user", JSON.stringify(result.user));
          setAuthUser(result.user);
        }
      } catch (e) {
        console.error("Failed to save onboarding data:", e);
      }
    }

    // Set local user state
    setUser({
      name: authUser?.name || name,
      company: authUser?.company || company,
      bizNum,
      type,
      targetRevenue,
      activeMCPs: [type],
      rfiData: {}
    });
  };

  // If authenticated but no user data (needs onboarding), show Onboarding
  if (!user) {
    return <Onboarding onStart={handleOnboardingComplete} />;
  }

  const pageTitle = {
    home: "AI CFO 브리핑",
    risk: "세무 리스크",
    accounting: "회계/증빙 관리",
    runway: "Runway / Burn",
    competitions: "R&D / 정부지원",
    hospital_claims: "보험 청구 심사",
    hospital_pnl: "진료과별 손익",
    commerce_roas: "ROAS / 마케팅",
    commerce_inventory: "재고 / 정산",
    memory: "결정 히스토리",
    settings: "My Page & Settings",
  }[activeTab] || "AI CFO OS";

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
            {(user.activeMCPs || []).includes('startup') ? 'Startup Ed.' : (user.activeMCPs || []).includes('hospital') ? 'Medi-Tech Ed.' : (user.activeMCPs || []).includes('commerce') ? 'Commerce Ed.' : 'Standard Ed.'}
          </div>
        </div>

        <nav className="flex-1 p-4 space-y-6 overflow-y-auto">
          {/* HOME */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 px-3 tracking-wider">HOME</div>
            <div className="space-y-1">
              <SidebarItem icon={LayoutDashboard} label="AI CFO 브리핑" active={activeTab === "home"} onClick={() => setActiveTab("home")} />
            </div>
          </div>

          {/* DETECT */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 px-3 tracking-wider">DETECT</div>
            <div className="space-y-1">
              <SidebarItem icon={AlertTriangle} label="세무 리스크" active={activeTab === "risk"} onClick={() => setActiveTab("risk")} />
              <SidebarItem icon={Calculator} label="회계/증빙 관리" active={activeTab === "accounting"} onClick={() => setActiveTab("accounting")} />
            </div>
          </div>

          {/* DECIDE */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 px-3 tracking-wider">DECIDE</div>
            <div className="space-y-1">
              {(user.activeMCPs || []).includes('startup') && (
                <>
                  <SidebarItem icon={TrendingUp} label="Runway / Burn" active={activeTab === "runway"} onClick={() => setActiveTab("runway")} />
                  <SidebarItem icon={Rocket} label="R&D / 정부지원" active={activeTab === "competitions"} onClick={() => setActiveTab("competitions")} />
                </>
              )}
            </div>
          </div>

          {/* EXPLAIN */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 px-3 tracking-wider">EXPLAIN</div>
            <div className="space-y-1">
              <SidebarItem
                icon={Activity}
                label="재무 근거 대시보드"
                active={activeTab === "home" && isDetailOpen && dashboardTab === "home"}
                onClick={() => {
                  setActiveTab("home");
                  setIsDetailOpen(true);
                  setDashboardTab("home");
                }}
              />
              {(user.activeMCPs || []).includes('hospital') && (
                <>
                  <SidebarItem icon={Stethoscope} label="보험 청구 심사" active={activeTab === "hospital_claims"} onClick={() => setActiveTab("hospital_claims")} />
                  <SidebarItem icon={Activity} label="진료과별 손익" active={activeTab === "hospital_pnl"} onClick={() => setActiveTab("hospital_pnl")} />
                </>
              )}
              {(user.activeMCPs || []).includes('commerce') && (
                <>
                  <SidebarItem icon={ShoppingBag} label="ROAS / 마케팅" active={activeTab === "commerce_roas"} onClick={() => setActiveTab("commerce_roas")} />
                  <SidebarItem icon={Building2} label="재고 / 정산" active={activeTab === "commerce_inventory"} onClick={() => setActiveTab("commerce_inventory")} />
                </>
              )}
            </div>
          </div>

          {/* MEMORY */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 px-3 tracking-wider">MEMORY</div>
            <div className="space-y-1">
              <SidebarItem icon={FileText} label="결정 히스토리" active={activeTab === "memory"} onClick={() => setActiveTab("memory")} />
            </div>
          </div>
        </nav>

        <div className="p-4 border-t border-sidebar-border space-y-2">
          <div
            onClick={() => setActiveTab("settings")}
            className={`flex items-center gap-3 px-3 py-2 rounded-lg cursor-pointer transition-colors ${activeTab === 'settings' ? 'bg-sidebar-accent' : 'hover:bg-sidebar-accent/50'}`}
          >
            <div className="w-8 h-8 rounded-full bg-sidebar-accent flex items-center justify-center">
              <User className="w-4 h-4" />
            </div>
            <div className="text-sm">
              <div className="font-medium">{user.name} 대표</div>
              <div className="text-xs text-muted-foreground">My Page & Settings</div>
            </div>
          </div>
          <button
            onClick={handleLogout}
            className="w-full flex items-center gap-3 px-3 py-2 rounded-lg cursor-pointer transition-colors hover:bg-red-500/10 text-red-500 text-sm"
          >
            <LogOut className="w-4 h-4" />
            <span>로그아웃</span>
          </button>
        </div>
      </aside>

      {/* Main Content */}
      <main className="flex-1 flex flex-col min-w-0 bg-background/50">
        {/* Header */}
        <header className="h-16 border-b bg-background/80 backdrop-blur-sm sticky top-0 z-10 px-6 flex items-center justify-between">
          <button onClick={() => setIsSidebarOpen(!isSidebarOpen)} className="p-2 hover:bg-accent rounded-md md:hidden"><Menu className="w-5 h-5" /></button>
          <h1 className="font-semibold text-lg opacity-80 pl-4 md:pl-0">
            {pageTitle}
          </h1>
          <div className="flex items-center gap-4 relative">
            <button
              onClick={() => setIsQuestionOpen(true)}
              className="p-2 hover:bg-accent rounded-full transition-colors"
              aria-label="질문 패널 열기"
            >
              <MessageSquare className="w-5 h-5 text-muted-foreground" />
            </button>
            {isSilent && (
              <span className="text-xs font-medium text-orange-600 bg-orange-100 px-2 py-1 rounded-full">
                SILENT
              </span>
            )}
            <button
              onClick={() => setShowNotifications(!showNotifications)}
              className="p-2 hover:bg-accent rounded-full relative transition-colors"
            >
              <Bell className="w-5 h-5 text-muted-foreground" />
              {calendarAlerts.length > 0 && <span className="absolute top-2 right-2 w-2 h-2 bg-red-500 rounded-full animate-ping" />}
              {calendarAlerts.length > 0 && <span className="absolute top-2 right-2 w-2 h-2 bg-red-500 rounded-full" />}
            </button>

            {showNotifications && (
              <div className="absolute top-full right-0 mt-2 w-80 bg-white rounded-xl shadow-xl border overflow-hidden animate-in fade-in slide-in-from-top-2 z-50">
                <div className="p-3 border-b bg-gray-50 flex justify-between items-center">
                  <span className="font-bold text-sm">알림</span>
                  <button onClick={() => setCalendarAlerts([])} className="text-xs text-gray-500 hover:text-gray-800">모두 읽음</button>
                </div>
                <div className="max-h-80 overflow-y-auto">
                  {calendarAlerts.length > 0 ? calendarAlerts.map((alert, i) => (
                    <div key={i} className="p-3 border-b hover:bg-gray-50 flex gap-3">
                      <div className={`w-2 h-2 rounded-full mt-1.5 flex-shrink-0 ${alert.d_day <= 3 ? 'bg-red-500' : 'bg-blue-500'}`} />
                      <div>
                        <p className="text-sm font-medium">{alert.title}</p>
                        <p className="text-xs text-muted-foreground">{alert.date} ({alert.type === 'mandatory' ? '필수' : '권장'})</p>
                      </div>
                    </div>
                  )) : (
                    <div className="p-8 text-center text-gray-400 text-sm">새로운 알림이 없습니다.</div>
                  )}
                </div>
              </div>
            )}
          </div>
        </header>

        <div className="flex-1 overflow-auto p-6">

          {/* DASHBOARD VIEW */}
          {activeTab === "home" && !effectiveDashboardData && dataStatus !== "none" && dataStatus !== "partial" && (
            <div className="max-w-6xl mx-auto flex items-center justify-center py-20">
              <div className="text-center">
                <div className="mx-auto w-12 h-12 border-4 border-primary border-t-transparent rounded-full animate-spin mb-4" />
                <p className="text-muted-foreground">데이터를 불러오는 중...</p>
              </div>
            </div>
          )}
          {activeTab === "home" && (dataStatus === "none" || dataStatus === "partial") && !effectiveDashboardData && (
            <DashboardEmptyState status={dataStatus as "none" | "partial"} onSetup={() => setActiveTab("settings")} />
          )}
          {activeTab === "home" && effectiveDashboardData && (
            <div className="max-w-7xl mx-auto space-y-6">
              {/* A. Executive Dashboard (6 KPI) */}
              <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
                {effectiveDashboardData.stats?.map((stat: any, i: number) => (
                  <div key={i} className="p-5 bg-card rounded-xl border shadow-sm hover:shadow-md transition-shadow">
                    <div className="flex justify-between items-start mb-2">
                      {summaryLength !== "short" && (
                        <span className="text-sm font-medium text-muted-foreground">{stat.title}</span>
                      )}
                      <span className={`text-xs px-2 py-0.5 rounded-full ${stat.trend === 'up' ? 'bg-red-100 text-red-600' : 'bg-green-100 text-green-600'}`}>
                        {stat.trend === 'up' ? '▲' : '▼'} {stat.change}
                      </span>
                    </div>
                    <div className="text-2xl font-bold">{stat.value}</div>
                    {summaryLength !== "short" && (
                      <div className="text-xs text-muted-foreground mt-1">{stat.desc}</div>
                    )}
                  </div>
                ))}
              </div>

              {/* B. AI CFO 브리핑 & 결정 카드 */}
              <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                <div className="lg:col-span-2 space-y-6">
                  <div ref={decisionCardRef} className="p-6 bg-card rounded-xl border shadow-sm">
                    <div className="flex items-start justify-between gap-4">
                      <div>
                        <p className="text-xs text-muted-foreground mb-1">AI CFO 브리핑 · {new Date().getFullYear()}년 {new Date().getMonth() + 1}월</p>
                        <h2 className="text-xl font-bold">
                          {tonePreference === "direct"
                            ? "이번 주 결론"
                            : tonePreference === "soft"
                              ? "이번 주 참고 요약"
                              : "이번 주 핵심 요약"}
                        </h2>
                      </div>
                      <button
                        onClick={() => setIsQuestionOpen(true)}
                        className="text-xs px-3 py-1 rounded-full border hover:bg-muted transition-colors"
                      >
                        질문하기
                      </button>
                    </div>

                    {isSilent ? (
                      <div className="mt-4 p-4 rounded-lg bg-orange-50 border border-orange-100 text-sm text-orange-700">
                        AI CFO가 침묵 모드입니다. 필요하면 다시 호출해 주세요.
                        <button
                          onClick={() => setIsSilent(false)}
                          className="ml-3 text-orange-700 underline"
                        >
                          다시 말해줘
                        </button>
                      </div>
                    ) : (
                      <div className="mt-4 space-y-4">
                        <div className={`grid grid-cols-1 ${summaryLength === "short" ? "md:grid-cols-2" : "md:grid-cols-3"} gap-4 text-sm`}>
                          <div className="p-4 bg-muted/40 rounded-lg">
                            <p className="text-xs text-muted-foreground mb-1">핵심 리스크</p>
                            <p className="font-semibold">
                              {summaryLength === "short"
                                ? (risk?.title || "리스크 분석 중").split(" ").slice(0, 4).join(" ")
                                : (risk?.title || "리스크 분석 중")}
                            </p>
                          </div>
                          <div className="p-4 bg-muted/40 rounded-lg">
                            <p className="text-xs text-muted-foreground mb-1">지금 내려야 할 결정</p>
                            <p className="font-semibold">
                              {summaryLength === "short"
                                ? decisionContext.summary.split(" ").slice(0, 6).join(" ")
                                : decisionContext.summary}
                            </p>
                          </div>
                          {summaryLength !== "short" && (
                            <div className="p-4 bg-muted/40 rounded-lg">
                              <p className="text-xs text-muted-foreground mb-1">무시했을 때 결과</p>
                              <p className="font-semibold">
                                {summaryLength === "normal"
                                  ? decisionContext.impact.split(".")[0]
                                  : decisionContext.impact}
                              </p>
                            </div>
                          )}
                        </div>
                        {similarOutcomes.length > 0 && (
                          <div className="p-4 rounded-lg border bg-white text-sm">
                            <div className="flex items-center justify-between mb-2">
                              <p className="text-xs text-muted-foreground">과거 유사 결정 결과</p>
                              <button
                                onClick={() => {
                                  setMemoryFilterKey(decisionContext.actionKey);
                                  setActiveTab("memory");
                                }}
                                className="text-[11px] px-2 py-1 rounded-full border hover:bg-muted"
                              >
                                전체 보기
                              </button>
                            </div>
                            {warningTone !== "none" && (
                              <div className={`mb-3 rounded-lg px-3 py-2 text-xs ${warningTone === "strong" ? "bg-red-50 text-red-700 border border-red-100" : "bg-yellow-50 text-yellow-700 border border-yellow-100"}`}>
                                {warningMessage}
                              </div>
                            )}
                            {summaryLength === "short" && (
                              <button
                                onClick={() => setShowSimilarOutcomes((prev) => !prev)}
                                className="text-xs px-2 py-1 rounded-full border hover:bg-muted"
                                aria-label={showSimilarOutcomes ? "유사 결과 접기" : "유사 결과 펼치기"}
                              >
                                {showSimilarOutcomes ? "접기" : "펼치기"}
                              </button>
                            )}
                            {(summaryLength !== "short" || showSimilarOutcomes) && (
                              <div className="space-y-2 mt-2">
                                {similarOutcomes.map((record) => (
                                  <div key={record.createdAt} className="flex items-center justify-between">
                                    <div>
                                      <p className="text-sm font-medium">
                                        {summaryLength === "short"
                                          ? record.title.split(" ").slice(0, 5).join(" ")
                                          : record.title}
                                      </p>
                                      <p className="text-xs text-muted-foreground">
                                        {new Date(record.createdAt).toLocaleDateString()} · {summaryLength === "short"
                                          ? record.summary.split(".")[0]
                                          : record.summary}
                                      </p>
                                      <p className="text-[11px] text-muted-foreground">
                                        {getSimilarityReason(record)}
                                      </p>
                                    </div>
                                    <span className={`text-xs px-2 py-1 rounded-full ${record.outcomeStatus === "positive"
                                      ? "bg-emerald-100 text-emerald-700"
                                      : record.outcomeStatus === "negative"
                                        ? "bg-red-100 text-red-700"
                                        : "bg-gray-100 text-gray-600"
                                      }`}>
                                      {record.outcomeStatus === "positive" ? "긍정" : record.outcomeStatus === "negative" ? "부정" : "중립"}
                                    </span>
                                  </div>
                                ))}
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  <div className="p-6 bg-card rounded-xl border shadow-sm">
                    <div className="flex items-start justify-between gap-4">
                      <div>
                        <p className="text-xs text-muted-foreground mb-1">AI CFO 공식 판단</p>
                        <div className="flex items-center gap-2">
                          <h3 className="text-lg font-bold">
                            {summaryLength === "short"
                              ? decisionContext.summary.split(" ").slice(0, 6).join(" ")
                              : decisionContext.summary}
                          </h3>
                          {hasNegativeOutcome && (
                            <button
                              onClick={() => {
                                if (!negativeOutcomeTarget) return;
                                setMemoryHighlightId(negativeOutcomeTarget.id);
                                setActiveTab("memory");
                              }}
                              className={`text-[11px] px-2 py-1 rounded-full transition-colors ${hasNegativeOutcome && bestSimilarId === negativeOutcomeTarget?.id
                                ? "bg-red-200 text-red-800"
                                : "bg-red-100 text-red-700 hover:bg-red-200"
                                }`}
                            >
                              과거 부정 결과
                            </button>
                          )}
                        </div>
                      </div>
                      <div className="text-xs text-muted-foreground text-right">
                        <div>{summaryLength === "short" ? "결정 #1" : "결정 카드 #1"}</div>
                        <div className="mt-1 inline-flex items-center gap-2 px-2 py-1 rounded-full bg-muted text-[11px] text-muted-foreground">
                          우선순위 {decisionContext.priorityScore}/100
                        </div>
                      </div>
                    </div>

                    {isSilent ? (
                      <div className="mt-4 p-4 rounded-lg bg-orange-50 border border-orange-100 text-sm text-orange-700">
                        침묵 모드에서는 판단을 제시하지 않습니다.
                        <button
                          onClick={() => setIsSilent(false)}
                          className="ml-3 text-orange-700 underline"
                        >
                          다시 말해줘
                        </button>
                      </div>
                    ) : (
                      <>
                        <div className="mt-4 space-y-2 text-sm text-muted-foreground">
                          {(summaryLength === "short" ? decisionContext.reasons.slice(0, 1) : decisionContext.reasons).map((reason) => (
                            <p key={reason}>• {reason}</p>
                          ))}
                        </div>
                        {decisionContext.drivers.length > 0 && (
                          <div className="mt-3 flex flex-wrap gap-2 text-xs">
                            {decisionContext.drivers.map((driver) => (
                              <span
                                key={driver.label}
                                className="px-2 py-1 rounded-full bg-muted text-muted-foreground"
                              >
                                {driver.label} +{driver.score}
                              </span>
                            ))}
                          </div>
                        )}

                        <div className="mt-4 flex flex-wrap gap-2">
                          <button
                            onClick={() => recordDecision("accepted")}
                            className="px-3 py-2 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90"
                            title="이 판단 따르기"
                            aria-label="이 판단 따르기"
                          >
                            {summaryLength === "short"
                              ? "▶"
                              : tonePreference === "direct"
                                ? "지금 실행"
                                : tonePreference === "soft"
                                  ? "검토 후 진행"
                                  : "이 판단 따르기"}
                          </button>
                          <button
                            onClick={() => {
                              setRejectionInput("");
                              setShowRejectionModal(true);
                            }}
                            className="px-3 py-2 rounded-lg border text-sm hover:bg-muted"
                            title="이 판단 거부"
                            aria-label="이 판단 거부"
                          >
                            {summaryLength === "short"
                              ? "✕"
                              : tonePreference === "direct"
                                ? "거부"
                                : tonePreference === "soft"
                                  ? "다른 선택 고려"
                                  : "이 판단 거부"}
                          </button>
                          <button
                            onClick={() => setShowDecisionDetails((prev) => !prev)}
                            className="px-3 py-2 rounded-lg border text-sm hover:bg-muted"
                            title="근거 보기"
                            aria-label="근거 보기"
                          >
                            {summaryLength === "short" ? "ⓘ" : "근거 보기"}
                          </button>
                          <button
                            onClick={() => setShowDecisionSimulation((prev) => !prev)}
                            className="px-3 py-2 rounded-lg border text-sm hover:bg-muted"
                            title="시뮬레이션"
                            aria-label="시뮬레이션"
                          >
                            {summaryLength === "short" ? "∑" : "시뮬레이션"}
                          </button>
                          <button
                            onClick={() => setIsSilent(true)}
                            className="px-3 py-2 rounded-lg border text-sm text-orange-600 border-orange-200 hover:bg-orange-50"
                            title="그만"
                            aria-label="그만"
                          >
                            {summaryLength === "short" ? "⏸" : "그만"}
                          </button>
                        </div>

                        {showDecisionDetails && (
                          <div className="mt-4 p-4 rounded-lg bg-muted/30 text-sm">
                            <p className="font-semibold mb-2">근거 상세</p>
                            <ul className="space-y-1 text-muted-foreground">
                              {decisionContext.reasons.map((reason) => (
                                <li key={reason}>{reason}</li>
                              ))}
                            </ul>
                          </div>
                        )}

                        {showDecisionSimulation && (
                          <div className="mt-4 p-4 rounded-lg bg-muted/30 text-sm">
                            <p className="font-semibold mb-2">시뮬레이션</p>
                            <p className="text-muted-foreground">
                              월 Burn 15% 증가 기준, 현재 {runwayCash.toLocaleString()}원 보유 시 Runway가 약 {(runwayCash / (runwayBurn * 1.15)).toFixed(1)}개월로 감소합니다.
                            </p>
                          </div>
                        )}
                      </>
                    )}
                  </div>
                </div>

                <div className="lg:col-span-1">
                  {risk && risk.action_items && (
                    summaryLength === "short" ? (
                      <div className="p-4 bg-card rounded-xl border shadow-sm">
                        <p className="text-xs text-muted-foreground mb-1">리스크 요약</p>
                        <p className="font-semibold">{risk.title}</p>
                        <p className="text-sm text-muted-foreground mt-2">{risk.reason}</p>
                        <div className="mt-3 text-xs text-muted-foreground">
                          대응 항목 {risk.action_items.length}건 · 예상 리스크 {risk.estimated_penalty.toLocaleString()}원
                        </div>
                      </div>
                    ) : (
                      <RiskCard risk={risk} />
                    )
                  )}
                </div>
              </div>

              {/* C. 상세 대시보드 (접힘/펼침) */}
              <div className="bg-card rounded-xl border shadow-sm overflow-hidden">
                <button
                  onClick={() => setIsDetailOpen((prev) => !prev)}
                  className="w-full flex items-center justify-between px-6 py-4 text-sm font-medium hover:bg-muted/40"
                >
                  <span>{isDetailOpen ? "▲ 상세 재무·세무 대시보드 접기" : "▼ 상세 재무·세무 대시보드 펼치기"}</span>
                  <ChevronRight className={`w-4 h-4 transition-transform ${isDetailOpen ? "rotate-90" : ""}`} />
                </button>

                {isDetailOpen && (
                  <div className="p-6 border-t space-y-6 animate-in fade-in slide-in-from-top-2 duration-200">
                    <div className="flex flex-wrap gap-2">
                      <button
                        onClick={() => setDashboardTab("home")}
                        className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${dashboardTab === "home" ? "bg-primary/10 text-primary border border-primary/20" : "text-muted-foreground hover:bg-muted"
                          }`}
                      >
                        <LayoutDashboard className="w-4 h-4" />
                        대시보드
                      </button>
                      <button
                        onClick={() => setDashboardTab("accounting")}
                        className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${dashboardTab === "accounting" ? "bg-primary/10 text-primary border border-primary/20" : "text-muted-foreground hover:bg-muted"
                          }`}
                      >
                        <Calculator className="w-4 h-4" />
                        세무/회계
                      </button>
                      <button
                        onClick={() => setDashboardTab("management")}
                        className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${dashboardTab === "management" ? "bg-primary/10 text-primary border border-primary/20" : "text-muted-foreground hover:bg-muted"
                          }`}
                      >
                        <Building className="w-4 h-4" />
                        경영 지원
                      </button>
                    </div>

                    {dashboardTab === "home" && (
                      <div ref={dashboardRef} className={`space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500 ${highlightKey === "dashboard" ? "ring-2 ring-primary/40 rounded-xl" : ""}`}>
                        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                          <div className="lg:col-span-2 p-6 bg-card rounded-xl border shadow-sm flex flex-col h-auto min-h-[400px] lg:h-full">
                            <h3 className="font-bold flex items-center gap-2 mb-6">
                              <Activity className="w-5 h-5 text-primary" />
                              재무 트렌드 (2026)
                            </h3>
                            <ResponsiveContainer width="100%" height="100%">
                              <AreaChart data={effectiveDashboardData.chart || []}>
                                <defs>
                                  <linearGradient id="colorIncome" x1="0" y1="0" x2="0" y2="1">
                                    <stop offset="5%" stopColor="hsl(var(--primary))" stopOpacity={0.1} />
                                    <stop offset="95%" stopColor="hsl(var(--primary))" stopOpacity={0} />
                                  </linearGradient>
                                  <linearGradient id="colorExpense" x1="0" y1="0" x2="0" y2="1">
                                    <stop offset="5%" stopColor="#ef4444" stopOpacity={0.1} />
                                    <stop offset="95%" stopColor="#ef4444" stopOpacity={0} />
                                  </linearGradient>
                                </defs>
                                <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="#f0f0f0" />
                                <XAxis dataKey="name" axisLine={false} tickLine={false} tick={{ fill: '#888', fontSize: 12 }} dy={10} />
                                <YAxis axisLine={false} tickLine={false} tick={{ fill: '#888', fontSize: 12 }} tickFormatter={(value) => `${value / 10000}만`} />
                                <Tooltip
                                  contentStyle={{ borderRadius: '8px', border: 'none', boxShadow: '0 4px 12px rgba(0,0,0,0.1)' }}
                                  formatter={(value: any) => `${(value || 0).toLocaleString()}원`}
                                />
                                <Area type="monotone" dataKey="income" stroke="hsl(var(--primary))" fillOpacity={1} fill="url(#colorIncome)" strokeWidth={2} name="수입" />
                                <Area type="monotone" dataKey="expense" stroke="#ef4444" fillOpacity={1} fill="url(#colorExpense)" strokeWidth={2} name="지출" />
                              </AreaChart>
                            </ResponsiveContainer>
                          </div>

                          <div className="lg:col-span-1">
                            {risk && risk.action_items && <RiskCard risk={risk} />}
                          </div>
                        </div>

                        {(user.type === 'hospital' || user.activeMCPs.includes('hospital')) && (
                          <div className="p-6 bg-blue-50 rounded-xl border border-blue-100">
                            <div className="flex items-center gap-2 mb-2">
                              <Stethoscope className="w-5 h-5 text-blue-600" />
                              <h3 className="font-bold text-blue-800">병원 경영 리포트 요약</h3>
                            </div>
                            <div className="grid grid-cols-2 gap-4">
                              <div className="bg-white p-3 rounded-lg shadow-sm">
                                <p className="text-xs text-gray-500">보험 청구 삭감률</p>
                                <p className="text-lg font-bold text-red-500">2.4% <span className="text-xs font-normal text-gray-400">▼0.1%</span></p>
                              </div>
                              <div className="bg-white p-3 rounded-lg shadow-sm">
                                <p className="text-xs text-gray-500">비급여 매출 비중</p>
                                <p className="text-lg font-bold text-blue-600">35% <span className="text-xs font-normal text-gray-400">-</span></p>
                              </div>
                            </div>
                          </div>
                        )}

                        {(user.type === 'commerce' || user.activeMCPs.includes('commerce')) && (
                          <div className="p-6 bg-indigo-50 rounded-xl border border-indigo-100">
                            <div className="flex items-center gap-2 mb-2">
                              <ShoppingBag className="w-5 h-5 text-indigo-600" />
                              <h3 className="font-bold text-indigo-800">이커머스 현황 요약</h3>
                            </div>
                            <div className="grid grid-cols-2 gap-4">
                              <div className="bg-white p-3 rounded-lg shadow-sm">
                                <p className="text-xs text-gray-500">이번 달 ROAS</p>
                                <p className="text-lg font-bold text-indigo-600">340% <span className="text-xs font-normal text-gray-400">▲15%</span></p>
                              </div>
                              <div className="bg-white p-3 rounded-lg shadow-sm">
                                <p className="text-xs text-gray-500">재고 회전일</p>
                                <p className="text-lg font-bold text-green-600">14일 <span className="text-xs font-normal text-gray-400">빠름</span></p>
                              </div>
                            </div>
                          </div>
                        )}
                      </div>
                    )}

                    {dashboardTab === "accounting" && (
                      <div ref={accountingRef} className={`grid grid-cols-1 lg:grid-cols-2 gap-6 animate-in fade-in slide-in-from-bottom-4 duration-500 ${highlightKey === "accounting" ? "ring-2 ring-primary/40 rounded-xl p-2" : ""}`}>
                        <div className="col-span-1 space-y-6">
                          <TaxSimulator businessType={user?.type || 'startup'} />
                          <TaxCalendar alerts={calendarAlerts} />
                        </div>
                        <div className="col-span-1">
                          <FinancialAnalysis revenue={user?.targetRevenue || 150000000} industry={user?.type || 'startup'} />
                        </div>
                      </div>
                    )}

                    {dashboardTab === "management" && (
                      <div ref={managementRef} className={`grid grid-cols-1 xl:grid-cols-12 gap-6 animate-in fade-in slide-in-from-bottom-4 duration-500 ${highlightKey === "management" ? "ring-2 ring-primary/40 rounded-xl p-2" : ""}`}>
                        <div className="xl:col-span-5 space-y-6">
                          <BusinessLookup />
                          <div className="p-6 bg-card rounded-xl border shadow-sm border-dashed">
                            <h3 className="font-bold mb-2 flex items-center gap-2"><div className="w-2 h-2 bg-green-500 rounded-full" />스마트 문서 관리</h3>
                            <p className="text-sm text-muted-foreground mb-4">홈택스 PDF를 업로드하면 자동으로 분석합니다.</p>
                            <button
                              onClick={() => setActiveTab('settings')}
                              className="w-full py-2 bg-secondary text-secondary-foreground rounded-lg text-sm hover:bg-secondary/80"
                            >
                              문서 업로드 페이지로 이동
                            </button>
                          </div>
                        </div>
                        <div className="xl:col-span-7">
                          <div className="p-6 bg-card rounded-xl border shadow-sm h-full">
                            <h3 className="font-semibold text-lg flex items-center gap-2 mb-4">
                              <Rocket className="w-5 h-5 text-primary" />
                              2026년 맞춤 지원사업 예측
                            </h3>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                              {recommendations.map((rec, i) => (
                                <PredictionCard key={i} rec={rec} />
                              ))}
                            </div>
                          </div>

                          {/* Subsidies Section */}
                          {subsidies.length > 0 && (
                            <div className="p-6 bg-card rounded-xl border shadow-sm mt-6">
                              <h3 className="font-semibold text-lg flex items-center gap-2 mb-4">
                                <Coins className="w-5 h-5 text-green-600" />
                                정부 보조금 안내
                              </h3>
                              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                {subsidies.slice(0, 6).map((sub, i) => (
                                  <a
                                    key={i}
                                    href={sub.link}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="p-4 border rounded-lg hover:bg-muted/50 transition-colors"
                                  >
                                    <div className="font-medium text-sm mb-1">{sub.title}</div>
                                    <div className="text-xs text-muted-foreground mb-2">{sub.org}</div>
                                    <div className="text-xs text-primary">
                                      {sub.start_date} ~ {sub.end_date}
                                    </div>
                                    <div className="flex flex-wrap gap-1 mt-2">
                                      {sub.tags.slice(0, 3).map((tag, ti) => (
                                        <span key={ti} className="text-xs bg-secondary px-2 py-0.5 rounded-full">
                                          {tag}
                                        </span>
                                      ))}
                                    </div>
                                  </a>
                                ))}
                              </div>
                            </div>
                          )}
                        </div>
                      </div>
                    )}
                  </div>
                )}
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

              {showCompetitorCompare && (
                <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4 animate-in fade-in">
                  <div className="bg-white rounded-xl shadow-xl max-w-4xl w-full p-6">
                    <div className="flex justify-between items-center mb-6">
                      <h2 className="text-xl font-bold">🏢 경쟁사 심층 비교 분석</h2>
                      <button onClick={() => setShowCompetitorCompare(false)}><XCircle className="w-6 h-6 text-gray-400" /></button>
                    </div>
                    <div className="grid grid-cols-4 gap-4 text-sm">
                      <div className="col-span-1 bg-gray-50 p-4 rounded-lg font-bold">구분</div>
                      <div className="col-span-1 bg-blue-50 p-4 rounded-lg font-bold text-blue-700">우리 기업 (My)</div>
                      <div className="col-span-1 bg-white border p-4 rounded-lg font-bold">업계 평균</div>
                      <div className="col-span-1 bg-white border p-4 rounded-lg font-bold">Top 10% 리더</div>

                      <div className="col-span-1 p-3">연간 예상 매출</div>
                      <div className="col-span-1 p-3 font-bold text-blue-600">3.5억원</div>
                      <div className="col-span-1 p-3">2.8억원</div>
                      <div className="col-span-1 p-3 text-green-600">8.2억원</div>

                      <div className="col-span-1 p-3 border-t">직원 수</div>
                      <div className="col-span-1 p-3 border-t font-bold text-blue-600">5명</div>
                      <div className="col-span-1 p-3 border-t">4.2명</div>
                      <div className="col-span-1 p-3 border-t">12명</div>

                      <div className="col-span-1 p-3 border-t">평균 연봉</div>
                      <div className="col-span-1 p-3 border-t font-bold text-blue-600">4,200만원</div>
                      <div className="col-span-1 p-3 border-t">3,800만원</div>
                      <div className="col-span-1 p-3 border-t">5,500만원</div>

                      <div className="col-span-1 p-3 border-t">R&D 투자 비중</div>
                      <div className="col-span-1 p-3 border-t font-bold text-blue-600">15%</div>
                      <div className="col-span-1 p-3 border-t">8%</div>
                      <div className="col-span-1 p-3 border-t text-green-600">22%</div>
                    </div>
                    <div className="mt-6 p-4 bg-yellow-50 rounded-lg text-yellow-800 text-sm flex items-start gap-2">
                      <Rocket className="w-5 h-5 shrink-0" />
                      <p><strong>Insight:</strong> 우리 기업은 <strong>R&D 투자 비중</strong>이 업계 평균 대비 2배 가까이 높습니다. 이는 장기적인 기술 경쟁력 확보에 긍정적이나, 단기 현금 흐름 관리가 중요합니다.</p>
                    </div>
                  </div>
                </div>
              )}

              <div className="mt-6 text-center">
                <button
                  onClick={() => setShowCompetitorCompare(true)}
                  className="px-6 py-3 bg-white border border-gray-300 shadow-sm rounded-lg font-bold text-gray-700 hover:bg-gray-50 transition-all flex items-center gap-2 mx-auto"
                >
                  <Activity className="w-4 h-4" />
                  경쟁사 상세 비교표 보기
                </button>
              </div>
            </div>
          )}

          {/* RISK VIEW */}
          {activeTab === "risk" && (
            <div className="max-w-4xl mx-auto space-y-6">
              <div className="mb-4">
                <h2 className="text-2xl font-bold">세무 리스크 정밀 진단</h2>
                <p className="text-sm text-muted-foreground">AI가 분석한 사업장의 세무 리스크 현황입니다.</p>
              </div>
              {risk ? <RiskCard risk={risk} /> : <div className="p-8 text-center bg-gray-50 rounded-xl">리스크 데이터 로딩 중...</div>}
            </div>
          )}

          {/* ACCOUNTING VIEW */}
          {activeTab === "accounting" && (
            <div className="max-w-4xl mx-auto space-y-6">
              <div className="mb-4">
                <h2 className="text-2xl font-bold">회계 및 증빙 관리</h2>
                <p className="text-sm text-muted-foreground">간편하게 영수증을 업로드하고 증빙 누락을 방지하세요.</p>
              </div>

              {/* VAT ESTIMATOR */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
                <div className="bg-white p-6 rounded-xl border shadow-sm">
                  <div className="flex items-center gap-2 mb-4">
                    <div className="bg-blue-100 p-2 rounded-lg text-blue-600">
                      <Calculator className="w-5 h-5" />
                    </div>
                    <h3 className="font-bold text-lg">실시간 부가세(VAT) 계산기</h3>
                  </div>
                  <div className="space-y-4">
                    <div>
                      <label className="text-sm font-medium text-gray-500 mb-1 block">예상 매출액 (공급가액)</label>
                      <input
                        type="number"
                        className="w-full text-right p-3 bg-gray-50 border rounded-lg text-lg font-bold"
                        placeholder="0"
                        onChange={(e) => setVatRevenue(Number(e.target.value))}
                      />
                    </div>
                    <div>
                      <label className="text-sm font-medium text-gray-500 mb-1 block">예상 매입액 (공급가액)</label>
                      <input
                        type="number"
                        className="w-full text-right p-3 bg-gray-50 border rounded-lg text-lg font-bold"
                        placeholder="0"
                        onChange={(e) => setVatPurchase(Number(e.target.value))}
                      />
                    </div>
                    <div className="pt-4 border-t space-y-2">
                      <div className="flex justify-between text-sm text-gray-400">
                        <span>매출세액 (10%)</span>
                        <span>+ {(vatRevenue * 0.1).toLocaleString()}원</span>
                      </div>
                      <div className="flex justify-between text-sm text-gray-400">
                        <span>매입세액 (10%)</span>
                        <span>- {(vatPurchase * 0.1).toLocaleString()}원</span>
                      </div>

                      {/* Dynamic Deduction from Checklist */}
                      {deductionChecklist.filter(i => i.checked).length > 0 && (
                        <div className="flex justify-between text-sm text-emerald-600 animate-in fade-in slide-in-from-left-2">
                          <span>절세 공제 적용</span>
                          <span>- {deductionChecklist.reduce((acc, curr) =>
                            acc + (curr.checked ? (curr.id === 3 ? 1500000 : curr.id === 4 ? 200000 : 500000) : 0), 0
                          ).toLocaleString()}원</span>
                        </div>
                      )}

                      <div className="flex justify-between items-center pt-2 border-t">
                        <span className="font-bold text-gray-600">납부 예상 세액</span>
                        <span className={`text-2xl font-extrabold ${Math.max(0, (vatRevenue * 0.1) - (vatPurchase * 0.1) - deductionChecklist.reduce((acc, curr) => acc + (curr.checked ? (curr.id === 3 ? 1500000 : curr.id === 4 ? 200000 : 500000) : 0), 0)) > 0 ? 'text-red-600' : 'text-green-600'}`}>
                          {Math.max(0, (vatRevenue * 0.1) - (vatPurchase * 0.1) - deductionChecklist.reduce((acc, curr) =>
                            acc + (curr.checked ? (curr.id === 3 ? 1500000 : curr.id === 4 ? 200000 : 500000) : 0), 0
                          )).toLocaleString()}원
                        </span>
                      </div>
                    </div>
                    <p className="text-xs text-gray-400 text-right">* 대략적인 추산액이며 실제와 다를 수 있습니다.</p>
                  </div>
                </div>

                {/* DEDUCTION FINDER */}
                <div className="bg-white p-6 rounded-xl border shadow-sm">
                  <div className="flex items-center gap-2 mb-4">
                    <div className="bg-emerald-100 p-2 rounded-lg text-emerald-600">
                      <TrendingUp className="w-5 h-5" />
                    </div>
                    <h3 className="font-bold text-lg">절세 공제 항목 체크리스트</h3>
                  </div>
                  <div className="space-y-3">
                    {deductionChecklist.map(item => (
                      <div
                        key={item.id}
                        onClick={() => {
                          toggleDeduction(item.id);
                          if (!item.checked) {
                            const savingAmount = item.id === 3 ? "1,500,000" : item.id === 4 ? "200,000" : "500,000";
                            showToast(`${item.label} 체크! 예상 절세액: ${savingAmount}원`, "success");
                          }
                        }}
                        className={`flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-colors ${item.checked ? 'bg-emerald-50 border-emerald-200' : 'hover:bg-gray-50'}`}
                      >
                        <div className={`mt-0.5 w-5 h-5 rounded border flex items-center justify-center ${item.checked ? 'bg-emerald-500 border-emerald-500' : 'bg-white border-gray-300'}`}>
                          {item.checked && <Check className="w-3.5 h-3.5 text-white" />}
                        </div>
                        <div>
                          <p className={`text-sm font-medium ${item.checked ? 'text-emerald-900 line-through opacity-70' : 'text-gray-900'}`}>{item.label}</p>
                          <p className="text-xs text-gray-500 mt-0.5">{item.tip}</p>
                        </div>
                      </div>
                    ))}
                  </div>
                  <div className="mt-4 pt-3 border-t text-center">
                    <p className="text-sm text-emerald-600 font-bold">
                      {deductionChecklist.filter(i => i.checked).length} / {deductionChecklist.length} 완료
                    </p>
                  </div>
                </div>
              </div>

              <div className="p-10 border-2 border-dashed border-gray-300 rounded-xl flex flex-col items-center justify-center bg-gray-50 hover:bg-gray-100 transition-colors cursor-pointer group">
                <div className="w-16 h-16 bg-white rounded-full flex items-center justify-center border shadow-sm mb-4 group-hover:scale-110 transition-transform">
                  <PlusCircle className="w-8 h-8 text-primary" />
                </div>
                <h3 className="font-bold text-gray-800 text-lg">새로운 증빙 업로드</h3>
                <p className="text-sm text-gray-500 mt-1">종이 영수증 사진을 찍어 올리거나 PDF를 드래그하세요.</p>
              </div>
            </div>
          )}

          {/* RUNWAY VIEW (Startup) */}
          {activeTab === "runway" && (
            <div className="max-w-4xl mx-auto space-y-6">
              <div className="mb-4">
                <h2 className="text-2xl font-bold flex items-center gap-2">
                  <TrendingUp className="w-6 h-6 text-primary" /> Runway & Burn Rate
                </h2>
                <p className="text-sm text-muted-foreground">스타트업 생존을 위한 자금 흐름 분석</p>
              </div>
              <div className="grid grid-cols-2 gap-4 mb-6">
                <div className="p-6 bg-card rounded-xl border shadow-sm">
                  <h3 className="text-sm font-medium text-gray-500">Current Runway</h3>
                  <div className={`text-3xl font-bold mt-2 ${Number(estimatedMonths) < 6 ? 'text-red-500' : 'text-blue-600'}`}>
                    {estimatedMonths} Months
                  </div>
                  <p className="text-xs text-gray-400 mt-1">이대로라면 {new Date(new Date().setMonth(new Date().getMonth() + Number(estimatedMonths))).toLocaleDateString()} 자금 소진 예상</p>
                </div>
                <div className="p-6 bg-card rounded-xl border shadow-sm">
                  <h3 className="text-sm font-medium text-gray-500">Monthly Burn Rate</h3>
                  <div className="text-3xl font-bold mt-2 text-red-500">₩{runwayBurn.toLocaleString()}</div>
                  <div className="flex items-center gap-2 mt-2">
                    <span className="text-xs text-gray-400">조정:</span>
                    <input
                      type="range"
                      min="1000000"
                      max="50000000"
                      step="1000000"
                      value={runwayBurn}
                      onChange={(e) => setRunwayBurn(Number(e.target.value))}
                      className="w-24 h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer"
                    />
                  </div>
                </div>
              </div>

              <div className="bg-white p-4 rounded-xl border shadow-sm mb-6">
                <h3 className="font-bold text-sm text-gray-700 mb-3">💰 시뮬레이터 설정</h3>
                <div className="flex gap-4 items-end">
                  <div className="flex-1">
                    <label className="text-xs font-medium text-gray-500 mb-1 block">현재 보유 현금 (Cash)</label>
                    <input
                      type="number"
                      value={runwayCash}
                      onChange={(e) => setRunwayCash(Number(e.target.value))}
                      className="w-full p-2 border rounded font-bold text-right"
                    />
                  </div>
                  <div className="flex-1">
                    <label className="text-xs font-medium text-gray-500 mb-1 block">월 평균 지출 (Burn)</label>
                    <input
                      type="number"
                      value={runwayBurn}
                      onChange={(e) => setRunwayBurn(Number(e.target.value))}
                      className="w-full p-2 border rounded font-bold text-right text-red-500"
                    />
                  </div>
                  <div className="pb-2 text-sm text-gray-500">
                    = 런웨이 <strong className="text-black">{estimatedMonths}개월</strong>
                  </div>
                </div>
              </div>

              <div className="h-80 bg-white rounded-xl border shadow-sm p-4 relative overflow-hidden">
                <h3 className="font-bold text-sm text-gray-500 mb-4">Cash Flow Projection (Simulator)</h3>
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={Array.from({ length: 12 }, (_, i) => ({
                    month: `${i + 1}월후`,
                    cash: Math.max(0, runwayCash - (runwayBurn * i))
                  }))}>
                    <defs>
                      <linearGradient id="colorCash" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.8} />
                        <stop offset="95%" stopColor="#3b82f6" stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                    <XAxis dataKey="month" axisLine={false} tickLine={false} tick={{ fontSize: 12 }} />
                    <YAxis axisLine={false} tickLine={false} tick={{ fontSize: 12 }} hide />
                    <Tooltip contentStyle={{ borderRadius: '8px', border: 'none', boxShadow: '0 4px 6px -1px rgb(0 0 0 / 0.1)' }} />
                    <Area type="monotone" dataKey="cash" stroke="#3b82f6" fillOpacity={1} fill="url(#colorCash)" />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
              <div className="h-80 bg-white rounded-xl border shadow-sm p-4 relative overflow-hidden">
                <h3 className="font-bold text-sm text-gray-500 mb-4">비용 구성 비율</h3>
                <ResponsiveContainer width="100%" height="80%">
                  <PieChart>
                    <Pie
                      data={[
                        { name: '인건비', value: 45, color: '#3b82f6' },
                        { name: '마케팅', value: 20, color: '#10b981' },
                        { name: '운영비', value: 15, color: '#f59e0b' },
                        { name: '기타', value: 20, color: '#6b7280' },
                      ]}
                      cx="50%"
                      cy="50%"
                      innerRadius={50}
                      outerRadius={80}
                      paddingAngle={2}
                      dataKey="value"
                      label={({ name, percent }: any) => `${name || ''} ${((percent || 0) * 100).toFixed(0)}%`}
                    >
                      {[
                        { name: '인건비', value: 45, color: '#3b82f6' },
                        { name: '마케팅', value: 20, color: '#10b981' },
                        { name: '운영비', value: 15, color: '#f59e0b' },
                        { name: '기타', value: 20, color: '#6b7280' },
                      ].map((entry, index) => (
                        <Cell key={`cell-${index}`} fill={entry.color} />
                      ))}
                    </Pie>
                    <Tooltip />
                  </PieChart>
                </ResponsiveContainer>
              </div>
            </div>
          )}

          {/* HOSPITAL CLAIMS VIEW */}
          {activeTab === "hospital_claims" && (
            <div className="max-w-6xl mx-auto space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
              <div className="border-b pb-4">
                <h1 className="text-2xl font-bold">🏥 요양급여 청구 현황</h1>
                <p className="text-muted-foreground">건강보험심사평가원 심사결과 및 청구 현황</p>
              </div>
              <div className="grid grid-cols-4 gap-4">
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">청구금액</p>
                  <p className="text-2xl font-bold text-blue-600">156,780,000원</p>
                </div>
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">삭감금액</p>
                  <p className="text-2xl font-bold text-red-500">-4,230,000원</p>
                </div>
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">결정금액</p>
                  <p className="text-2xl font-bold text-green-600">152,550,000원</p>
                </div>
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">삭감률</p>
                  <p className="text-2xl font-bold">2.7%</p>
                </div>
              </div>
              <div className="bg-white p-6 rounded-xl border">
                <h3 className="font-bold mb-4">진료과별 청구 현황</h3>
                <table className="w-full text-sm">
                  <thead className="bg-gray-50">
                    <tr><th className="p-2 text-left">진료과</th><th className="p-2">환자수</th><th className="p-2">청구건수</th><th className="p-2">청구금액</th></tr>
                  </thead>
                  <tbody>
                    <tr className="border-t"><td className="p-2">내과</td><td className="p-2 text-center">412</td><td className="p-2 text-center">523</td><td className="p-2 text-right">45,230,000원</td></tr>
                    <tr className="border-t"><td className="p-2">외과</td><td className="p-2 text-center">189</td><td className="p-2 text-center">245</td><td className="p-2 text-right">38,450,000원</td></tr>
                    <tr className="border-t"><td className="p-2">정형외과</td><td className="p-2 text-center">234</td><td className="p-2 text-center">312</td><td className="p-2 text-right">42,100,000원</td></tr>
                    <tr className="border-t"><td className="p-2">피부과</td><td className="p-2 text-center">156</td><td className="p-2 text-center">178</td><td className="p-2 text-right">15,800,000원</td></tr>
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* HOSPITAL P&L VIEW */}
          {activeTab === "hospital_pnl" && (
            <div className="max-w-6xl mx-auto space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
              <div className="border-b pb-4">
                <h1 className="text-2xl font-bold">📊 병원 손익 분석</h1>
                <p className="text-muted-foreground">급여/비급여 매출 및 비용 분석</p>
              </div>
              <div className="grid grid-cols-3 gap-4">
                <div className="bg-gradient-to-br from-blue-500 to-blue-600 text-white p-6 rounded-xl">
                  <p className="text-sm opacity-80">총 매출</p>
                  <p className="text-3xl font-bold">233,060,000원</p>
                  <p className="text-sm mt-2">급여 67.3% / 비급여 32.7%</p>
                </div>
                <div className="bg-gradient-to-br from-red-400 to-red-500 text-white p-6 rounded-xl">
                  <p className="text-sm opacity-80">총 비용</p>
                  <p className="text-3xl font-bold">187,830,000원</p>
                  <p className="text-sm mt-2">인건비 45% / 재료비 12%</p>
                </div>
                <div className="bg-gradient-to-br from-green-500 to-green-600 text-white p-6 rounded-xl">
                  <p className="text-sm opacity-80">영업이익</p>
                  <p className="text-3xl font-bold">45,230,000원</p>
                  <p className="text-sm mt-2">영업이익률 19.4%</p>
                </div>
              </div>
              <div className="bg-white p-6 rounded-xl border">
                <h3 className="font-bold mb-4">비급여 매출 TOP 5</h3>
                <div className="space-y-3">
                  {[
                    { name: "도수치료", amount: 24500000, ratio: 32 },
                    { name: "MRI 검사", amount: 17800000, ratio: 23 },
                    { name: "CT 검사", amount: 12300000, ratio: 16 },
                    { name: "초음파", amount: 9360000, ratio: 12 },
                    { name: "주사료", amount: 6840000, ratio: 9 },
                  ].map((item, i) => (
                    <div key={i} className="flex items-center gap-4">
                      <span className="w-24 text-sm">{item.name}</span>
                      <div className="flex-1 h-6 bg-gray-100 rounded-full overflow-hidden">
                        <div className="h-full bg-primary" style={{ width: `${item.ratio}%` }}></div>
                      </div>
                      <span className="text-sm font-medium w-28 text-right">{item.amount.toLocaleString()}원</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* COMMERCE ROAS VIEW */}
          {activeTab === "commerce_roas" && (
            <div className="max-w-6xl mx-auto space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
              <div className="border-b pb-4">
                <h1 className="text-2xl font-bold">📈 광고 ROAS 분석</h1>
                <p className="text-muted-foreground">채널별 광고 성과 및 수익률 분석</p>
              </div>
              <div className="grid grid-cols-4 gap-4">
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">총 광고비</p>
                  <p className="text-2xl font-bold">37,000,000원</p>
                </div>
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">광고 매출</p>
                  <p className="text-2xl font-bold text-green-600">118,400,000원</p>
                </div>
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">평균 ROAS</p>
                  <p className="text-2xl font-bold text-blue-600">320%</p>
                </div>
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">전환수</p>
                  <p className="text-2xl font-bold">3,990건</p>
                </div>
              </div>
              <div className="bg-white p-6 rounded-xl border">
                <h3 className="font-bold mb-4">채널별 성과</h3>
                <table className="w-full text-sm">
                  <thead className="bg-gray-50">
                    <tr><th className="p-2 text-left">채널</th><th className="p-2">광고비</th><th className="p-2">클릭수</th><th className="p-2">전환수</th><th className="p-2">ROAS</th></tr>
                  </thead>
                  <tbody>
                    <tr className="border-t"><td className="p-2">네이버 검색광고</td><td className="p-2 text-right">12,500,000원</td><td className="p-2 text-center">45,000</td><td className="p-2 text-center">1,350</td><td className="p-2 text-center text-green-600 font-bold">320%</td></tr>
                    <tr className="border-t"><td className="p-2">쿠팡 광고</td><td className="p-2 text-right">4,000,000원</td><td className="p-2 text-center">15,000</td><td className="p-2 text-center">600</td><td className="p-2 text-center text-green-600 font-bold">450%</td></tr>
                    <tr className="border-t"><td className="p-2">카카오 모먼트</td><td className="p-2 text-right">8,200,000원</td><td className="p-2 text-center">28,000</td><td className="p-2 text-center">840</td><td className="p-2 text-center text-green-600 font-bold">285%</td></tr>
                    <tr className="border-t"><td className="p-2">구글 Ads</td><td className="p-2 text-right">6,800,000원</td><td className="p-2 text-center">22,000</td><td className="p-2 text-center">660</td><td className="p-2 text-center text-green-600 font-bold">310%</td></tr>
                    <tr className="border-t"><td className="p-2">메타 광고</td><td className="p-2 text-right">5,500,000원</td><td className="p-2 text-center">18,000</td><td className="p-2 text-center">540</td><td className="p-2 text-center text-yellow-600 font-bold">275%</td></tr>
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* COMMERCE INVENTORY VIEW */}
          {activeTab === "commerce_inventory" && (
            <div className="max-w-6xl mx-auto space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
              <div className="border-b pb-4">
                <h1 className="text-2xl font-bold">📦 재고 관리</h1>
                <p className="text-muted-foreground">카테고리별 재고 현황 및 회전율 분석</p>
              </div>
              <div className="grid grid-cols-4 gap-4">
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">총 재고금액</p>
                  <p className="text-2xl font-bold">111,000,000원</p>
                </div>
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">재고회전일</p>
                  <p className="text-2xl font-bold text-blue-600">14일</p>
                </div>
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">평균 마진율</p>
                  <p className="text-2xl font-bold text-green-600">47%</p>
                </div>
                <div className="bg-white p-4 rounded-xl border">
                  <p className="text-sm text-gray-500">SKU 수</p>
                  <p className="text-2xl font-bold">1,245개</p>
                </div>
              </div>
              <div className="bg-white p-6 rounded-xl border">
                <h3 className="font-bold mb-4">카테고리별 재고</h3>
                <table className="w-full text-sm">
                  <thead className="bg-gray-50">
                    <tr><th className="p-2 text-left">카테고리</th><th className="p-2">매출액</th><th className="p-2">원가</th><th className="p-2">마진율</th><th className="p-2">재고금액</th></tr>
                  </thead>
                  <tbody>
                    <tr className="border-t"><td className="p-2">의류/패션</td><td className="p-2 text-right">145,000,000원</td><td className="p-2 text-right">72,500,000원</td><td className="p-2 text-center">50%</td><td className="p-2 text-right">38,000,000원</td></tr>
                    <tr className="border-t"><td className="p-2">화장품/뷰티</td><td className="p-2 text-right">89,000,000원</td><td className="p-2 text-right">35,600,000원</td><td className="p-2 text-center text-green-600">60%</td><td className="p-2 text-right">22,000,000원</td></tr>
                    <tr className="border-t"><td className="p-2">생활/주방</td><td className="p-2 text-right">56,000,000원</td><td className="p-2 text-right">33,600,000원</td><td className="p-2 text-center">40%</td><td className="p-2 text-right">15,000,000원</td></tr>
                    <tr className="border-t"><td className="p-2">가전/디지털</td><td className="p-2 text-right">34,000,000원</td><td className="p-2 text-right">27,200,000원</td><td className="p-2 text-center text-red-500">20%</td><td className="p-2 text-right">28,000,000원</td></tr>
                    <tr className="border-t"><td className="p-2">식품</td><td className="p-2 text-right">20,000,000원</td><td className="p-2 text-right">14,000,000원</td><td className="p-2 text-center">30%</td><td className="p-2 text-right">8,000,000원</td></tr>
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* MEMORY VIEW */}
          {activeTab === "memory" && (
            <div className="max-w-4xl mx-auto space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
              <div className="border-b pb-4">
                <h1 className="text-2xl font-bold">📚 결정 히스토리</h1>
                <p className="text-muted-foreground">과거 결정과 결과를 요약합니다.</p>
                {memoryFilterKey && (
                  <div className="mt-2 flex flex-wrap gap-2">
                    <button
                      onClick={() => setMemoryFilterKey(null)}
                      className="text-xs px-2 py-1 rounded-full border hover:bg-muted"
                    >
                      필터 해제
                    </button>
                    <button
                      onClick={() => setShowSimilarOnly((prev) => !prev)}
                      className={`text-xs px-2 py-1 rounded-full border ${showSimilarOnly ? "bg-primary text-primary-foreground border-primary" : "hover:bg-muted"}`}
                    >
                      유사 판단만 보기
                    </button>
                  </div>
                )}
                <div className="mt-3 flex flex-wrap gap-2 text-xs">
                  {(["all", "positive", "neutral", "negative", "pending"] as const).map((status) => (
                    <button
                      key={status}
                      onClick={() => setOutcomeFilter(status)}
                      className={`px-2 py-1 rounded-full border ${outcomeFilter === status ? "bg-primary text-primary-foreground border-primary" : "hover:bg-muted"}`}
                    >
                      {status === "all"
                        ? "전체"
                        : status === "positive"
                          ? "긍정"
                          : status === "neutral"
                            ? "중립"
                            : status === "negative"
                              ? "부정"
                              : "보류"}
                    </button>
                  ))}
                </div>
                {memoryFilterKey && (
                  <div className="mt-3 p-3 rounded-lg border bg-muted/30 space-y-3">
                    <div>
                      <p className="text-xs text-muted-foreground mb-2">유사도 가중치</p>
                      <div className="flex flex-wrap gap-3 text-xs">
                        <label className="flex items-center gap-2">
                          <span>우선순위</span>
                          <input
                            type="range"
                            min="1"
                            max="3"
                            value={similarityWeights.priority}
                            onChange={(e) => setSimilarityWeights((prev) => ({ ...prev, priority: Number(e.target.value) }))}
                          />
                          <span>{similarityWeights.priority}</span>
                        </label>
                        <label className="flex items-center gap-2">
                          <span>런웨이</span>
                          <input
                            type="range"
                            min="1"
                            max="3"
                            value={similarityWeights.runway}
                            onChange={(e) => setSimilarityWeights((prev) => ({ ...prev, runway: Number(e.target.value) }))}
                          />
                          <span>{similarityWeights.runway}</span>
                        </label>
                        <label className="flex items-center gap-2">
                          <span>리스크</span>
                          <input
                            type="range"
                            min="1"
                            max="3"
                            value={similarityWeights.risk}
                            onChange={(e) => setSimilarityWeights((prev) => ({ ...prev, risk: Number(e.target.value) }))}
                          />
                          <span>{similarityWeights.risk}</span>
                        </label>
                      </div>
                    </div>
                  </div>
                )}
                <div className="mt-3 text-[11px] text-muted-foreground">
                  유사도 범례: 낮음(0~2) · 중간(3~4) · 높음(5+)
                </div>
              </div>
              <div className="p-6 bg-white rounded-xl border shadow-sm">
                <div className="flex justify-between items-start mb-4">
                  <div>
                    <h3 className="font-bold flex items-center gap-2 text-lg">🧠 AI 사고 지도 (Thinking Map)</h3>
                    <p className="text-sm text-muted-foreground mt-1">
                      의사결정 간의 연결 관계를 시각화하여 AI가 판단 근거를 어떻게 확장하는지 보여줍니다.
                    </p>
                  </div>
                  {memoryGraph && (
                    <div className="flex items-center gap-2">
                      <div className="text-xs bg-muted px-2 py-1 rounded">
                        Nodes: {memoryGraph.nodes.length} / Edges: {memoryGraph.edges.length}
                      </div>
                      <button
                        onClick={saveGraphPositions}
                        className="text-xs px-2 py-1 rounded border hover:bg-muted"
                      >
                        레이아웃 저장
                      </button>
                      {graphLayout && (
                        <button
                          onClick={() => setGraphPositions(graphLayout.positionMap)}
                          className="text-xs px-2 py-1 rounded border hover:bg-muted"
                        >
                          레이아웃 초기화
                        </button>
                      )}
                      {(selectedGraphNode || selectedEdgeKey) && (
                        <button
                          onClick={() => {
                            setSelectedGraphNode(null);
                            setSelectedEdgeKey(null);
                          }}
                          className="text-xs px-2 py-1 rounded border hover:bg-muted"
                        >
                          선택 해제
                        </button>
                      )}
                      {layoutSaved && (
                        <span className="text-xs text-emerald-600">저장됨</span>
                      )}
                    </div>
                  )}
                </div>

                {memoryGraph && memoryGraph.nodes.length > 0 ? (
                  <div className="relative h-[300px] w-full bg-slate-50/50 rounded-lg overflow-hidden border flex items-center justify-center">
                    <div className="absolute inset-0 flex items-center justify-center opacity-10 pointer-events-none">
                      <Activity className="w-64 h-64 text-slate-300" />
                    </div>
                    <div className="absolute top-3 right-3 text-[10px] bg-white/90 border rounded px-2 py-1 space-y-1">
                      <div className="flex items-center gap-2"><span className="w-2 h-2 rounded-full bg-emerald-500" /> CompanyState / AI_Judgement</div>
                      <div className="flex items-center gap-2"><span className="w-2 h-2 rounded-full bg-blue-500" /> Human_Decision</div>
                      <div className="flex items-center gap-2"><span className="w-2 h-2 rounded-full bg-red-400" /> Outcome</div>
                    </div>
                    {graphLayout && (
                      <svg className="absolute inset-0" viewBox="0 0 300 300">
                        {memoryGraph.edges.map((edge, idx) => {
                          const source = graphPositions[edge.source] || graphLayout.positionMap[edge.source];
                          const target = graphPositions[edge.target] || graphLayout.positionMap[edge.target];
                          if (!source || !target) return null;
                          const edgeKey = `${edge.source}-${edge.target}-${idx}`;
                          const isHovered = hoveredEdgeKey === edgeKey;
                          const isSelected = selectedEdgeKey === edgeKey;
                          return (
                            <line
                              key={edgeKey}
                              x1={source.x}
                              y1={source.y}
                              x2={target.x}
                              y2={target.y}
                              stroke={isSelected ? "rgba(16, 185, 129, 0.9)" : isHovered ? "rgba(59, 130, 246, 0.9)" : "rgba(148, 163, 184, 0.6)"}
                              strokeWidth={isSelected ? "2.5" : isHovered ? "2" : "1"}
                              onMouseEnter={() => setHoveredEdgeKey(edgeKey)}
                              onMouseLeave={() => setHoveredEdgeKey(null)}
                              onClick={() => setSelectedEdgeKey(edgeKey)}
                              style={{ cursor: "pointer" }}
                            />
                          );
                        })}
                      </svg>
                    )}
                    <div className="relative z-10 w-[300px] h-[300px]">
                      {memoryGraph.nodes.map((node, _i) => {
                        const pos = graphPositions[node.id] || graphLayout?.positionMap[node.id];
                        if (!pos) return null;
                        return (
                          <button
                            key={node.id}
                            onClick={() => {
                              const baseId = node.id.split(":").slice(1).join(":") || node.id;
                              setMemoryHighlightId(baseId);
                              setSelectedGraphNode(node);
                            }}
                            onPointerDown={(event) => {
                              event.stopPropagation();
                              event.currentTarget.setPointerCapture(event.pointerId);
                              setDraggingNodeId(node.id);
                            }}
                            onPointerMove={(event) => {
                              if (draggingNodeId !== node.id) return;
                              const rect = (event.currentTarget.parentElement as HTMLDivElement).getBoundingClientRect();
                              // Limit boundaries
                              const x = Math.max(10, Math.min(290, ((event.clientX - rect.left) / rect.width) * 300));
                              const y = Math.max(10, Math.min(290, ((event.clientY - rect.top) / rect.height) * 300));
                              setGraphPositions((prev) => ({ ...prev, [node.id]: { x, y } }));
                            }}
                            onPointerUp={(event) => {
                              setDraggingNodeId(null);
                              event.currentTarget.releasePointerCapture(event.pointerId);
                            }}
                            onPointerLeave={() => { }}
                            className={`absolute -translate-x-1/2 -translate-y-1/2 bg-white px-3 py-2 rounded-lg border shadow-sm hover:shadow-md hover:border-primary/50 transition-all text-left flex items-center gap-2 ${selectedGraphNode?.id === node.id ? "ring-2 ring-primary/40" : ""}`}
                            style={{ left: pos.x, top: pos.y }}
                            aria-label={`그래프 노드 ${node.type}`}
                          >
                            <span className={`w-2 h-2 rounded-full ${node.type === 'Human_Decision' ? 'bg-blue-500' : node.type === 'Outcome' ? 'bg-red-400' : 'bg-emerald-500'}`} />
                            <span className="text-[10px] font-medium max-w-[90px] truncate">
                              {node.data.label || node.id}
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  </div>
                ) : (
                  <div className="h-[200px] flex flex-col items-center justify-center bg-slate-50 rounded-lg border border-dashed">
                    <p className="text-muted-foreground text-sm">아직 연결된 지식 그래프가 충분하지 않습니다.</p>
                    <p className="text-xs text-gray-400 mt-1">결정을 기록하면 AI가 관계를 학습합니다.</p>
                  </div>
                )}
                {memoryGraph && memoryGraph.edges.length > 0 && (
                  <div className="mt-4 p-4 bg-muted/30 rounded-lg text-xs text-muted-foreground">
                    <p className="font-medium mb-2">엣지 목록 (클릭 시 해당 기록으로 이동)</p>
                    <div className="space-y-1">
                      {memoryGraph.edges.slice(0, 8).map((edge, idx) => (
                        <button
                          key={`${edge.source}-${edge.target}-${idx}`}
                          onClick={() => {
                            const pickBaseId = (value: string) => value.split(":").slice(1).join(":") || value;
                            const baseId = pickBaseId(edge.target) || pickBaseId(edge.source);
                            setMemoryHighlightId(baseId);
                            setSelectedEdgeKey(`${edge.source}-${edge.target}-${idx}`);
                          }}
                          className={`text-left hover:underline ${selectedEdgeKey === `${edge.source}-${edge.target}-${idx}` ? "text-primary" : ""}`}
                          aria-label={`엣지 이동 ${edge.type}`}
                        >
                          {edge.source} → {edge.target}
                          <span className="ml-2 inline-flex items-center rounded-full bg-white/80 px-2 py-0.5 text-[10px]">
                            {edge.type}
                          </span>
                        </button>
                      ))}
                      {memoryGraph.edges.length > 8 && (
                        <div>... {memoryGraph.edges.length - 8}개 더 있음</div>
                      )}
                    </div>
                  </div>
                )}
                {selectedGraphNode && (
                  <div className="mt-4 p-4 bg-white rounded-lg border text-xs">
                    <div className="flex items-center justify-between mb-2">
                      <p className="font-medium">선택 노드 상세</p>
                      <div className="flex items-center gap-2">
                        <button
                          onClick={() => {
                            const baseId = selectedGraphNode.id.split(":").slice(1).join(":") || selectedGraphNode.id;
                            setMemoryHighlightId(baseId);
                            setActiveTab("memory");
                          }}
                          className="text-[11px] px-2 py-1 rounded-full border hover:bg-muted"
                        >
                          MEMORY로 이동
                        </button>
                        <button
                          onClick={() => setSelectedGraphNode(null)}
                          className="text-[11px] px-2 py-1 rounded-full border hover:bg-muted"
                        >
                          닫기
                        </button>
                      </div>
                    </div>
                    <div className="text-muted-foreground">Type: {selectedGraphNode.type}</div>
                    {selectedGraphNode.type === "CompanyState" && (
                      <div className="mt-2 space-y-2">
                        <div className="flex items-start gap-2">
                          <span className="font-medium">runway_months</span>
                          <span className="text-muted-foreground">{String(selectedGraphNode.data?.runway_months ?? "-")}</span>
                        </div>
                        <div className="flex items-start gap-2">
                          <span className="font-medium">risk_score</span>
                          <span className="text-muted-foreground">{String(selectedGraphNode.data?.risk_score ?? "-")}</span>
                        </div>
                      </div>
                    )}
                    {selectedGraphNode.type === "Outcome" && (
                      <div className="mt-2 space-y-2">
                        <div className="flex items-start gap-2">
                          <span className="font-medium">status</span>
                          <span className="text-muted-foreground">{String(selectedGraphNode.data?.status ?? "-")}</span>
                        </div>
                        <div className="flex items-start gap-2">
                          <span className="font-medium">memo</span>
                          <span className="text-muted-foreground break-all">{String(selectedGraphNode.data?.memo ?? "-")}</span>
                        </div>
                      </div>
                    )}
                    {selectedGraphNode.type === "AI_Judgement" && (
                      <div className="mt-2 space-y-2">
                        <div className="flex items-start gap-2">
                          <span className="font-medium">title</span>
                          <span className="text-muted-foreground break-all">{String(selectedGraphNode.data?.title ?? "-")}</span>
                        </div>
                        <div className="flex items-start gap-2">
                          <span className="font-medium">summary</span>
                          <span className="text-muted-foreground break-all">{String(selectedGraphNode.data?.summary ?? "-")}</span>
                        </div>
                        <div className="flex items-start gap-2">
                          <span className="font-medium">priority_score</span>
                          <span className="text-muted-foreground">{String(selectedGraphNode.data?.priority_score ?? "-")}</span>
                        </div>
                      </div>
                    )}
                    {selectedGraphNode.type === "Human_Decision" && (
                      <div className="mt-2 space-y-2">
                        <div className="flex items-start gap-2">
                          <span className="font-medium">status</span>
                          <span className="text-muted-foreground">{String(selectedGraphNode.data?.status ?? "-")}</span>
                        </div>
                        <div className="flex items-start gap-2">
                          <span className="font-medium">rejection_reason</span>
                          <span className="text-muted-foreground break-all">{String(selectedGraphNode.data?.rejection_reason ?? "-")}</span>
                        </div>
                      </div>
                    )}
                    {!["CompanyState", "Outcome"].includes(selectedGraphNode.type) && (
                      <div className="mt-2 space-y-1">
                        {Object.entries(selectedGraphNode.data || {}).slice(0, 8).map(([key, value]) => (
                          <div key={key} className="flex items-start gap-2">
                            <span className="font-medium">{key}</span>
                            <span className="text-muted-foreground break-all">{String(value)}</span>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
              {decisionHistory.length === 0 ? (
                <div className="mt-4 p-4 bg-muted/40 rounded-lg text-sm">
                  아직 기록된 결정이 없습니다. 브리핑 카드의 결정을 수용/거부하면 히스토리가 생성됩니다.
                </div>
              ) : filteredMemoryWithOutcome.length === 0 ? (
                <div className="mt-4 p-4 bg-muted/40 rounded-lg text-sm">
                  필터 조건에 맞는 기록이 없습니다.
                </div>
              ) : (
                <div className="mt-4 space-y-3">
                  {filteredMemoryWithOutcome.map((record) => (
                    <div
                      key={record.id}
                      id={`memory-${record.id}`}
                      className={`p-4 rounded-lg border bg-muted/20 flex items-start justify-between ${memoryHighlightId === record.id ? "ring-2 ring-primary/40" : ""} ${bestSimilarId === record.id ? "ring-2 ring-emerald-400" : ""}`}
                    >
                      <div className="flex-1">
                        <p className="text-sm font-semibold">{record.title}</p>
                        <p className="text-xs text-muted-foreground mt-1">{record.summary}</p>
                        {record.reasons.length > 0 && (
                          <ul className="mt-2 text-xs text-muted-foreground space-y-1">
                            {record.reasons.map((reason) => (
                              <li key={reason}>- {reason}</li>
                            ))}
                          </ul>
                        )}
                        {record.rejectionReason && (
                          <div className="mt-2 text-xs text-red-600">
                            거부 사유: {record.rejectionReason}
                          </div>
                        )}
                        {record.outcomeMemo && (
                          <div
                            className="mt-2 text-xs text-muted-foreground"
                            title={record.outcomeMemo}
                          >
                            결과 메모: {record.outcomeMemo.length > 60 ? `${record.outcomeMemo.slice(0, 60)}...` : record.outcomeMemo}
                          </div>
                        )}
                        <p className="text-[11px] text-muted-foreground mt-2">
                          {new Date(record.createdAt).toLocaleString()}
                        </p>
                        <div className="mt-3 text-[11px] text-muted-foreground">
                          CompanyState → AI_Judgement → Human_Decision → Outcome({record.outcomeStatus})
                        </div>
                        {memoryFilterKey && (
                          <div className="mt-1 text-[11px] text-muted-foreground">
                            Graph Edge: {decisionContext.actionKey}
                          </div>
                        )}
                        {memoryFilterKey && (
                          <div className="mt-2 text-[11px] text-muted-foreground">
                            유사도: {getSimilarityReason(record)} · 점수 {getSimilarityScore(record)}
                          </div>
                        )}
                        <button
                          onClick={() => {
                            setActiveTab("home");
                            setIsDetailOpen(false);
                          }}
                          className="mt-3 text-xs px-2 py-1 rounded-full border hover:bg-muted"
                        >
                          브리핑으로 돌아가기
                        </button>
                      </div>
                      <div className="flex flex-col items-end gap-2">
                        <span className={`text-xs font-medium px-2 py-1 rounded-full ${record.status === "accepted" ? "bg-emerald-100 text-emerald-700" : "bg-red-100 text-red-700"}`}>
                          {record.status === "accepted" ? "수용" : "거부"}
                        </span>
                        <button
                          onClick={() => {
                            setOutcomeTarget(record);
                            setOutcomeStatus(record.outcomeStatus === "pending" ? "neutral" : record.outcomeStatus);
                            setOutcomeMemo(record.outcomeMemo || "");
                            setShowOutcomeModal(true);
                          }}
                          className="text-xs px-2 py-1 rounded-full border hover:bg-muted"
                        >
                          결과 기록
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* SETTINGS VIEW (My Page) */}
          {activeTab === "settings" && (
            <div className="max-w-3xl mx-auto space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
              <div className="border-b pb-6">
                <h1 className="text-3xl font-bold mb-2">My Page & Settings</h1>
                <p className="text-muted-foreground">AI CFO 시스템 설정 및 기업 상세 정보를 관리합니다.</p>
              </div>

              {/* 1. Profile Summary */}
              <div className="bg-white p-6 rounded-xl border shadow-sm flex items-center gap-6">
                <div className="w-20 h-20 bg-primary/10 rounded-full flex items-center justify-center text-primary text-2xl font-bold">
                  {user.name.charAt(0)}
                </div>
                <div>
                  <h2 className="text-xl font-bold">{user.name} 대표님</h2>
                  <p className="text-gray-500">{user.company} | {user.bizNum}</p>
                  <span className="inline-block mt-2 px-3 py-1 bg-gray-100 rounded text-xs text-gray-600 font-mono">
                    Base: {user.type.toUpperCase()} Edition
                  </span>
                </div>
              </div>

              {/* 1.5 System Status */}
              <div className="bg-white p-6 rounded-xl border shadow-sm">
                <div className="flex items-center justify-between mb-3">
                  <h3 className="text-lg font-bold">🛰️ System Status</h3>
                  <button
                    onClick={fetchApiHealth}
                    disabled={apiHealthLoading}
                    className="text-xs px-2 py-1 rounded border hover:bg-muted disabled:opacity-60"
                  >
                    {apiHealthLoading ? "확인 중..." : "새로고침"}
                  </button>
                </div>
                <div className="flex items-center gap-2 text-sm">
                  <span
                    className={`w-2 h-2 rounded-full ${apiHealth?.status === "ok"
                      ? "bg-emerald-500"
                      : apiHealthError
                        ? "bg-red-500"
                        : "bg-gray-300"
                      }`}
                  />
                  <span>
                    API 상태: {apiHealth?.status === "ok" ? "정상" : apiHealthError ? "오류" : "확인 중"}
                  </span>
                  {apiHealthCheckedAt && (
                    <span className="text-xs text-muted-foreground">마지막 확인 {apiHealthCheckedAt}</span>
                  )}
                </div>
                {apiHealthError && (
                  <p className="text-xs text-red-600 mt-2">서버 연결에 실패했습니다.</p>
                )}
              </div>

              {/* 1.6 Company Info */}
              <div className="bg-white p-6 rounded-xl border shadow-sm">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-lg font-bold">회사 정보 관리</h3>
                  <span className="text-xs bg-emerald-50 text-emerald-700 px-2 py-1 rounded">온보딩/수정</span>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <label className="text-sm font-medium">대표자 이름</label>
                    <input
                      type="text"
                      className="w-full p-2 border rounded-lg"
                      value={profileDraft.name}
                      onChange={(e) => setProfileDraft((prev) => ({ ...prev, name: e.target.value }))}
                      placeholder="예: 홍길동"
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">회사명</label>
                    <input
                      type="text"
                      className="w-full p-2 border rounded-lg"
                      value={profileDraft.company}
                      onChange={(e) => setProfileDraft((prev) => ({ ...prev, company: e.target.value }))}
                      placeholder="예: (주)회사명"
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">사업자번호</label>
                    <input
                      type="text"
                      className="w-full p-2 border rounded-lg"
                      value={profileDraft.bizNum}
                      onChange={(e) => setProfileDraft((prev) => ({ ...prev, bizNum: e.target.value }))}
                      placeholder="000-00-00000"
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">업종 타입</label>
                    <select
                      className="w-full p-2 border rounded-lg bg-white"
                      value={profileDraft.type}
                      onChange={(e) => setProfileDraft((prev) => ({ ...prev, type: e.target.value }))}
                    >
                      <option value="general">General</option>
                      <option value="startup">Startup</option>
                      <option value="hospital">Hospital</option>
                      <option value="commerce">Commerce</option>
                    </select>
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">목표 매출</label>
                    <input
                      type="text"
                      className="w-full p-2 border rounded-lg"
                      value={profileDraft.targetRevenue}
                      onChange={(e) => setProfileDraft((prev) => ({ ...prev, targetRevenue: e.target.value }))}
                      placeholder="예: 500000000"
                    />
                  </div>
                </div>
                <div className="mt-4 flex items-center justify-between p-3 bg-gray-50 rounded-lg">
                  <span className="text-xs text-gray-500">* 마이페이지에서 회사 정보를 수정/저장할 수 있습니다.</span>
                  <button
                    onClick={handleProfileSave}
                    disabled={isSavingProfile}
                    className="px-4 py-2 bg-primary text-primary-foreground text-sm font-medium rounded-lg hover:bg-primary/90 transition-colors shadow-sm flex items-center gap-2"
                  >
                    {isSavingProfile && <Loader2 className="w-4 h-4 animate-spin" />}
                    {isSavingProfile ? "저장 중..." : "회사 정보 저장"}
                  </button>
                </div>
              </div>

              {/* 2. Advanced RFI Input */}
              <div className="bg-white p-6 rounded-xl border shadow-sm">
                <div className="flex justify-between items-center mb-6">
                  <h3 className="text-lg font-bold flex items-center gap-2">
                    <User className="w-5 h-5 text-primary" /> RFI (Request For Information)
                  </h3>
                  <span className="text-xs bg-blue-50 text-blue-600 px-2 py-1 rounded">AI 정확도 향상용</span>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <label className="text-sm font-medium">현재 팀 규모 (명)</label>
                    <input type="number" className="w-full p-2 border rounded-lg" placeholder="예: 5" defaultValue={user.rfiData?.teamSize} onChange={e => setUser({ ...user, rfiData: { ...user.rfiData, teamSize: e.target.value } })} />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">주요 비용 항목</label>
                    <input type="text" className="w-full p-2 border rounded-lg" placeholder="예: 인건비, 서버비" defaultValue={user.rfiData?.keyExpense} onChange={e => setUser({ ...user, rfiData: { ...user.rfiData, keyExpense: e.target.value } })} />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">현재 투자/자금 단계</label>
                    <select className="w-full p-2 border rounded-lg bg-white" defaultValue={user.rfiData?.fundingStage} onChange={e => setUser({ ...user, rfiData: { ...user.rfiData, fundingStage: e.target.value } })}>
                      <option value="">선택해주세요</option>
                      <option value="bootstrap">Bootstrap (자기자본)</option>
                      <option value="seed">Seed / Angel</option>
                      <option value="seriesA">Series A</option>
                    </select>
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">연간 예산 범위</label>
                    <input type="text" className="w-full p-2 border rounded-lg" placeholder="예: 5억 ~ 10억" defaultValue={user.rfiData?.budget} onChange={e => setUser({ ...user, rfiData: { ...user.rfiData, budget: e.target.value } })} />
                  </div>
                </div>
                <div className="mt-4 flex items-center justify-between p-3 bg-gray-50 rounded-lg">
                  <span className="text-xs text-gray-500">* 입력하신 정보는 AI 상담 및 분석 시 컨텍스트로 활용되어 더 정확한 답변을 제공합니다.</span>
                  <button
                    onClick={() => {
                      setIsSavingRFI(true);
                      setTimeout(() => {
                        showToast('RFI 정보가 성공적으로 저장되었습니다.', 'success');
                        setIsSavingRFI(false);
                      }, 1000);
                    }}
                    className="px-4 py-2 bg-primary text-primary-foreground text-sm font-medium rounded-lg hover:bg-primary/90 transition-colors shadow-sm flex items-center gap-2"
                  >
                    {isSavingRFI && <Loader2 className="w-4 h-4 animate-spin" />}
                    {isSavingRFI ? '저장 중...' : '저장하기'}
                  </button>
                </div>
              </div>

              {/* 3. Preset Dataset Loader */}
              <div className="bg-white p-6 rounded-xl border shadow-sm space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <h3 className="text-lg font-bold">프리셋 데이터셋</h3>
                    <p className="text-sm text-muted-foreground">업종별 예시 데이터를 바로 적용하세요.</p>
                  </div>
                  {activeDatasetName && (
                    <span className="text-xs bg-primary/10 text-primary px-2 py-1 rounded-full">적용중: {activeDatasetName}</span>
                  )}
                </div>
                <div className="space-y-3">
                  {PRESET_GROUPS.map((group) => (
                    <div key={group.id} className="rounded-xl border bg-muted/30 p-4">
                      <div className="flex items-center justify-between mb-3">
                        <div>
                          <div className="text-sm font-semibold">{group.label}</div>
                          <div className="text-xs text-muted-foreground">{group.desc}</div>
                        </div>
                      </div>
                      <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                        {group.items.map((preset) => {
                          const isActive = activeDatasetName === preset.name;
                          return (
                            <div key={preset.id} className={`p-3 rounded-lg border text-left transition-colors ${isActive ? "border-primary ring-2 ring-primary/20 bg-white" : "border-muted bg-white hover:bg-muted/40"}`}>
                              <div className="flex items-start justify-between gap-2">
                                <div>
                                  <div className="text-sm font-semibold">{preset.persona}</div>
                                  <div className="text-xs text-muted-foreground">{preset.summary}</div>
                                </div>
                                <div className="flex flex-col items-end gap-1">
                                  {preset.badge && <span className="text-[10px] bg-primary/10 text-primary px-2 py-0.5 rounded-full">{preset.badge}</span>}
                                  {isActive && <span className="text-[10px] bg-emerald-100 text-emerald-700 px-2 py-0.5 rounded-full">사용중</span>}
                                </div>
                              </div>
                              <div className="mt-2 text-[11px] text-muted-foreground flex flex-wrap gap-2">
                                <span>현금 ₩{preset.meta.cash.toLocaleString()}</span>
                                <span>매출 ₩{preset.meta.monthlyRevenue.toLocaleString()}/월</span>
                                <span>지출 ₩{preset.meta.monthlyExpense.toLocaleString()}/월</span>
                              </div>
                              <div className="mt-3 flex gap-2">
                                <button type="button" onClick={() => setPresetPreview(preset)} className="flex-1 px-3 py-1.5 text-[11px] rounded-lg border hover:bg-muted transition-colors">미리보기</button>
                                <button type="button" onClick={() => applyPresetDataset(preset)} disabled={isActive} className={`flex-1 px-3 py-1.5 text-[11px] rounded-lg transition-colors ${isActive ? "bg-muted text-muted-foreground cursor-not-allowed" : "bg-primary text-primary-foreground hover:bg-primary/90"}`}>{isActive ? "사용중" : "바로 적용"}</button>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              {/* Warning Preferences */}
              <div className="bg-white p-6 rounded-xl border shadow-sm">
                <h3 className="text-lg font-bold mb-4">경고 톤 & 임계값</h3>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
                  <div className="space-y-2">
                    <label className="text-sm font-medium">경고 톤</label>
                    <select
                      className="w-full p-2 border rounded-lg bg-white"
                      value={tonePreference}
                      onChange={(e) => setTonePreference(e.target.value as "direct" | "neutral" | "soft")}
                    >
                      <option value="direct">직설</option>
                      <option value="neutral">중립</option>
                      <option value="soft">완곡</option>
                    </select>
                    <p className="text-xs text-muted-foreground">브리핑/질문 답변의 경고 톤에 반영됩니다.</p>
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">경고 임계값</label>
                    <input
                      type="range"
                      min="1"
                      max="5"
                      value={warningThreshold}
                      onChange={(e) => setWarningThreshold(Number(e.target.value))}
                      className="w-full"
                    />
                    <div className="text-xs text-muted-foreground">현재: {warningThreshold}</div>
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">요약 길이</label>
                    <select
                      className="w-full p-2 border rounded-lg bg-white"
                      value={summaryLength}
                      onChange={(e) => setSummaryLength(e.target.value as "short" | "normal" | "detailed")}
                    >
                      <option value="short">짧게</option>
                      <option value="normal">보통</option>
                      <option value="detailed">자세히</option>
                    </select>
                    <p className="text-xs text-muted-foreground">질문 패널 답변 길이에 반영됩니다.</p>
                  </div>
                </div>
              </div>

              {/* AI CFO Calibration */}
              <div className="bg-white p-6 rounded-xl border shadow-sm">
                <div className="flex items-center justify-between mb-3">
                  <h3 className="text-lg font-bold">AI CFO 초기 설정</h3>
                  <span className="text-xs bg-blue-50 text-blue-600 px-2 py-1 rounded">리셋 가능</span>
                </div>
                <p className="text-sm text-muted-foreground mb-4">
                  결정 히스토리가 비어 있을 때 자동으로 표시됩니다. 필요하면 언제든 다시 설정할 수 있어요.
                </p>
                <div className="flex flex-col md:flex-row gap-2">
                  <button
                    onClick={() => {
                      setCalibrationStep(1);
                      setShowCalibrationModal(true);
                    }}
                    className="flex-1 py-2 border rounded-lg text-sm font-medium hover:bg-muted"
                  >
                    다시 설정
                  </button>
                  <button
                    onClick={saveCfoSettings}
                    disabled={isSavingCfoSettings}
                    className="flex-1 py-2 bg-primary text-white rounded-lg text-sm font-medium hover:bg-primary/90 disabled:opacity-60 flex items-center justify-center gap-2"
                  >
                    {isSavingCfoSettings && <Loader2 className="w-4 h-4 animate-spin" />}
                    {isSavingCfoSettings ? "저장 중..." : "설정 저장"}
                  </button>
                </div>
              </div>

              {/* 3. Domain Extension Store (MCP Toggles) */}
              <div className="bg-white p-6 rounded-xl border shadow-sm">
                <h3 className="text-lg font-bold flex items-center gap-2 mb-6">
                  <Settings className="w-5 h-5 text-primary" /> Domain Extension Store
                </h3>
                <div className="space-y-4">
                  {[
                    { id: 'startup', label: 'Startup Growth Pack', desc: 'R&D 세액공제, 정부지원사업, 런웨이 관리' },
                    { id: 'hospital', label: 'Medi-Tech Accounting', desc: '요양급여 청구 심사, 비급여 매출 분석' },
                    { id: 'commerce', label: 'E-Commerce Analytics', desc: '몰인몰 정산, ROAS 마케팅 효율 분석' }
                  ].map(mcp => {
                    const safeMCPs = user.activeMCPs || [];
                    const isActive = safeMCPs.includes(mcp.id);
                    return (
                      <div key={mcp.id} className="flex items-center justify-between p-4 border rounded-lg hover:bg-gray-50 transition-colors">
                        <div>
                          <div className="font-bold text-gray-900">{mcp.label}</div>
                          <div className="text-sm text-gray-500">{mcp.desc}</div>
                        </div>
                        <button
                          onClick={() => {
                            const newMCPs = isActive
                              ? safeMCPs.filter(id => id !== mcp.id)
                              : [...safeMCPs, mcp.id];
                            setUser({ ...user, activeMCPs: newMCPs });
                          }}
                          className={`relative w-12 h-6 transition-colors rounded-full ${isActive ? 'bg-primary' : 'bg-gray-300'}`}
                        >
                          <span className={`absolute left-1 top-1 w-4 h-4 bg-white rounded-full transition-transform ${isActive ? 'translate-x-6' : 'translate-x-0'}`} />
                        </button>
                      </div>
                    );
                  })}
                </div>
                <button
                  onClick={async () => {
                    if (authUser?.email) {
                      setIsSavingSettings(true);
                      try {
                        const result = await api.updateMCPs(authUser.email, user.activeMCPs || []);
                        if (result.success) {
                          showToast('MCP 설정이 저장되었습니다.', 'success');
                        }
                      } catch (e) {
                        showToast('저장 중 오류가 발생했습니다.', 'error');
                      } finally {
                        setIsSavingSettings(false);
                      }
                    }
                  }}
                  disabled={isSavingSettings}
                  className="mt-4 w-full py-2 bg-primary text-white rounded-lg font-medium hover:bg-primary/90 transition-colors flex items-center justify-center gap-2"
                >
                  {isSavingSettings && <Loader2 className="w-4 h-4 animate-spin" />}
                  {isSavingSettings ? '저장 중...' : 'MCP 설정 저장'}
                </button>
              </div>

              {/* 4. Password Change */}
              <div className="bg-white p-6 rounded-xl border shadow-sm">
                <h3 className="text-lg font-bold flex items-center gap-2 mb-6">
                  🔒 비밀번호 변경
                </h3>
                <div className="space-y-4">
                  <div>
                    <label className="text-sm font-medium text-gray-700">현재 비밀번호</label>
                    <input type="password" id="currentPassword" className="mt-1 w-full p-2 border rounded-lg" placeholder="••••••••" />
                  </div>
                  <div>
                    <label className="text-sm font-medium text-gray-700">새 비밀번호</label>
                    <input type="password" id="newPassword" className="mt-1 w-full p-2 border rounded-lg" placeholder="••••••••" />
                  </div>
                  <div>
                    <label className="text-sm font-medium text-gray-700">새 비밀번호 확인</label>
                    <input type="password" id="confirmPassword" className="mt-1 w-full p-2 border rounded-lg" placeholder="••••••••" />
                  </div>
                  <button
                    onClick={async () => {
                      const currentPw = (document.getElementById('currentPassword') as HTMLInputElement)?.value;
                      const newPw = (document.getElementById('newPassword') as HTMLInputElement)?.value;
                      const confirmPw = (document.getElementById('confirmPassword') as HTMLInputElement)?.value;

                      if (!currentPw || !newPw || !confirmPw) {
                        showToast('모든 필드를 입력해주세요.', 'error');
                        return;
                      }
                      if (newPw !== confirmPw) {
                        showToast('새 비밀번호가 일치하지 않습니다.', 'error');
                        return;
                      }
                      if (authUser?.email) {
                        try {
                          const result = await api.changePassword(authUser.email, currentPw, newPw);
                          if (result.success) {
                            showToast('비밀번호가 변경되었습니다.', 'success');
                            (document.getElementById('currentPassword') as HTMLInputElement).value = '';
                            (document.getElementById('newPassword') as HTMLInputElement).value = '';
                            (document.getElementById('confirmPassword') as HTMLInputElement).value = '';
                          } else {
                            showToast(result.message, 'error');
                          }
                        } catch (e) {
                          showToast('비밀번호 변경 중 오류가 발생했습니다.', 'error');
                        }
                      }
                    }}
                    className="w-full py-2 bg-gray-800 text-white rounded-lg font-medium hover:bg-gray-700 transition-colors"
                  >
                    비밀번호 변경
                  </button>
                </div>
              </div>

              {/* 5. Theme Toggle */}
              <div className="bg-white p-6 rounded-xl border shadow-sm">
                <h3 className="text-lg font-bold flex items-center gap-2 mb-4">
                  🎨 테마 설정
                </h3>
                <div className="flex items-center justify-between">
                  <span className="text-gray-700">다크 모드</span>
                  <button
                    onClick={() => document.documentElement.classList.toggle('dark')}
                    className="relative w-12 h-6 transition-colors rounded-full bg-gray-300"
                  >
                    <span className="absolute left-1 top-1 w-4 h-4 bg-white rounded-full transition-transform" />
                  </button>
                </div>
              </div>

              {/* 6. Data Export */}
              <div className="bg-white p-6 rounded-xl border shadow-sm">
                <h3 className="text-lg font-bold flex items-center gap-2 mb-4">
                  📊 데이터 내보내기
                </h3>
                <p className="text-sm text-gray-500 mb-4">대시보드 데이터를 CSV 파일로 다운로드합니다.</p>
                <div className="space-y-2">
                  <button
                    onClick={() => {
                      if (!effectiveDashboardData) return;
                      const kpiCsv = effectiveDashboardData.kpi?.map((k: any) => `${k.label},${k.value},${k.trend},${k.status}`).join('\n') || '';
                      const chartCsv = effectiveDashboardData.chart?.map((c: any) => `${c.name},${c.income},${c.expense}`).join('\n') || '';
                      const csv = `KPI 데이터\n라벨,값,트렌드,상태\n${kpiCsv}\n\n차트 데이터\n기간,수입,지출\n${chartCsv}`;
                      const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
                      const url = URL.createObjectURL(blob);
                      const a = document.createElement('a');
                      a.href = url;
                      a.download = `dashboard_${new Date().toISOString().slice(0, 10)}.csv`;
                      a.click();
                    }}
                    className="w-full py-2 bg-green-600 text-white rounded-lg font-medium hover:bg-green-700 transition-colors"
                  >
                    대시보드 데이터 CSV 다운로드
                  </button>
                  <button
                    onClick={() => {
                      const userData = { ...user, email: authUser?.email };
                      const json = JSON.stringify(userData, null, 2);
                      const blob = new Blob([json], { type: 'application/json' });
                      const url = URL.createObjectURL(blob);
                      const a = document.createElement('a');
                      a.href = url;
                      a.download = `user_profile_${new Date().toISOString().slice(0, 10)}.json`;
                      a.click();
                    }}
                    className="w-full py-2 border border-gray-300 text-gray-700 rounded-lg font-medium hover:bg-gray-50 transition-colors"
                  >
                    프로필 정보 JSON 다운로드
                  </button>
                </div>
              </div>

              {/* 7. NTS Electronic Document Upload */}
              <div className="bg-white p-6 rounded-xl border shadow-sm">
                <h3 className="text-lg font-bold flex items-center gap-2 mb-4">
                  📄 국세청 전자문서 업로드
                </h3>
                <p className="text-sm text-gray-500 mb-4">
                  홈택스에서 발급받은 전자문서(PDF)를 업로드하면 세무 데이터를 자동으로 추출합니다.
                </p>
                <div className="border-2 border-dashed border-gray-300 rounded-lg p-8 text-center hover:border-primary transition-colors">
                  <input
                    type="file"
                    id="ntsFileUpload"
                    accept=".pdf"
                    className="hidden"
                    onChange={async (e) => {
                      const file = e.target.files?.[0];
                      if (!file) return;

                      const resultDiv = document.getElementById('ntsResult');
                      if (resultDiv) resultDiv.innerHTML = '<p class="text-sm text-gray-500">처리 중...</p>';

                      try {
                        const result = await api.uploadNTSDocument(file);
                        if (result.success && resultDiv) {
                          const items = result.data.items || [];
                          const itemsHtml = items.slice(0, 5).map((item: any) =>
                            `<div style="display:flex;justify-content:space-between;padding:4px 0;border-bottom:1px solid #eee"><span>${item.category}</span><span style="font-weight:600">${(item.amount || 0).toLocaleString()}원</span></div>`
                          ).join('');

                          resultDiv.innerHTML = `
                            <div style="margin-top:16px;padding:16px;background:#f0fdf4;border:1px solid #bbf7d0;border-radius:8px;text-align:left">
                              <div style="display:flex;align-items:center;gap:8px;margin-bottom:12px">
                                <span style="font-size:24px">✅</span>
                                <span style="font-weight:700;color:#166534">전자문서 검증 성공!</span>
                              </div>
                              <div style="font-size:14px;color:#374151">
                                <p><strong>문서 유형:</strong> ${result.data.document_type || '알 수 없음'}</p>
                                <p><strong>페이지 수:</strong> ${result.verification.page_count || 1}페이지</p>
                              </div>
                              ${items.length > 0 ? `<div style="margin-top:12px;padding-top:12px;border-top:1px solid #bbf7d0">${itemsHtml}</div>` : ''}
                              ${result.data.total_amount ? `<p style="margin-top:8px;font-weight:700">합계: ${result.data.total_amount.toLocaleString()}원</p>` : ''}
                            </div>
                          `;
                          console.log("NTS Document Data:", result.data);
                        } else if (resultDiv) {
                          resultDiv.innerHTML = `<div style="margin-top:16px;padding:16px;background:#fef2f2;border:1px solid #fecaca;border-radius:8px"><span style="color:#dc2626">❌ ${result.message}</span></div>`;
                        }
                      } catch (err) {
                        if (resultDiv) resultDiv.innerHTML = `<div style="margin-top:16px;padding:16px;background:#fef2f2;border:1px solid #fecaca;border-radius:8px"><span style="color:#dc2626">❌ 업로드 오류</span></div>`;
                      }
                    }}
                  />
                  <label htmlFor="ntsFileUpload" className="cursor-pointer">
                    <div className="text-4xl mb-2">📤</div>
                    <p className="font-medium text-gray-700">클릭하여 파일 선택</p>
                    <p className="text-xs text-gray-400 mt-1">PDF 파일만 가능</p>
                  </label>
                </div>
                <div id="ntsResult"></div>
                <div className="mt-4 text-xs text-gray-400">
                  <div className="flex items-center justify-between mb-2">
                    <span>지원 문서 유형</span>
                    <button
                      onClick={fetchNtsDocTypes}
                      disabled={ntsDocTypesLoading}
                      className="text-[10px] px-2 py-1 rounded border hover:bg-muted disabled:opacity-60"
                    >
                      {ntsDocTypesLoading ? "불러오는 중..." : "새로고침"}
                    </button>
                  </div>
                  {ntsDocTypes.length > 0 ? (
                    <div className="flex flex-wrap gap-1">
                      {ntsDocTypes.map((doc) => (
                        <span
                          key={doc.code}
                          className={`text-[10px] px-2 py-1 rounded border ${doc.supported ? "bg-emerald-50 text-emerald-700 border-emerald-200" : "bg-gray-50 text-gray-500 border-gray-200"}`}
                        >
                          {doc.name}
                        </span>
                      ))}
                    </div>
                  ) : (
                    <p>지원 목록을 불러오지 못했습니다.</p>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>
      </main>

      {
        showRejectionModal && (
          <div className="fixed inset-0 z-50">
            <button
              onClick={() => setShowRejectionModal(false)}
              className="absolute inset-0 bg-black/30"
              aria-label="거부 사유 입력 닫기"
            />
            <div className="absolute inset-x-4 top-24 mx-auto max-w-lg bg-white rounded-xl shadow-2xl border">
              <div className="p-4 border-b flex items-center justify-between">
                <div>
                  <h2 className="font-semibold">거부 사유 입력</h2>
                  <p className="text-xs text-muted-foreground">
                    {tonePreference === "direct"
                      ? "명확한 이유가 필요합니다."
                      : tonePreference === "soft"
                        ? "간단히 이유를 남겨주셔도 괜찮습니다."
                        : "AI가 기억할 이유를 남겨 주세요."}
                  </p>
                </div>
                <button onClick={() => setShowRejectionModal(false)} className="p-2 hover:bg-muted rounded-lg">
                  <X className="w-4 h-4" />
                </button>
              </div>
              <div className="p-4 space-y-3">
                <textarea
                  className="w-full min-h-[120px] resize-none rounded-lg border px-3 py-2 text-sm focus:ring-2 ring-primary/20 outline-none"
                  placeholder="예: 현재 채용을 진행해야 하는 사업상 이유가 있어요."
                  value={rejectionInput}
                  onChange={(e) => setRejectionInput(e.target.value)}
                />
                <div className="flex flex-wrap gap-2">
                  {(tonePreference === "direct"
                    ? [
                      "채용 필요",
                      "매출 확장이 우선",
                      "투자 확정",
                      "운영 인력 부족",
                    ]
                    : tonePreference === "soft"
                      ? [
                        "채용이 필요한 상황이에요",
                        "매출 확장이 더 중요해 보여요",
                        "투자 유치가 확정되었습니다",
                        "운영 인력이 부족한 편입니다",
                      ]
                      : [
                        "지금 매출 성장 구간이라 채용이 필요함",
                        "리스크보다 매출 확대가 우선임",
                        "외부 투자 유치가 확정됨",
                        "운영 인력이 부족함",
                      ]
                  ).map((reason) => (
                    <button
                      key={reason}
                      onClick={() => setRejectionInput(reason)}
                      className="px-3 py-1.5 rounded-full text-xs border hover:bg-muted"
                    >
                      {reason}
                    </button>
                  ))}
                </div>
              </div>
              <div className="p-4 border-t flex justify-end gap-2">
                <button
                  onClick={() => setShowRejectionModal(false)}
                  className="px-4 py-2 rounded-lg border text-sm hover:bg-muted"
                >
                  취소
                </button>
                <button
                  onClick={() => {
                    recordDecision("rejected", rejectionInput.trim() || "사유 미입력");
                    setShowRejectionModal(false);
                  }}
                  className="px-4 py-2 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90"
                >
                  거부 사유 저장
                </button>
              </div>
            </div>
          </div>
        )
      }

      {
        showOutcomeModal && outcomeTarget && (
          <div className="fixed inset-0 z-50">
            <button
              onClick={() => setShowOutcomeModal(false)}
              className="absolute inset-0 bg-black/30"
              aria-label="결과 입력 닫기"
            />
            <div className="absolute inset-x-4 top-24 mx-auto max-w-lg bg-white rounded-xl shadow-2xl border">
              <div className="p-4 border-b flex items-center justify-between">
                <div>
                  <h2 className="font-semibold">결과 기록</h2>
                  <p className="text-xs text-muted-foreground">{outcomeTarget.title}</p>
                </div>
                <button onClick={() => setShowOutcomeModal(false)} className="p-2 hover:bg-muted rounded-lg">
                  <X className="w-4 h-4" />
                </button>
              </div>
              <div className="p-4 space-y-3">
                <div className="text-xs text-muted-foreground">이번 결정의 실제 결과는 어땠나요?</div>
                <div className="flex gap-2">
                  {[
                    { key: "positive", label: "긍정적" },
                    { key: "neutral", label: "중립" },
                    { key: "negative", label: "부정적" },
                  ].map((item) => (
                    <button
                      key={item.key}
                      onClick={() => setOutcomeStatus(item.key as "positive" | "neutral" | "negative")}
                      className={`flex-1 py-2 rounded-lg text-sm border ${outcomeStatus === item.key ? "bg-primary text-primary-foreground border-primary" : "hover:bg-muted"}`}
                    >
                      {item.label}
                    </button>
                  ))}
                </div>
                <div className="mt-4">
                  <div className="text-xs text-muted-foreground mb-1">상세 메모 (선택사항)</div>
                  <textarea
                    className="w-full border rounded-lg p-2 text-sm h-20 resize-none focus:ring-2 focus:ring-primary/20 outline-none"
                    placeholder="결과에 대한 간단한 코멘트를 남겨주세요..."
                    value={outcomeMemo}
                    onChange={(e) => setOutcomeMemo(e.target.value)}
                  />
                </div>
              </div>
              <div className="p-4 border-t flex justify-end gap-2">
                <button
                  onClick={() => setShowOutcomeModal(false)}
                  className="px-4 py-2 rounded-lg border text-sm hover:bg-muted"
                >
                  취소
                </button>
                <button
                  onClick={() => {
                    const updated = { ...outcomeTarget, outcomeStatus, outcomeMemo };
                    setDecisionHistory((prev) => prev.map((item) => item.id === updated.id ? updated : item));
                    if (user?.bizNum) {
                      api.updateMemoryOutcome(user.bizNum, updated.id, outcomeStatus, outcomeMemo).catch((err) => {
                        console.error("Failed to update outcome:", err);
                      });
                    }
                    setShowOutcomeModal(false);
                  }}
                  className="px-4 py-2 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90"
                >
                  결과 저장
                </button>
              </div>
            </div>
          </div>
        )
      }

      {
        isQuestionOpen && (
          <div className="fixed inset-0 z-50">
            <button
              onClick={() => setIsQuestionOpen(false)}
              className="absolute inset-0 bg-black/30"
              aria-label="질문 패널 닫기"
            />
            <div className="absolute right-0 top-0 h-full w-full max-w-md bg-white shadow-2xl flex flex-col">
              <div className="p-4 border-b flex items-center justify-between">
                <div>
                  <h2 className="font-semibold">질문 패널</h2>
                  <p className="text-xs text-muted-foreground">
                    {tonePreference === "direct"
                      ? "핵심만 빠르게 묻고 답합니다."
                      : tonePreference === "soft"
                        ? "필요한 만큼만 조심스럽게 묻겠습니다."
                        : "결정에 필요한 질문만 빠르게."}
                  </p>
                </div>
                <button onClick={() => setIsQuestionOpen(false)} className="p-2 hover:bg-muted rounded-lg">
                  <X className="w-4 h-4" />
                </button>
              </div>

              <div className="p-4 border-b space-y-3">
                <div className="flex flex-wrap gap-2">
                  {(tonePreference === "direct"
                    ? [
                      "이번 달 핵심 리스크만",
                      "부가세 영향만 말해줘",
                      "증빙 누락 위치만",
                    ]
                    : tonePreference === "soft"
                      ? [
                        "이번 달 위험한 부분이 있을까요?",
                        "부가세 영향이 있을지 궁금해요",
                        "증빙 누락이 있을까요?",
                      ]
                      : [
                        "이번 달 가장 위험한 건?",
                        "부가세 영향은?",
                        "증빙 누락 어디?",
                      ]
                  ).map((q) => (
                    <button
                      key={q}
                      onClick={() => handleSend(q)}
                      className="px-3 py-1.5 rounded-full text-xs bg-muted hover:bg-muted/70"
                    >
                      {q}
                    </button>
                  ))}
                </div>
                <div className="flex flex-wrap gap-2">
                  {[
                    "자세히 보기",
                    "근거 펼치기",
                    "시뮬레이션 요청",
                  ].map((q) => (
                    <button
                      key={q}
                      onClick={() => handleSend(q)}
                      className="px-3 py-1.5 rounded-full text-xs border hover:bg-muted"
                    >
                      {q}
                    </button>
                  ))}
                </div>
                <div className="text-xs text-muted-foreground">
                  현재 판단: {decisionContext.summary} (우선순위 {decisionContext.priorityScore}/100)
                </div>
                <button
                  onClick={openRelatedScreen}
                  className="w-full px-3 py-2 rounded-lg border text-xs font-medium hover:bg-muted"
                >
                  관련 화면 열기
                </button>
              </div>

              <div className="flex-1 overflow-y-auto p-4 space-y-4">
                {messages.map((m, i) => (
                  <div
                    key={i}
                    className={`flex ${m.role === "user" ? "justify-end" : "justify-start"}`}
                  >
                    <div
                      className={`max-w-[85%] rounded-2xl px-4 py-3 ${m.role === "user"
                        ? "bg-primary text-primary-foreground"
                        : "bg-muted"
                        }`}
                    >
                      {/* Markdown Rendering Support */}
                      <div className={`text-sm leading-relaxed ${m.role === 'user' ? 'text-white' : 'text-gray-900 markdown-body'}`}>
                        {m.role === 'user' ? (
                          <p className="whitespace-pre-wrap">{m.content}</p>
                        ) : (
                          <ReactMarkdown remarkPlugins={[remarkGfm]} components={{
                            ul: ({ node, ...props }) => <ul className="list-disc pl-4 my-2" {...props} />,
                            ol: ({ node, ...props }) => <ol className="list-decimal pl-4 my-2" {...props} />,
                            li: ({ node, ...props }) => <li className="mb-1" {...props} />,
                            p: ({ node, ...props }) => <p className="mb-2 last:mb-0" {...props} />,
                            a: ({ node, ...props }) => <a className="text-blue-600 underline hover:text-blue-800" target="_blank" rel="noopener noreferrer" {...props} />,
                            strong: ({ node, ...props }) => <strong className="font-semibold" {...props} />,
                            table: ({ node, ...props }) => <div className="overflow-x-auto my-2"><table className="min-w-full divide-y divide-gray-200 border" {...props} /></div>,
                            th: ({ node, ...props }) => <th className="px-3 py-2 bg-gray-50 text-left text-xs font-medium text-gray-500 uppercase tracking-wider border-b" {...props} />,
                            td: ({ node, ...props }) => <td className="px-3 py-2 whitespace-nowrap text-sm border-b" {...props} />,
                          }}>
                            {m.content}
                          </ReactMarkdown>
                        )}
                      </div>
                      {m.role === "assistant" && typeof m.latencyMs === "number" && (
                        <div className="mt-2 text-[10px] text-muted-foreground">
                          응답 시간: {(m.latencyMs / 1000).toFixed(1)}s
                        </div>
                      )}
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
                    <div className="bg-gradient-to-r from-blue-50 to-indigo-50 border border-blue-100 rounded-2xl px-4 py-3 flex items-center gap-3 shadow-sm">
                      <div className="w-6 h-6 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
                      <div className="flex flex-col">
                        <span className="text-sm font-medium text-blue-800">{getLoadingMessage()}</span>
                        {loadingElapsed >= 5 && (
                          <span className="text-xs text-blue-500">{loadingElapsed}초 경과</span>
                        )}
                      </div>
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
                    placeholder="예: 법인세율이 어떻게 되나요?"
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
          </div>
        )
      }
      {
        showCalibrationModal && (
          <div className="fixed inset-0 z-[70] bg-black/80 flex items-center justify-center animate-in fade-in duration-300 backdrop-blur-sm">
            <div className="bg-white rounded-2xl shadow-2xl w-full max-w-4xl p-8 animate-in zoom-in-95 relative overflow-hidden flex flex-col h-[600px]">
              <div className="absolute top-0 left-0 w-full h-2 bg-muted">
                <div
                  className="h-full bg-primary transition-all duration-500"
                  style={{ width: `${(calibrationStep / 3) * 100}%` }}
                />
              </div>

              <div className="mt-4 mb-8 text-center">
                <h2 className="text-2xl font-bold mb-2">
                  {calibrationStep === 1 ? "AI CFO 성향 설정" : calibrationStep === 2 ? "가치관 파악 시뮬레이션" : "핵심 목표 설정"}
                </h2>
                <p className="text-muted-foreground">
                  {calibrationStep === 1
                    ? "대표님과 가장 잘 맞는 소통 방식을 선택해주세요."
                    : calibrationStep === 2
                      ? "다음 상황에서 어떤 결정을 내리시겠습니까?"
                      : "우리 회사가 현재 가장 중요하게 생각하는 가치는 무엇인가요?"}
                </p>
              </div>

              <div className="min-h-[300px] flex flex-col justify-center">
                {calibrationStep === 1 && (
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-8 h-full">
                    <div className="space-y-4">
                      {[
                        { key: "direct", label: "직설적/명확함", desc: "결론부터 빠르게, 리스크는 강하게 경고", icon: "⚡️" },
                        { key: "neutral", label: "객관적/분석", desc: "데이터 중심, 감정 없이 팩트만 전달", icon: "📊" },
                        { key: "soft", label: "완곡/제안", desc: "부드러운 어조, 대안 중심으로 제안", icon: "🤝" }
                      ].map((opt) => (
                        <button
                          key={opt.key}
                          onClick={() => setTonePreference(opt.key as any)}
                          className={`w-full p-4 rounded-xl border-2 text-left transition-all ${tonePreference === opt.key ? "border-primary bg-primary/5 ring-2 ring-primary/20" : "border-muted hover:border-primary/50"}`}
                        >
                          <div className="flex items-center gap-3">
                            <span className="text-2xl">{opt.icon}</span>
                            <div>
                              <div className="font-bold text-lg">{opt.label}</div>
                              <div className="text-xs text-muted-foreground">{opt.desc}</div>
                            </div>
                          </div>
                        </button>
                      ))}
                      <button
                        onClick={() => setCalibrationStep(2)}
                        className="w-full py-3 mt-4 bg-primary text-primary-foreground font-bold rounded-xl hover:bg-primary/90 transition-all"
                      >
                        선택 완료 및 다음
                      </button>
                    </div>

                    <div className="bg-gray-100 rounded-xl p-4 flex flex-col border shadow-inner">
                      <div className="text-xs text-center text-muted-foreground mb-4">💬 실시간 AI 답변 미리보기</div>
                      <div className="flex-1 space-y-4 overflow-y-auto">
                        <div className="flex justify-end">
                          <div className="bg-primary text-primary-foreground rounded-2xl rounded-tr-none px-4 py-2 text-sm max-w-[80%]">
                            지금 Burn Rate가 너무 높은 것 같아요. 어떻게 할까요?
                          </div>
                        </div>
                        <div className="flex justify-start">
                          <div className="bg-white border rounded-2xl rounded-tl-none px-4 py-3 text-sm shadow-sm max-w-[90%]">
                            {tonePreference === "direct" && (
                              <div>
                                <p className="font-bold text-red-600 mb-1">⚠️ 경고: 즉시 지출 축소가 필요합니다.</p>
                                <p>현재 Burn Rate로는  runway가 3개월 미만입니다. 마케팅 비용을 50% 삭감하고 고정비를 재조정하십시오.</p>
                              </div>
                            )}
                            {tonePreference === "neutral" && (
                              <div>
                                <p className="font-semibold mb-1">📊 현재 Burn Rate 분석 결과</p>
                                <p>전월 대비 15% 상승했습니다. 이 추세라면 4개월 후 자금이 소진됩니다. 예산 재배정이 권장됩니다.</p>
                              </div>
                            )}
                            {tonePreference === "soft" && (
                              <div>
                                <p className="font-semibold mb-1 text-emerald-700">💡 예산 조정이 필요해 보입니다.</p>
                                <p>현재 지출 속도가 조금 빠른 편이에요. 마케팅 예산을 조금만 줄여서 Runway를 확보하는 건 어떨까요?</p>
                              </div>
                            )}
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                )}

                {calibrationStep === 2 && (
                  <div className="space-y-6">
                    <div className="bg-muted/30 p-6 rounded-xl border">
                      <h4 className="font-semibold mb-2 flex items-center gap-2">
                        <span className="bg-blue-100 text-blue-700 px-2 py-1 rounded text-xs">Scenario #1</span>
                        투자 유치 직후의 자금 운용
                      </h4>
                      <p className="text-sm leading-relaxed">
                        최근 5억원의 시드 투자를 유치했습니다. <br />
                        하지만 현재 개발팀 인력이 부족하여 제품 출시가 지연되고 있습니다. <br />
                        동시에 마케팅을 시작하지 않으면 초기 유저 확보가 어려울 것으로 보입니다.
                      </p>
                    </div>
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                      <button
                        onClick={() => {
                          const seedDecision: DecisionRecord = {
                            id: `seed-growth-${Date.now()}`,
                            title: "초기 자금 운용 전략 (Calibration)",
                            summary: "공격적인 인재 채용 및 마케팅 집행",
                            reasons: ["시장 선점이 최우선 과제", "투자금 기반의 빠른 스케일업 필요"],
                            impact: "Burn Rate 급증하지만 점유율 확대 기대",
                            priorityScore: 90,
                            riskScore: 70,
                            runwayMonths: 12,
                            outcomeStatus: "positive",
                            outcomeMemo: "초기 성장을 위해 리스크를 감수하는 성향 확인됨",
                            status: "accepted",
                            createdAt: new Date().toISOString(),
                            riskLevel: "warning",
                            drivers: [],
                            relatedTab: "financial",
                            actionKey: "seed-growth"
                          };
                          setDecisionHistory([seedDecision]);
                          api.saveDecisionMemory(mapDecisionToMemory(seedDecision, user?.bizNum || "temp")).catch(console.error);
                          setCalibrationStep(3);
                        }}
                        className="p-5 border rounded-xl hover:bg-muted text-left"
                      >
                        <div className="font-bold mb-1">공격적 투자 (Growth)</div>
                        <div className="text-sm text-muted-foreground">Runway가 줄더라도 인재 채용과 마케팅에 자금을 집중하여 시장을 선점합니다.</div>
                      </button>
                      <button
                        onClick={() => {
                          const seedDecision: DecisionRecord = {
                            id: `seed-stable-${Date.now()}`,
                            title: "초기 자금 운용 전략 (Calibration)",
                            summary: "최소 인력 유지 및 제품 내실화",
                            reasons: ["재무 안정성 확보가 최우선", "PMF 검증 후 마케팅 집행"],
                            impact: "성장 속도는 느리지만 Runway 24개월 확보",
                            priorityScore: 60,
                            riskScore: 30,
                            runwayMonths: 24,
                            outcomeStatus: "positive",
                            outcomeMemo: "재무 안정성을 중시하고 보수적으로 접근하는 성향 확인됨",
                            status: "accepted",
                            createdAt: new Date().toISOString(),
                            riskLevel: "safe",
                            drivers: [],
                            relatedTab: "financial",
                            actionKey: "seed-stable"
                          };
                          setDecisionHistory([seedDecision]);
                          api.saveDecisionMemory(mapDecisionToMemory(seedDecision, user?.bizNum || "temp")).catch(console.error);
                          setCalibrationStep(3);
                        }}
                        className="p-5 border rounded-xl hover:bg-muted text-left"
                      >
                        <div className="font-bold mb-1">안정적 운용 (Profit/Stability)</div>
                        <div className="text-sm text-muted-foreground">최소한의 핵심 인력으로 제품을 고도화하며 현금을 최대한 보존합니다.</div>
                      </button>
                    </div>
                  </div>
                )}

                {calibrationStep === 3 && (
                  <div className="space-y-8 max-w-2xl mx-auto w-full">
                    <div className="space-y-6">
                      {[
                        {
                          key: "priority",
                          label: "우선순위 (Priority)",
                          sub: "업무 처리 순서",
                          vals: ["모든 업무 동일", "핵심 업무 위주", "긴급 건 최우선", "전략적 우선순위", "생존 직결 과제"]
                        },
                        {
                          key: "runway",
                          label: "런웨이 (Runway)",
                          sub: "현금 흐름 관리",
                          vals: ["공격적 투자", "성장 중심", "균형 유지", "보수적 관리", "생존 모드"]
                        },
                        {
                          key: "risk",
                          label: "리스크 (Risk)",
                          sub: "법적/세무 위험",
                          vals: ["리스크 감수", "유연한 대처", "일반적 관리", "엄격한 관리", "Zero Risk"]
                        }
                      ].map((item) => (
                        <div key={item.key} className="bg-muted/20 p-6 rounded-xl border">
                          <div className="flex justify-between items-end mb-4">
                            <div>
                              <div className="font-bold text-lg">{item.label}</div>
                              <div className="text-xs text-muted-foreground">{item.sub}</div>
                            </div>
                            <div className="text-sm font-semibold text-primary">
                              {item.vals[((similarityWeights as any)[item.key] || 1) - 1]}
                            </div>
                          </div>
                          <input
                            type="range"
                            min="1"
                            max="5"
                            step="1"
                            value={(similarityWeights as any)[item.key]}
                            onChange={(e) => setSimilarityWeights(prev => ({ ...prev, [item.key]: Number(e.target.value) }))}
                            className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-primary"
                          />
                          <div className="flex justify-between text-[10px] text-muted-foreground mt-2">
                            <span>Low</span>
                            <span>High</span>
                          </div>
                        </div>
                      ))}
                    </div>
                    <button
                      onClick={() => {
                        setIsCalibrating(true);
                        setTimeout(() => {
                          setIsCalibrating(false);
                          setShowCalibrationModal(false);
                        }, 2000);
                      }}
                      disabled={isCalibrating}
                      className="w-full py-4 bg-primary text-primary-foreground text-lg font-bold rounded-xl hover:bg-primary/90 shadow-lg transition-all hover:scale-[1.02] flex items-center justify-center gap-2"
                    >
                      {isCalibrating ? (
                        <>
                          <Loader2 className="w-5 h-5 animate-spin" />
                          AI CFO 최적화 적용 중...
                        </>
                      ) : (
                        "설정 완료 및 AI CFO 시작하기"
                      )}
                    </button>
                  </div>
                )}
              </div>

              <div className="mt-8 flex justify-center gap-2">
                {[1, 2, 3].map(step => (
                  <div key={step} className={`w-2 h-2 rounded-full ${calibrationStep === step ? "bg-primary" : "bg-muted"}`} />
                ))}
              </div>
            </div>
          </div>
        )
      }
      {
        showRejectionModal && (
          <div className="fixed inset-0 z-[60] bg-black/50 flex items-center justify-center animate-in fade-in duration-200">
            <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6 animate-in zoom-in-95">
              <h3 className="font-bold text-lg mb-2">판단 거부 사유 입력</h3>
              <p className="text-sm text-gray-500 mb-4">
                거부 사유를 입력해주시면 AI CFO가 학습하여 다음 제안에 반영합니다.
              </p>
              <textarea
                autoFocus
                className="w-full h-32 p-3 border rounded-lg resize-none focus:ring-2 focus:ring-primary/20 outline-none mb-4 text-sm"
                placeholder="예: 지금은 채용이 급해서 리스크를 감수하겠습니다."
                value={rejectionInput}
                onChange={(e) => setRejectionInput(e.target.value)}
              />
              <div className="flex justify-end gap-2">
                <button
                  onClick={() => setShowRejectionModal(false)}
                  className="px-4 py-2 text-sm text-gray-600 hover:bg-gray-100 rounded-lg"
                >
                  취소
                </button>
                <button
                  onClick={() => {
                    recordDecision("rejected", rejectionInput);
                    setShowRejectionModal(false);
                  }}
                  className="px-4 py-2 text-sm bg-red-600 text-white hover:bg-red-700 rounded-lg font-medium"
                >
                  거부 확정
                </button>
              </div>
            </div>
          </div>
        )
      }

      {/* Preset Preview Modal */}
      {presetPreview && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
          <button className="absolute inset-0 bg-black/40" onClick={() => setPresetPreview(null)} aria-label="프리셋 미리보기 닫기" />
          <div className="relative bg-white rounded-2xl shadow-2xl w-full max-w-3xl overflow-hidden">
            <div className="p-6 border-b flex items-start justify-between gap-4">
              <div>
                <div className="text-xs text-muted-foreground">{presetPreview.name}</div>
                <h3 className="text-xl font-bold">{presetPreview.persona}</h3>
                <p className="text-sm text-muted-foreground mt-1">{presetPreview.summary}</p>
              </div>
              <div className="flex items-center gap-2">
                {presetPreview.badge && <span className="text-xs bg-primary/10 text-primary px-2 py-1 rounded-full">{presetPreview.badge}</span>}
                <button onClick={() => setPresetPreview(null)} className="p-2 rounded-full hover:bg-muted"><X className="w-4 h-4" /></button>
              </div>
            </div>
            <div className="p-6 space-y-6">
              <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
                <div className="p-3 rounded-lg border bg-muted/30"><div className="text-xs text-muted-foreground">현금</div><div className="text-lg font-semibold">₩{presetPreview.meta.cash.toLocaleString()}</div></div>
                <div className="p-3 rounded-lg border bg-muted/30"><div className="text-xs text-muted-foreground">월 매출</div><div className="text-lg font-semibold">₩{presetPreview.meta.monthlyRevenue.toLocaleString()}</div></div>
                <div className="p-3 rounded-lg border bg-muted/30"><div className="text-xs text-muted-foreground">월 지출</div><div className="text-lg font-semibold">₩{presetPreview.meta.monthlyExpense.toLocaleString()}</div></div>
                <div className="p-3 rounded-lg border bg-muted/30"><div className="text-xs text-muted-foreground">Runway</div><div className="text-lg font-semibold">{presetPreview.meta.monthlyExpense > 0 ? (presetPreview.meta.cash / presetPreview.meta.monthlyExpense).toFixed(1) : "∞"}개월</div></div>
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="p-4 rounded-xl border">
                  <h4 className="text-sm font-semibold mb-2">연간 요약</h4>
                  <div className="text-sm text-muted-foreground space-y-1">
                    <div>연 매출: ₩{(presetPreview.meta.monthlyRevenue * 12).toLocaleString()}</div>
                    <div>연 지출: ₩{(presetPreview.meta.monthlyExpense * 12).toLocaleString()}</div>
                    <div>연 순이익: ₩{((presetPreview.meta.monthlyRevenue - presetPreview.meta.monthlyExpense) * 12).toLocaleString()}</div>
                  </div>
                </div>
                <div className="p-4 rounded-xl border">
                  <h4 className="text-sm font-semibold mb-2">현재 데이터셋 대비</h4>
                  {activeDatasetName ? (
                    <div className="text-sm text-muted-foreground space-y-1">
                      <div>현금: {formatDelta(presetPreview.meta.cash - activeDatasetMetrics.cash, "원")}</div>
                      <div>월 매출: {formatDelta(presetPreview.meta.monthlyRevenue - activeDatasetMetrics.monthlyRevenue, "원")}</div>
                      <div>월 지출: {formatDelta(presetPreview.meta.monthlyExpense - activeDatasetMetrics.monthlyExpense, "원")}</div>
                      <div>Runway: {formatDelta((presetPreview.meta.monthlyExpense > 0 ? presetPreview.meta.cash / presetPreview.meta.monthlyExpense : 0) - (activeDatasetMetrics.monthlyExpense > 0 ? activeDatasetMetrics.cash / activeDatasetMetrics.monthlyExpense : 0), "개월")}</div>
                    </div>
                  ) : (
                    <div className="text-sm text-muted-foreground">현재 활성 데이터셋이 없습니다.</div>
                  )}
                </div>
              </div>
            </div>
            <div className="p-6 border-t flex items-center justify-end gap-2">
              <button onClick={() => setPresetPreview(null)} className="px-4 py-2 rounded-lg border text-sm hover:bg-muted">닫기</button>
              <button onClick={() => { applyPresetDataset(presetPreview); setPresetPreview(null); }} className="px-4 py-2 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90">바로 적용</button>
            </div>
          </div>
        </div>
      )}

      {/* GLOBAL TOAST */}
      {
        toast && (
          <div className="fixed bottom-6 right-6 z-[100] animate-in slide-in-from-bottom-5 fade-in duration-300">
            <div className={`px-4 py-3 rounded-xl shadow-2xl border flex items-center gap-3 ${toast.type === 'success' ? 'bg-emerald-50 border-emerald-200 text-emerald-800' :
              toast.type === 'error' ? 'bg-red-50 border-red-200 text-red-800' :
                'bg-white border-gray-200 text-gray-800'
              }`}>
              {toast.type === 'success' && <CheckCircle className="w-5 h-5" />}
              {toast.type === 'error' && <AlertTriangle className="w-5 h-5" />}
              {toast.type === 'info' && <Bell className="w-5 h-5" />}
              <span className="font-medium text-sm">{toast.message}</span>
            </div>
          </div>
        )
      }
    </div >
  );
}

export default App;
