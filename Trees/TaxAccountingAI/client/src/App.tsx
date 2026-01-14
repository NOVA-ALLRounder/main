import { useState, useEffect, useCallback } from "react";
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
  Building
} from "lucide-react";
import {
  api,
  type ChatMessage,
  type Recommendation,
  type Competition,
  type RiskAnalysis,
  type CalendarAlert
} from "./services/api";
import { LoginPage } from "./components/LoginPage";

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
                <div className="flex items-center justify-between w-full">
                  <div className="flex items-center gap-3">
                    <div
                      onClick={(e) => toggleCheck(i, e)}
                      className={`w-5 h-5 rounded border-2 flex items-center justify-center transition-colors flex-shrink-0 ${checkedItems.has(i) ? 'bg-primary border-primary' : 'border-gray-300 group-hover:border-primary'}`}
                    >
                      {checkedItems.has(i) && <Check className="w-3.5 h-3.5 text-white" />}
                    </div>
                    <div>
                      <div className="text-sm font-bold text-gray-800">{item.task}</div>
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
        {items.map((item) => (
          <div
            key={item.key}
            onClick={() => toggle(item.key)}
            className={`p-3 rounded-lg border cursor-pointer transition-colors flex items-center gap-3 ${toggles[item.key] ? 'bg-primary/5 border-primary' : 'hover:bg-muted/50'}`}
          >
            <div className={`w-5 h-5 rounded border flex items-center justify-center flex-shrink-0 ${toggles[item.key] ? 'bg-primary border-primary text-white' : 'border-gray-400'}`}>
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
              <p className="text-[10px] text-gray-400 mt-0.5">{item.legal}</p>
            </div>
          </div>
        ))}
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

  useEffect(() => {
    fetchAnalysis();
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
    </div>
  );
};

// 5. Business Lookup Component
const BusinessLookup = () => {
  const [bizNum, setBizNum] = useState('');
  const [result, setResult] = useState<any>(null);
  const [loading, setLoading] = useState(false);

  const handleLookup = async () => {
    if (bizNum.replace(/-/g, '').length !== 10) {
      alert('사업자등록번호 10자리를 입력하세요');
      return;
    }
    setLoading(true);
    try {
      const res = await api.lookupBusiness(bizNum);
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
            <p className="text-xs text-muted-foreground mt-2 border-t pt-2">
              ⚠️ 테스트 데이터입니다. 실제 데이터는 DATA_GO_KR_API_KEY 환경변수 설정 필요.
            </p>
          )}
        </div>
      )}
    </div>
  );
};

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

export default function App() {
  const [activeTab, setActiveTab] = useState<string>("dashboard");
  const [dashboardTab, setDashboardTab] = useState<"home" | "accounting" | "management">("home");
  const [isSidebarOpen, setIsSidebarOpen] = useState(true); // Keep existing isSidebarOpen
  const [isLoading, setIsLoading] = useState(true); // New general isLoading

  // -- Tax/Accounting Feature States --
  const [vatRevenue, setVatRevenue] = useState(0);
  const [vatPurchase, setVatPurchase] = useState(0);
  // Simple VAT Calculation: (Revenue * 10%) - (Purchase * 10%)
  const estimatedVat = Math.max(0, (vatRevenue * 0.1) - (vatPurchase * 0.1));

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

  const [showCompetitorCompare, setShowCompetitorCompare] = useState(false);
  // -----------------------------------
  // -----------------------------------

  // User State (Auth)


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
  const [user, setUser] = useState<{
    name: string;
    company: string;
    bizNum: string;
    type: string;
    targetRevenue?: number;
    activeMCPs: string[];
    rfiData: any;
  } | null>(null);

  const [input, setInput] = useState("");
  const [showNotifications, setShowNotifications] = useState(false);

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
          alert("서버 연결에 실패하여 데모 모드로 전환됩니다. (백엔드 실행 여부를 확인하세요)");
        });
      api.getRecommendations(user.type || 'startup').then(setRecommendations).catch(console.error);
      api.getCompetitions().then(setCompetitions).catch(console.error);
      // Advanced
      api.getTaxRisk(user.bizNum, safeMCPs).then(setRisk).catch(console.error);
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
    console.log("Restoring user state check:", { authUser, user });
    if (authUser && authUser.onboarding_completed && authUser.biz_num && !user) {
      console.log("Restoring user state now...");
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
          {/* Section 1: CFO Core */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 px-3 tracking-wider">CFO CORE</div>
            <div className="space-y-1">
              <SidebarItem icon={LayoutDashboard} label="재무 대시보드" active={activeTab === "dashboard"} onClick={() => setActiveTab("dashboard")} />
              <SidebarItem icon={AlertTriangle} label="세무 리스크" active={activeTab === "risk"} onClick={() => setActiveTab("risk")} />
              <SidebarItem icon={Calculator} label="회계/증빙 관리" active={activeTab === "accounting"} onClick={() => setActiveTab("accounting")} />
              <SidebarItem icon={MessageSquare} label="AI 자문 (Domain Specific)" active={activeTab === "chat"} onClick={() => setActiveTab("chat")} />
            </div>
          </div>

          {/* Section 2: Domain MCPs */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 px-3 tracking-wider flex justify-between items-center">
              <span>DOMAIN EXTENSIONS</span>
              <span className="text-[10px] bg-primary/10 text-primary px-1.5 py-0.5 rounded">ON</span>
            </div>
            <div className="space-y-1">
              {(user.activeMCPs || []).includes('startup') && (
                <>
                  <SidebarItem icon={Rocket} label="R&D / 정부지원" active={activeTab === "competitions"} onClick={() => setActiveTab("competitions")} />
                  <SidebarItem icon={TrendingUp} label="Runway / Burn Rate" active={activeTab === "runway"} onClick={() => setActiveTab("runway")} />
                </>
              )}
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
            {activeTab === 'dashboard' ? `${user.type.toUpperCase()} Dashboard` : activeTab === 'chat' ? 'AI CFO Chat' : 'Domain Intelligence'}
          </h1>
          <div className="flex items-center gap-4 relative">
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
          {activeTab === "dashboard" && !dashboardData && (
            <div className="max-w-6xl mx-auto flex items-center justify-center py-20">
              <div className="text-center">
                <div className="mx-auto w-12 h-12 border-4 border-primary border-t-transparent rounded-full animate-spin mb-4" />
                <p className="text-muted-foreground">데이터를 불러오는 중...</p>
              </div>
            </div>
          )}
          {activeTab === "dashboard" && dashboardData && (
            <div className="max-w-7xl mx-auto space-y-6">

              {/* Tab Navigation */}
              {/* Tab Navigation - Full Width Sticky Header */}
              {/* Tab Navigation - Full Width Sticky Header */}
              {/* Tab Navigation - Full Width Sticky Header */}
              {/* Tab Navigation - Static Header (Scrolls with page) */}
              <div className="-mx-6 px-6 py-2 bg-background border-y mb-8 shadow-sm">
                <div className="max-w-7xl mx-auto flex flex-wrap gap-2">
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
              </div>

              {/* === HOME TAB === */}
              {dashboardTab === "home" && (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
                  <div className="md:hidden">
                    <h2 className="text-xl font-bold">환영합니다, {user?.name || '대표'}님! 👋</h2>
                    <p className="text-sm text-muted-foreground">{user?.company || '우리 회사'}의 재무 현황입니다.</p>
                  </div>

                  {/* 1. Stats Grid */}
                  <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
                    {dashboardData.stats?.map((stat: any, i: number) => (
                      <div key={i} className="p-5 bg-card rounded-xl border shadow-sm hover:shadow-md transition-shadow">
                        <div className="flex justify-between items-start mb-2">
                          <span className="text-sm font-medium text-muted-foreground">{stat.title}</span>
                          <span className={`text-xs px-2 py-0.5 rounded-full ${stat.trend === 'up' ? 'bg-red-100 text-red-600' : 'bg-green-100 text-green-600'}`}>
                            {stat.trend === 'up' ? '▲' : '▼'} {stat.change}
                          </span>
                        </div>
                        <div className="text-2xl font-bold">{stat.value}</div>
                        <div className="text-xs text-muted-foreground mt-1">{stat.desc}</div>
                      </div>
                    ))}
                  </div>

                  <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                    {/* Financial Chart */}
                    <div className="lg:col-span-2 p-6 bg-card rounded-xl border shadow-sm flex flex-col h-auto min-h-[400px] lg:h-full">
                      <h3 className="font-bold flex items-center gap-2 mb-6">
                        <Activity className="w-5 h-5 text-primary" />
                        재무 트렌드 (2026)
                      </h3>
                      <ResponsiveContainer width="100%" height="100%">
                        <AreaChart data={dashboardData.chart || []}>
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

                    {/* Risk Analysis Card */}
                    <div className="lg:col-span-1">
                      {risk && risk.action_items && <RiskCard risk={risk} />}
                    </div>
                  </div>

                  {/* Domain Specific Widgets */}
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

              {/* === ACCOUNTING TAB === */}
              {dashboardTab === "accounting" && (
                <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
                  <div className="col-span-1 space-y-6">
                    <TaxSimulator businessType={user?.type || 'startup'} />
                    <TaxCalendar alerts={calendarAlerts} />
                  </div>
                  <div className="col-span-1">
                    <FinancialAnalysis revenue={user?.targetRevenue || 150000000} industry={user?.type || 'startup'} />
                  </div>
                </div>
              )}

              {/* === MANAGEMENT TAB === */}
              {dashboardTab === "management" && (
                <div className="grid grid-cols-1 xl:grid-cols-12 gap-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
                  <div className="xl:col-span-5 space-y-6">
                    <BusinessLookup />
                    {/* Placeholder for Smart NTS */}
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
                    <div className="pt-4 border-t flex justify-between items-center">
                      <span className="font-bold text-gray-600">납부 예상 세액</span>
                      <span className={`text-2xl font-extrabold ${estimatedVat > 0 ? 'text-red-600' : 'text-green-600'}`}>
                        {estimatedVat.toLocaleString()}원
                      </span>
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
                            // Simple simulation of "Saving" feedback
                            const savingAmount = item.id === 3 ? "1,500,000" : item.id === 4 ? "200,000" : "500,000";
                            alert(`${item.label} 항목이 체크되었습니다.\n예상 절세액: ${savingAmount}원 반영됨`);
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
                      alert('RFI 정보가 성공적으로 저장되었습니다.\nAI 분석에 즉시 반영됩니다.');
                    }}
                    className="px-4 py-2 bg-primary text-primary-foreground text-sm font-medium rounded-lg hover:bg-primary/90 transition-colors shadow-sm"
                  >
                    저장하기
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
                      try {
                        const result = await api.updateMCPs(authUser.email, user.activeMCPs || []);
                        if (result.success) {
                          alert('MCP 설정이 저장되었습니다.');
                        }
                      } catch (e) {
                        alert('저장 중 오류가 발생했습니다.');
                      }
                    }
                  }}
                  className="mt-4 w-full py-2 bg-primary text-white rounded-lg font-medium hover:bg-primary/90 transition-colors"
                >
                  MCP 설정 저장
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
                        alert('모든 필드를 입력해주세요.');
                        return;
                      }
                      if (newPw !== confirmPw) {
                        alert('새 비밀번호가 일치하지 않습니다.');
                        return;
                      }
                      if (authUser?.email) {
                        try {
                          const result = await api.changePassword(authUser.email, currentPw, newPw);
                          if (result.success) {
                            alert('비밀번호가 변경되었습니다.');
                            (document.getElementById('currentPassword') as HTMLInputElement).value = '';
                            (document.getElementById('newPassword') as HTMLInputElement).value = '';
                            (document.getElementById('confirmPassword') as HTMLInputElement).value = '';
                          } else {
                            alert(result.message);
                          }
                        } catch (e) {
                          alert('비밀번호 변경 중 오류가 발생했습니다.');
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
                      if (!dashboardData) return;
                      const kpiCsv = dashboardData.kpi?.map((k: any) => `${k.label},${k.value},${k.trend},${k.status}`).join('\n') || '';
                      const chartCsv = dashboardData.chart?.map((c: any) => `${c.name},${c.income},${c.expense}`).join('\n') || '';
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
                  <p>지원: 연말정산, 부가세, 원천징수, 요양급여심사, 오픈마켓정산</p>
                </div>
              </div>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
