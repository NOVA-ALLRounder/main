import React, { useMemo, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Checkbox } from "@/components/ui/checkbox";
import {
  BarChart3,
  BookOpen,
  ChevronRight,
  Database,
  FileText,
  FlaskConical,
  Layers,
  ListFilter,
  Loader2,
  Play,
  Plus,
  Search,
  Sparkles,
  Wand2,
} from "lucide-react";

/**
 * 논문 생성 AI UI/UX (가설→접근방식 선택→실험→리포트/DB→선택→논문) 프로토타입
 *
 * 워크플로우
 * 1) 사용자가 '가설' 또는 '질문'을 던진다
 * 2) 시스템이 접근 방식(실험 설계/분석법) 후보를 제안한다
 * 3) 사용자가 접근 방식을 선택하고 실험을 실행한다
 * 4) 결과 리포트가 생성되고, 실험 결과가 DB(라이브러리)에 축적된다
 * 5) 사용자가 여러 실험을 선택해 종합 논문을 생성한다
 */

type PromptType = "hypothesis" | "question";

type Approach = {
  id: string;
  title: string;
  summary: string;
  requirements: string[];
  outputs: string[];
  fitScore: number; // 0~100
  risk: "low" | "mid" | "high";
};

type ExperimentStatus = "draft" | "running" | "done" | "error";

type Experiment = {
  id: string;
  domain: string;
  title: string;
  promptType: PromptType;
  prompt: string;
  approachId: string;
  approachTitle: string;
  status: ExperimentStatus;
  createdAt: string;
  tags: string[];
  report?: {
    highlights: string[];
    metrics: { name: string; value: string }[];
    narrative: string;
    limitations: string[];
  };
};

type LogItem = { ts: string; level: "INFO" | "WARN" | "ERROR"; msg: string };

type ChatItem = { role: "user" | "assistant"; content: string; ts: string };

const stepOrder = [
  { key: "seed", title: "가설/질문 던지기", icon: Sparkles },
  { key: "approach", title: "접근 방식 선택", icon: Wand2 },
  { key: "run", title: "실험 실행", icon: FlaskConical },
  { key: "report", title: "결과 리포트", icon: BarChart3 },
  { key: "db", title: "실험 DB", icon: Database },
  { key: "paper", title: "논문 생성", icon: FileText },
] as const;

type StepKey = (typeof stepOrder)[number]["key"];

function nowStamp() {
  const d = new Date();
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${hh}:${mm}`;
}

function clamp(n: number, a: number, b: number) {
  return Math.max(a, Math.min(b, n));
}

function riskBadge(risk: Approach["risk"]) {
  if (risk === "low") return <Badge variant="secondary">리스크 낮음</Badge>;
  if (risk === "mid") return <Badge variant="outline">리스크 중간</Badge>;
  return <Badge variant="destructive">리스크 높음</Badge>;
}

function statusBadge(status: ExperimentStatus) {
  if (status === "draft") return <Badge variant="secondary">Draft</Badge>;
  if (status === "running")
    return (
      <Badge variant="outline" className="gap-1">
        <Loader2 className="h-3.5 w-3.5 animate-spin" /> Running
      </Badge>
    );
  if (status === "done") return <Badge>Done</Badge>;
  return <Badge variant="destructive">Error</Badge>;
}

function StatPill({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl bg-muted px-3 py-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="text-sm font-semibold leading-5">{value}</div>
    </div>
  );
}

export default function PaperGenWorkflowPrototype() {
  const [activeStep, setActiveStep] = useState<StepKey>("seed");

  // 입력
  const [domain, setDomain] = useState("IT 서비스 운영/DevOps");
  const [promptType, setPromptType] = useState<PromptType>("hypothesis");
  const [prompt, setPrompt] = useState(
    "배포 빈도 증가(+)는 MTTR 감소(-)와 관련이 있다(팀 규모, 장애 건수 통제)."
  );

  // 접근 방식 후보(데모)
  const approaches: Approach[] = useMemo(
    () => [
      {
        id: "a1",
        title: "회귀(OLS/GLM) + 강건성 체크",
        summary:
          "가설의 방향성과 통제변수를 포함한 회귀로 검증하고, 이상치/대체 변수로 강건성을 확인합니다.",
        requirements: [
          "정량 변수(목표/설명/통제)",
          "표본 수 N ≥ 200 권장",
          "결측/이상치 처리 규칙",
        ],
        outputs: ["회귀표", "진단 요약", "해석 문장", "한계/위협"],
        fitScore: 92,
        risk: "low",
      },
      {
        id: "a2",
        title: "준-인과: 매칭/가중치(PSM/IPW)",
        summary:
          "처치(예: 고배포/저배포) 그룹을 비교하기 위해 공변량 균형을 맞춰, 인과에 가까운 해석을 제공합니다.",
        requirements: [
          "처치/비처치 정의 가능",
          "공변량(혼란변수) 충분",
          "겹침(Overlap) 조건 확인",
        ],
        outputs: ["균형 진단", "ATE 추정", "민감도 요약", "해석 문장"],
        fitScore: 80,
        risk: "mid",
      },
      {
        id: "a3",
        title: "전후 비교: DiD(차분의 차분)",
        summary:
          "정책/프로세스 변경 전후로 처리/통제 집단을 비교해 효과를 추정합니다.",
        requirements: [
          "명확한 개입 시점",
          "처리/통제 집단 존재",
          "평행추세(가정) 점검",
        ],
        outputs: ["효과 추정", "이벤트 스터디", "가정 점검", "해석 문장"],
        fitScore: 68,
        risk: "high",
      },
    ],
    []
  );

  const [selectedApproachId, setSelectedApproachId] = useState<string>("a1");

  // 실험 구성
  const [expName, setExpName] = useState("배포 빈도 vs MTTR 검증");
  const [dataSource, setDataSource] = useState("CSV 업로드");
  const [alpha, setAlpha] = useState("0.05");
  const [seed, setSeed] = useState("42");

  // 실행 상태
  const [runStatus, setRunStatus] = useState<ExperimentStatus>("draft");
  const [runProgress, setRunProgress] = useState(0);

  // DB(실험 라이브러리)
  const [experiments, setExperiments] = useState<Experiment[]>([
    {
      id: "e101",
      domain: "IT 서비스 운영/DevOps",
      title: "배포 빈도 vs MTTR (강건 회귀)",
      promptType: "hypothesis",
      prompt:
        "배포 빈도 증가(+)는 MTTR 감소(-)와 관련이 있다(팀 규모, 장애 건수 통제).",
      approachId: "a1",
      approachTitle: "회귀 + 강건성 체크",
      status: "done",
      createdAt: "오늘 09:10",
      tags: ["devops", "mttr", "regression"],
      report: {
        highlights: [
          "배포 빈도는 MTTR과 음의 관계",
          "통제 포함 모델에서도 유의",
          "이상치 처리 후에도 결론 유지",
        ],
        metrics: [
          { name: "p-value", value: "< 0.05" },
          { name: "효과크기", value: "-3.1% / +1배포" },
          { name: "R²", value: "0.34" },
        ],
        narrative:
          "회귀 분석 결과, 배포 빈도는 MTTR과 음의 상관관계를 보였으며 통제변수를 포함한 모델에서도 유의하였다.",
        limitations: [
          "관측 데이터 기반으로 잠재적 교란 변수가 남을 수 있음",
          "팀별 프로세스 차이가 효과 추정에 영향 가능",
        ],
      },
    },
    {
      id: "e102",
      domain: "사이버보안",
      title: "패치 주기 단축 효과(PSM)",
      promptType: "question",
      prompt: "패치 주기를 단축하면 침해/취약점 노출이 감소하는가?",
      approachId: "a2",
      approachTitle: "매칭/가중치(준-인과)",
      status: "done",
      createdAt: "어제 22:45",
      tags: ["security", "patch", "causal"],
      report: {
        highlights: [
          "처치/비처치 균형 후 효과 추정",
          "민감도 분석에서 결과 안정",
        ],
        metrics: [
          { name: "ATE", value: "-0.12" },
          { name: "SMD", value: "< 0.10" },
        ],
        narrative:
          "매칭 후 추정된 평균 처리 효과는 음의 방향을 보였으며, 공변량 균형이 개선된 상태에서 해석 가능하다.",
        limitations: ["숨은 교란에 취약", "겹침 조건 부족 시 불안정"],
      },
    },
  ]);

  const [selectedExperimentIds, setSelectedExperimentIds] = useState<string[]>([
    "e101",
  ]);

  // DB 탐색/정렬
  const [dbQuery, setDbQuery] = useState("");
  const [dbDomain, setDbDomain] = useState<string>("all");
  const [dbSort, setDbSort] = useState<string>("recent");

  // 우측: 코파일럿 + 로그
  const [chat, setChat] = useState<ChatItem[]>([
    {
      role: "assistant",
      content:
        "흐름은 아주 좋아요. 핵심은 ‘접근 방식 후보’를 사람이 고를 수 있게 하고, 실험 결과를 DB에 축적해서 나중에 논문 재료로 쓰는 거예요.
먼저 가설/질문을 던지고, 데이터/제약(유의수준, 시드 등)을 최소로 고정해봅시다.",
      ts: "09:14",
    },
  ]);
  const [chatDraft, setChatDraft] = useState("");
  const [logs, setLogs] = useState<LogItem[]>([
    { ts: "09:12", level: "INFO", msg: "프로토타입 로드" },
    { ts: "09:13", level: "INFO", msg: "실험 DB 2건 적재" },
  ]);

  const selectedApproach = useMemo(
    () => approaches.find((a) => a.id === selectedApproachId)!,
    [approaches, selectedApproachId]
  );

  const filteredExperiments = useMemo(() => {
    const q = dbQuery.trim().toLowerCase();
    let list = experiments.slice();
    if (dbDomain !== "all") list = list.filter((e) => e.domain === dbDomain);
    if (q) {
      list = list.filter((e) => {
        const blob = [
          e.title,
          e.domain,
          e.prompt,
          e.approachTitle,
          e.tags.join(" "),
        ]
          .join(" ")
          .toLowerCase();
        return blob.includes(q);
      });
    }
    if (dbSort === "recent") {
      // 데모: createdAt 문자열 기반, 실제로는 timestamp로 정렬
      list = list.reverse();
    }
    if (dbSort === "domain") {
      list = list.sort((a, b) => a.domain.localeCompare(b.domain));
    }
    if (dbSort === "status") {
      const rank: Record<ExperimentStatus, number> = {
        running: 0,
        draft: 1,
        done: 2,
        error: 3,
      };
      list = list.sort((a, b) => rank[a.status] - rank[b.status]);
    }
    return list;
  }, [experiments, dbDomain, dbQuery, dbSort]);

  const selectedExperiments = useMemo(
    () => experiments.filter((e) => selectedExperimentIds.includes(e.id)),
    [experiments, selectedExperimentIds]
  );

  function pushLog(level: LogItem["level"], msg: string) {
    setLogs((prev) => [...prev, { ts: nowStamp(), level, msg }]);
  }

  function goNext() {
    const idx = stepOrder.findIndex((s) => s.key === activeStep);
    if (idx < stepOrder.length - 1) setActiveStep(stepOrder[idx + 1].key);
  }

  function goTo(key: StepKey) {
    setActiveStep(key);
  }

  function suggestApproaches() {
    pushLog("INFO", `접근 방식 후보 생성: ${promptType === "hypothesis" ? "가설" : "질문"}`);
    setChat((prev) => [
      ...prev,
      {
        role: "assistant",
        content:
          "접근 방식 후보를 제안했어요. ‘데이터 요건’과 ‘리스크’를 보고 하나를 선택하면, 실험 템플릿(설계/출력물)이 자동 채워집니다.",
        ts: nowStamp(),
      },
    ]);
    goTo("approach");
  }

  function runExperiment() {
    if (runStatus === "running") return;
    setRunStatus("running");
    setRunProgress(7);
    pushLog("INFO", `실험 실행 시작: ${expName}`);

    let t = 0;
    const interval = setInterval(() => {
      t += 1;
      setRunProgress((p) => clamp(p + 18, 0, 96));
      if (t === 1) pushLog("INFO", "입력 검증: OK" );
      if (t === 2) pushLog("INFO", "데이터 로드/전처리" );
      if (t === 3) pushLog("INFO", "분석/추정 실행" );
      if (t === 4) pushLog("INFO", "리포트 생성" );

      if (t >= 5) {
        clearInterval(interval);
        setRunStatus("done");
        setRunProgress(100);
        pushLog("INFO", "실험 완료" );

        const id = `e${Math.floor(200 + Math.random() * 900)}`;
        const createdAt = "방금";
        const report = {
          highlights: [
            "주요 계수/효과 추정 완료",
            "강건성 체크 통과(데모)",
            "해석 문장 자동 생성",
          ],
          metrics: [
            { name: "p-value", value: "< 0.05" },
            { name: "효과크기", value: "-2.7% / +1배포" },
            { name: "N", value: "12,480" },
          ],
          narrative:
            "선택한 접근 방식으로 추정한 결과, 핵심 설명변수는 목표변수와 기대한 방향의 관계를 보였으며 유의하였다(데모).",
          limitations: [
            "데모 리포트: 실제 데이터 조건에 따라 결과가 달라질 수 있음",
          ],
        };

        const exp: Experiment = {
          id,
          domain,
          title: expName,
          promptType,
          prompt,
          approachId: selectedApproachId,
          approachTitle: selectedApproach.title,
          status: "done",
          createdAt,
          tags: [
            domain.includes("DevOps") ? "devops" : "custom",
            selectedApproachId,
          ],
          report,
        };

        // DB에 저장(축적)
        setExperiments((prev) => [exp, ...prev]);
        pushLog("INFO", `DB 저장: ${id}`);

        // 자동 선택 리스트에 포함
        setSelectedExperimentIds((prev) => Array.from(new Set([id, ...prev])));

        // 다음 화면 이동
        setTimeout(() => {
          goTo("report");
        }, 250);
      }
    }, 520);
  }

  function toggleSelectExperiment(id: string, checked: boolean) {
    setSelectedExperimentIds((prev) => {
      if (checked) return Array.from(new Set([...prev, id]));
      return prev.filter((x) => x !== id);
    });
  }

  function sendChat() {
    const text = chatDraft.trim();
    if (!text) return;
    setChat((prev) => [...prev, { role: "user", content: text, ts: nowStamp() }]);
    setChatDraft("");

    setTimeout(() => {
      const auto =
        activeStep === "seed"
          ? "가설이면 ‘방향성/통제/측정단위’를, 질문이면 ‘비교 대상/기간/결과지표’를 추가하면 접근 방식 추천이 정확해져요."
          : activeStep === "approach"
          ? "선택 기준은 간단히: (1) 데이터 요건 충족, (2) 해석 난이도, (3) 리스크(가정)예요."
          : activeStep === "run"
          ? "실험 실행 전 최소 고정값: 유의수준(alpha), 시드(seed), 전처리 규칙(결측/이상치)만 잠그면 재현성이 좋아요."
          : activeStep === "report"
          ? "리포트는 ‘표/그림 + 한 줄 결론 + 한계’가 핵심이에요. 논문 문장으로 바로 변환되는 형태로 저장해둘게요."
          : activeStep === "db"
          ? "DB에서는 도메인/태그/상태로 정렬하고, 논문에 넣을 실험만 체크해서 묶으면 됩니다."
          : "논문 생성은 선택한 실험들의 ‘공통 스토리라인’(연구문제-방법-결과-논의)을 자동으로 엮는 게 포인트예요.";
      setChat((prev) => [...prev, { role: "assistant", content: auto, ts: nowStamp() }]);
    }, 450);
  }

  const paperDraft = useMemo(() => {
    const items = selectedExperiments.filter((e) => e.report);
    if (items.length === 0)
      return {
        title: "(선택된 실험이 없습니다)",
        abstract:
          "실험을 1개 이상 선택하면, 종합 논문 초안을 자동 생성합니다.",
        contributions: [""],
        methods: [""],
        results: [""],
        limitations: [""],
      };

    const title =
      items.length === 1
        ? `실험 기반 분석: ${items[0].title}`
        : `복수 실험 종합 분석 (${items.length}건)`;

    const abstract =
      `본 연구는 ${items
        .map((e) => `“${e.prompt}”`)
        .slice(0, 2)
        .join(" 및 ")}` +
      (items.length > 2 ? " 등" : "") +
      `의 연구문제를 대상으로, 선택된 ${items.length}개의 실험 결과를 종합하여 결론을 도출한다. ` +
      `각 실험은 접근 방식(회귀/준-인과/전후비교 등)에 따라 수행되었으며, 주요 결과와 한계를 함께 보고한다.`;

    const contributions = [
      "가설/질문 → 접근 방식 추천 → 실험 → 리포트/DB 축적 → 논문 생성의 end-to-end 파이프라인 제시",
      "실험 결과를 ‘논문 문장’ 단위로 저장하여 재사용 가능하게 구성",
    ];

    const methods = items.map(
      (e) => `실험(${e.id})은 ${e.approachTitle} 방식으로 수행하였다(유의수준 ${alpha}, seed=${seed}).`
    );

    const results = items
      .filter((e) => e.report)
      .map((e) => `(${e.id}) ${e.report!.narrative}`);

    const limitations = Array.from(
      new Set(items.flatMap((e) => e.report?.limitations ?? []))
    );

    return { title, abstract, contributions, methods, results, limitations };
  }, [alpha, seed, selectedExperiments]);

  const currentStepMeta = stepOrder.find((s) => s.key === activeStep)!;
  const CurrentIcon = currentStepMeta.icon;

  const progress = useMemo(() => {
    const idx = stepOrder.findIndex((s) => s.key === activeStep);
    return Math.round(((idx + 1) / stepOrder.length) * 100);
  }, [activeStep]);

  return (
    <div className="min-h-screen bg-background">
      <div className="mx-auto grid max-w-7xl grid-cols-12 gap-4 p-4 md:p-6">
        {/* Left: Experiment DB quick view */}
        <aside className="col-span-12 md:col-span-4 lg:col-span-3">
          <Card className="rounded-2xl">
            <CardHeader className="pb-3">
              <div className="flex items-center justify-between">
                <CardTitle className="text-base flex items-center gap-2">
                  <Database className="h-4 w-4" /> 실험 DB
                </CardTitle>
                <Dialog>
                  <DialogTrigger asChild>
                    <Button size="sm" className="rounded-2xl" variant="secondary">
                      <Plus className="mr-2 h-4 w-4" /> 새 실험
                    </Button>
                  </DialogTrigger>
                  <DialogContent className="rounded-2xl">
                    <DialogHeader>
                      <DialogTitle>새 실험 시작</DialogTitle>
                      <DialogDescription>
                        가설/질문을 던지고 접근 방식을 선택해 실험을 만들어요.
                      </DialogDescription>
                    </DialogHeader>
                    <div className="grid gap-3">
                      <Select value={domain} onValueChange={setDomain}>
                        <SelectTrigger className="rounded-2xl">
                          <SelectValue placeholder="도메인" />
                        </SelectTrigger>
                        <SelectContent className="rounded-2xl">
                          <SelectItem value="IT 서비스 운영/DevOps">
                            IT 서비스 운영/DevOps
                          </SelectItem>
                          <SelectItem value="사이버보안">사이버보안</SelectItem>
                          <SelectItem value="핀테크">핀테크</SelectItem>
                          <SelectItem value="스마트시티">스마트시티</SelectItem>
                          <SelectItem value="커스텀">커스텀</SelectItem>
                        </SelectContent>
                      </Select>
                      <Input
                        className="rounded-2xl"
                        placeholder="실험 이름"
                        value={expName}
                        onChange={(e) => setExpName(e.target.value)}
                      />
                      <Button
                        className="rounded-2xl"
                        onClick={() => {
                          goTo("seed");
                          pushLog("INFO", "새 실험 플로우로 이동" );
                        }}
                      >
                        시작
                      </Button>
                    </div>
                    <DialogFooter />
                  </DialogContent>
                </Dialog>
              </div>
              <CardDescription className="text-xs">
                결과 리포트가 저장되고, 선택해서 논문을 만들 수 있어요.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="flex flex-wrap gap-2">
                <div className="relative flex-1 min-w-[180px]">
                  <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
                  <Input
                    className="rounded-2xl pl-9"
                    placeholder="검색: 제목/태그/가설"
                    value={dbQuery}
                    onChange={(e) => setDbQuery(e.target.value)}
                  />
                </div>
                <Select value={dbDomain} onValueChange={setDbDomain}>
                  <SelectTrigger className="rounded-2xl w-[140px]">
                    <SelectValue placeholder="도메인" />
                  </SelectTrigger>
                  <SelectContent className="rounded-2xl">
                    <SelectItem value="all">전체</SelectItem>
                    {Array.from(new Set(experiments.map((e) => e.domain))).map((d) => (
                      <SelectItem key={d} value={d}>
                        {d}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Select value={dbSort} onValueChange={setDbSort}>
                  <SelectTrigger className="rounded-2xl w-[140px]">
                    <SelectValue placeholder="정렬" />
                  </SelectTrigger>
                  <SelectContent className="rounded-2xl">
                    <SelectItem value="recent">최신순</SelectItem>
                    <SelectItem value="domain">도메인</SelectItem>
                    <SelectItem value="status">상태</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className="h-[480px] overflow-auto rounded-2xl border bg-muted/15 p-2">
                <div className="space-y-2">
                  {filteredExperiments.map((e) => {
                    const checked = selectedExperimentIds.includes(e.id);
                    return (
                      <div
                        key={e.id}
                        className="rounded-2xl border bg-card p-3 hover:bg-muted/40"
                      >
                        <div className="flex items-start gap-3">
                          <Checkbox
                            checked={checked}
                            onCheckedChange={(v) =>
                              toggleSelectExperiment(e.id, Boolean(v))
                            }
                            className="mt-1"
                          />
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center justify-between gap-2">
                              <div className="truncate text-sm font-semibold">
                                {e.title}
                              </div>
                              {statusBadge(e.status)}
                            </div>
                            <div className="mt-1 flex flex-wrap gap-2">
                              <Badge variant="secondary">{e.domain}</Badge>
                              <Badge variant="outline">{e.approachTitle}</Badge>
                              <Badge variant="secondary">{e.createdAt}</Badge>
                            </div>
                            <div className="mt-2 line-clamp-2 text-xs text-muted-foreground">
                              {e.prompt}
                            </div>
                            <div className="mt-2 flex flex-wrap gap-1">
                              {e.tags.slice(0, 4).map((t) => (
                                <Badge key={t} variant="secondary" className="text-[11px]">
                                  {t}
                                </Badge>
                              ))}
                            </div>
                            <div className="mt-3 flex flex-wrap gap-2">
                              <Button
                                size="sm"
                                className="rounded-2xl"
                                variant="secondary"
                                onClick={() => {
                                  // 리포트 화면으로 이동하면서 해당 실험을 최상단 선택
                                  setSelectedExperimentIds((prev) =>
                                    Array.from(new Set([e.id, ...prev]))
                                  );
                                  goTo("report");
                                  pushLog("INFO", `리포트 열람: ${e.id}`);
                                }}
                              >
                                리포트 보기
                              </Button>
                              <Button
                                size="sm"
                                className="rounded-2xl"
                                onClick={() => {
                                  goTo("paper");
                                  pushLog("INFO", "논문 생성 화면으로 이동" );
                                }}
                              >
                                논문에 포함
                              </Button>
                            </div>
                          </div>
                        </div>
                      </div>
                    );
                  })}

                  {filteredExperiments.length === 0 ? (
                    <div className="rounded-2xl border bg-card p-4 text-sm text-muted-foreground">
                      검색 결과가 없어요.
                    </div>
                  ) : null}
                </div>
              </div>

              <div className="flex items-center justify-between">
                <div className="text-xs text-muted-foreground flex items-center gap-2">
                  <ListFilter className="h-4 w-4" /> 선택된 실험 {selectedExperimentIds.length}건
                </div>
                <Button
                  className="rounded-2xl"
                  onClick={() => goTo("paper")}
                  disabled={selectedExperimentIds.length === 0}
                >
                  <BookOpen className="mr-2 h-4 w-4" /> 종합 논문 만들기
                </Button>
              </div>
            </CardContent>
          </Card>
        </aside>

        {/* Center: Workflow */}
        <main className="col-span-12 md:col-span-8 lg:col-span-6">
          <Card className="rounded-2xl">
            <CardContent className="p-4">
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <div className="flex items-center gap-3">
                  <div className="rounded-2xl bg-muted p-2">
                    <CurrentIcon className="h-5 w-5" />
                  </div>
                  <div>
                    <div className="text-lg font-semibold">{currentStepMeta.title}</div>
                    <div className="text-sm text-muted-foreground">
                      가설/질문 → 접근 방식 → 실험 → 리포트/DB → 선택 → 논문
                    </div>
                  </div>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <StatPill label="도메인" value={domain} />
                  <StatPill label="선택 실험" value={`${selectedExperimentIds.length}건`} />
                </div>
              </div>

              <div className="mt-4 space-y-2">
                <div className="flex items-center justify-between text-xs text-muted-foreground">
                  <span>진행</span>
                  <span>{progress}%</span>
                </div>
                <Progress value={progress} />
              </div>
            </CardContent>
          </Card>

          <Card className="mt-4 rounded-2xl">
            <CardContent className="p-0">
              <Tabs value={activeStep} onValueChange={(v) => setActiveStep(v as StepKey)}>
                <div className="border-b p-3">
                  <TabsList className="grid w-full grid-cols-6 rounded-2xl">
                    {stepOrder.map((s) => (
                      <TabsTrigger key={s.key} value={s.key} className="rounded-2xl">
                        {s.title}
                      </TabsTrigger>
                    ))}
                  </TabsList>
                </div>

                {/* 1) Seed */}
                <TabsContent value="seed" className="m-0">
                  <div className="p-4 grid gap-4 lg:grid-cols-2">
                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">가설/질문 입력</CardTitle>
                        <CardDescription>
                          “가설”이면 검증 문장, “질문”이면 비교/관찰 질문을 던져요.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="space-y-2">
                          <div className="text-sm font-medium">도메인</div>
                          <Select value={domain} onValueChange={setDomain}>
                            <SelectTrigger className="rounded-2xl">
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent className="rounded-2xl">
                              <SelectItem value="IT 서비스 운영/DevOps">
                                IT 서비스 운영/DevOps
                              </SelectItem>
                              <SelectItem value="사이버보안">사이버보안</SelectItem>
                              <SelectItem value="핀테크">핀테크</SelectItem>
                              <SelectItem value="스마트시티">스마트시티</SelectItem>
                              <SelectItem value="커스텀">커스텀</SelectItem>
                            </SelectContent>
                          </Select>
                        </div>

                        <div className="grid gap-3 sm:grid-cols-2">
                          <div className="space-y-2">
                            <div className="text-sm font-medium">유형</div>
                            <Select
                              value={promptType}
                              onValueChange={(v) => setPromptType(v as PromptType)}
                            >
                              <SelectTrigger className="rounded-2xl">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent className="rounded-2xl">
                                <SelectItem value="hypothesis">가설</SelectItem>
                                <SelectItem value="question">질문</SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                          <div className="space-y-2">
                            <div className="text-sm font-medium">실험 이름</div>
                            <Input
                              className="rounded-2xl"
                              value={expName}
                              onChange={(e) => setExpName(e.target.value)}
                              placeholder="예: 배포 빈도 vs MTTR 검증"
                            />
                          </div>
                        </div>

                        <div className="space-y-2">
                          <div className="text-sm font-medium">가설/질문</div>
                          <Textarea
                            className="min-h-[140px] rounded-2xl"
                            value={prompt}
                            onChange={(e) => setPrompt(e.target.value)}
                            placeholder={
                              promptType === "hypothesis"
                                ? "예: X 증가(+)는 Y 감소(-)와 관련이 있다(통제변수...)."
                                : "예: A를 하면 B가 감소하는가? 비교 대상/기간/지표는?"
                            }
                          />
                        </div>

                        <div className="flex flex-wrap gap-2">
                          <Badge variant="secondary">
                            {promptType === "hypothesis" ? "반증 가능" : "관찰 질문"}
                          </Badge>
                          <Badge variant="secondary">도메인 템플릿</Badge>
                          <Badge variant="secondary">실험 자동 저장</Badge>
                        </div>

                        <Button
                          className="w-full rounded-2xl"
                          onClick={suggestApproaches}
                        >
                          <Sparkles className="mr-2 h-4 w-4" />
                          접근 방식 제안받기
                        </Button>
                      </CardContent>
                    </Card>

                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">입력 품질 가이드</CardTitle>
                        <CardDescription>
                          추천 후보의 정확도를 올리는 최소 정보.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">가설이면</div>
                          <div className="mt-2 text-sm text-muted-foreground">
                            방향성(+, -) · 통제변수 · 측정단위(기간/집계)
                          </div>
                        </div>
                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">질문이면</div>
                          <div className="mt-2 text-sm text-muted-foreground">
                            비교 대상(그룹) · 기간 · 결과지표 · 개입(있다면)
                          </div>
                        </div>
                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">저장되는 것</div>
                          <div className="mt-2 text-sm text-muted-foreground">
                            입력 + 선택한 접근 방식 + 설정(alpha/seed) + 리포트가 DB에 쌓여요.
                          </div>
                        </div>

                        <Button
                          variant="secondary"
                          className="w-full rounded-2xl"
                          onClick={() => {
                            setChat((prev) => [
                              ...prev,
                              {
                                role: "assistant",
                                content:
                                  "원하면 입력을 ‘논문용 연구문제 문장’으로 정제해줄게요. 가설/질문을 조금만 더 구체화해보죠.",
                                ts: nowStamp(),
                              },
                            ]);
                          }}
                        >
                          입력 문장 다듬기
                        </Button>
                      </CardContent>
                    </Card>
                  </div>
                </TabsContent>

                {/* 2) Approach */}
                <TabsContent value="approach" className="m-0">
                  <div className="p-4 space-y-4">
                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">접근 방식 후보</CardTitle>
                        <CardDescription>
                          후보를 비교하고 하나를 선택하면 실험 템플릿이 자동 채워져요.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="grid gap-3 lg:grid-cols-3">
                        {approaches.map((a) => {
                          const active = a.id === selectedApproachId;
                          return (
                            <button
                              key={a.id}
                              className={
                                "rounded-2xl border p-3 text-left transition hover:bg-muted/40 " +
                                (active ? "bg-muted" : "bg-card")
                              }
                              onClick={() => {
                                setSelectedApproachId(a.id);
                                pushLog("INFO", `접근 방식 선택: ${a.title}`);
                              }}
                            >
                              <div className="flex items-start justify-between gap-2">
                                <div className="min-w-0">
                                  <div className="text-sm font-semibold line-clamp-2">
                                    {a.title}
                                  </div>
                                  <div className="mt-1 text-xs text-muted-foreground line-clamp-3">
                                    {a.summary}
                                  </div>
                                </div>
                                <Badge variant={active ? "default" : "secondary"}>
                                  {a.fitScore}
                                </Badge>
                              </div>

                              <div className="mt-3 flex flex-wrap gap-2">
                                {riskBadge(a.risk)}
                                <Badge variant="secondary">산출물 {a.outputs.length}개</Badge>
                              </div>

                              <div className="mt-3">
                                <div className="text-xs font-medium text-muted-foreground">
                                  요구사항
                                </div>
                                <div className="mt-1 text-xs text-muted-foreground">
                                  {a.requirements.slice(0, 2).join(" · ")}
                                  {a.requirements.length > 2 ? " · …" : ""}
                                </div>
                              </div>
                            </button>
                          );
                        })}
                      </CardContent>
                    </Card>

                    <div className="grid gap-4 lg:grid-cols-2">
                      <Card className="rounded-2xl">
                        <CardHeader>
                          <CardTitle className="text-base">선택 요약</CardTitle>
                          <CardDescription>
                            선택한 방식의 요구사항/산출물을 확인해요.
                          </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-3">
                          <div className="rounded-2xl border p-3">
                            <div className="flex items-center justify-between gap-2">
                              <div className="text-sm font-semibold">
                                {selectedApproach.title}
                              </div>
                              {riskBadge(selectedApproach.risk)}
                            </div>
                            <div className="mt-2 text-sm text-muted-foreground">
                              {selectedApproach.summary}
                            </div>
                          </div>

                          <div className="grid gap-3 sm:grid-cols-2">
                            <div className="rounded-2xl border p-3">
                              <div className="text-xs text-muted-foreground">요구사항</div>
                              <div className="mt-2 space-y-1 text-sm">
                                {selectedApproach.requirements.map((r) => (
                                  <div key={r} className="flex items-center gap-2">
                                    <ChevronRight className="h-4 w-4 text-muted-foreground" />
                                    <span className="text-muted-foreground">{r}</span>
                                  </div>
                                ))}
                              </div>
                            </div>
                            <div className="rounded-2xl border p-3">
                              <div className="text-xs text-muted-foreground">산출물</div>
                              <div className="mt-2 flex flex-wrap gap-2">
                                {selectedApproach.outputs.map((o) => (
                                  <Badge key={o} variant="secondary">
                                    {o}
                                  </Badge>
                                ))}
                              </div>
                            </div>
                          </div>

                          <Button
                            className="w-full rounded-2xl"
                            onClick={() => {
                              pushLog("INFO", "실험 설정 단계로 이동" );
                              goTo("run");
                            }}
                          >
                            이 방식으로 실험 설계하기
                          </Button>
                        </CardContent>
                      </Card>

                      <Card className="rounded-2xl">
                        <CardHeader>
                          <CardTitle className="text-base">대안도 함께 남기기</CardTitle>
                          <CardDescription>
                            나중에 다른 방식으로 재실험(재현)하기 쉽도록.
                          </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-3">
                          <div className="rounded-2xl border p-3">
                            <div className="text-sm font-semibold">추천 UX</div>
                            <div className="mt-2 text-sm text-muted-foreground">
                              선택된 방식 1개는 “실험 실행”으로, 나머지 후보는 “대안 접근 방식”으로 메타데이터만 저장.
                            </div>
                          </div>
                          <Button
                            variant="secondary"
                            className="w-full rounded-2xl"
                            onClick={() => {
                              pushLog("INFO", "후보 목록을 메타데이터로 저장(데모)" );
                              setChat((prev) => [
                                ...prev,
                                {
                                  role: "assistant",
                                  content:
                                    "좋아요. 선택하지 않은 후보들도 ‘대안 접근 방식’으로 저장해둘게요. 나중에 클릭 한 번으로 재실험할 수 있어요.",
                                  ts: nowStamp(),
                                },
                              ]);
                            }}
                          >
                            후보 메모로 저장
                          </Button>
                        </CardContent>
                      </Card>
                    </div>
                  </div>
                </TabsContent>

                {/* 3) Run */}
                <TabsContent value="run" className="m-0">
                  <div className="p-4 grid gap-4 lg:grid-cols-2">
                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">실험 설정</CardTitle>
                        <CardDescription>
                          최소 고정값(alpha/seed/전처리)만 잠가도 재현성이 좋아져요.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="space-y-2">
                          <div className="text-sm font-medium">실험 이름</div>
                          <Input
                            className="rounded-2xl"
                            value={expName}
                            onChange={(e) => setExpName(e.target.value)}
                          />
                        </div>

                        <div className="rounded-2xl border p-3">
                          <div className="text-xs text-muted-foreground">선택된 접근 방식</div>
                          <div className="mt-1 text-sm font-semibold">
                            {selectedApproach.title}
                          </div>
                          <div className="mt-2 text-sm text-muted-foreground line-clamp-3">
                            {selectedApproach.summary}
                          </div>
                        </div>

                        <div className="grid gap-3 sm:grid-cols-2">
                          <div className="space-y-2">
                            <div className="text-sm font-medium">데이터 소스</div>
                            <Select value={dataSource} onValueChange={setDataSource}>
                              <SelectTrigger className="rounded-2xl">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent className="rounded-2xl">
                                <SelectItem value="CSV 업로드">CSV 업로드</SelectItem>
                                <SelectItem value="DB 연결">DB 연결</SelectItem>
                                <SelectItem value="로그 수집">로그 수집</SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                          <div className="space-y-2">
                            <div className="text-sm font-medium">유의수준(alpha)</div>
                            <Input
                              className="rounded-2xl"
                              value={alpha}
                              onChange={(e) => setAlpha(e.target.value)}
                              placeholder="0.05"
                            />
                          </div>
                        </div>

                        <div className="grid gap-3 sm:grid-cols-2">
                          <div className="space-y-2">
                            <div className="text-sm font-medium">seed</div>
                            <Input
                              className="rounded-2xl"
                              value={seed}
                              onChange={(e) => setSeed(e.target.value)}
                              placeholder="42"
                            />
                          </div>
                          <div className="space-y-2">
                            <div className="text-sm font-medium">출력</div>
                            <Select defaultValue="full">
                              <SelectTrigger className="rounded-2xl">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent className="rounded-2xl">
                                <SelectItem value="full">전체 리포트</SelectItem>
                                <SelectItem value="lite">요약 리포트</SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                        </div>

                        <div className="flex gap-2">
                          <Button
                            className="flex-1 rounded-2xl"
                            onClick={() => {
                              pushLog("INFO", "실험 실행 버튼 클릭" );
                              goTo("run");
                              runExperiment();
                            }}
                            disabled={runStatus === "running"}
                          >
                            {runStatus === "running" ? (
                              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                            ) : (
                              <Play className="mr-2 h-4 w-4" />
                            )}
                            실행
                          </Button>
                          <Button
                            variant="secondary"
                            className="rounded-2xl"
                            onClick={() => {
                              pushLog("INFO", "DB 화면으로 이동" );
                              goTo("db");
                            }}
                          >
                            DB 보기
                          </Button>
                        </div>

                        <div className="space-y-2">
                          <div className="flex items-center justify-between text-xs text-muted-foreground">
                            <span>실행 진행</span>
                            <span>{runProgress}%</span>
                          </div>
                          <Progress value={runProgress} />
                        </div>
                      </CardContent>
                    </Card>

                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">실험 입력 프리뷰</CardTitle>
                        <CardDescription>
                          DB에 저장되는 “재현 메타데이터”를 미리 확인해요.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="rounded-2xl border bg-muted/30 p-3">
                          <div className="text-xs text-muted-foreground">도메인</div>
                          <div className="mt-1 text-sm font-semibold">{domain}</div>
                          <div className="mt-3 text-xs text-muted-foreground">
                            {promptType === "hypothesis" ? "가설" : "질문"}
                          </div>
                          <div className="mt-1 text-sm text-muted-foreground whitespace-pre-wrap">
                            {prompt}
                          </div>
                        </div>

                        <div className="grid gap-3 sm:grid-cols-2">
                          <StatPill label="데이터" value={dataSource} />
                          <StatPill label="alpha" value={alpha} />
                          <StatPill label="seed" value={seed} />
                          <StatPill label="접근 방식" value={selectedApproachId} />
                        </div>

                        <div className="rounded-2xl border p-3">
                          <div className="flex items-center gap-2 text-sm font-medium">
                            <Layers className="h-4 w-4" />
                            저장 구조(권장)
                          </div>
                          <div className="mt-2 text-sm text-muted-foreground">
                            Experiment(메타) + Report(요약/표/문장) + Artifacts(표/그림/파일)
                          </div>
                        </div>

                        <Button
                          variant="secondary"
                          className="w-full rounded-2xl"
                          onClick={() => {
                            setRunStatus("draft");
                            setRunProgress(0);
                            pushLog("INFO", "실험 상태 초기화(데모)" );
                          }}
                        >
                          데모 초기화
                        </Button>
                      </CardContent>
                    </Card>
                  </div>
                </TabsContent>

                {/* 4) Report */}
                <TabsContent value="report" className="m-0">
                  <div className="p-4 grid gap-4 lg:grid-cols-2">
                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">결과 리포트</CardTitle>
                        <CardDescription>
                          가장 최근 생성/선택된 실험의 리포트를 보여줘요.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        {selectedExperiments[0]?.report ? (
                          <>
                            <div className="rounded-2xl border p-3">
                              <div className="flex items-center justify-between gap-2">
                                <div className="text-sm font-semibold">
                                  {selectedExperiments[0].title}
                                </div>
                                <Badge variant="secondary">
                                  {selectedExperiments[0].id}
                                </Badge>
                              </div>
                              <div className="mt-2 text-sm text-muted-foreground">
                                {selectedExperiments[0].report!.narrative}
                              </div>
                            </div>

                            <div className="grid gap-3 sm:grid-cols-3">
                              {selectedExperiments[0].report!.metrics.map((m) => (
                                <div key={m.name} className="rounded-2xl border p-3">
                                  <div className="text-xs text-muted-foreground">
                                    {m.name}
                                  </div>
                                  <div className="mt-1 text-sm font-semibold">
                                    {m.value}
                                  </div>
                                </div>
                              ))}
                            </div>

                            <div className="rounded-2xl border p-3">
                              <div className="text-sm font-semibold">하이라이트</div>
                              <div className="mt-2 space-y-1 text-sm text-muted-foreground">
                                {selectedExperiments[0].report!.highlights.map((h) => (
                                  <div key={h} className="flex items-center gap-2">
                                    <ChevronRight className="h-4 w-4 text-muted-foreground" />
                                    <span>{h}</span>
                                  </div>
                                ))}
                              </div>
                            </div>

                            <div className="rounded-2xl border p-3">
                              <div className="text-sm font-semibold">한계/위협</div>
                              <div className="mt-2 space-y-1 text-sm text-muted-foreground">
                                {selectedExperiments[0].report!.limitations.map((l) => (
                                  <div key={l} className="flex items-center gap-2">
                                    <ChevronRight className="h-4 w-4 text-muted-foreground" />
                                    <span>{l}</span>
                                  </div>
                                ))}
                              </div>
                            </div>

                            <div className="flex gap-2">
                              <Button
                                className="flex-1 rounded-2xl"
                                onClick={() => {
                                  goTo("paper");
                                  pushLog("INFO", "리포트에서 논문 생성으로 이동" );
                                }}
                              >
                                <BookOpen className="mr-2 h-4 w-4" />
                                논문에 반영
                              </Button>
                              <Button
                                variant="secondary"
                                className="rounded-2xl"
                                onClick={() => {
                                  goTo("db");
                                  pushLog("INFO", "DB에서 다른 리포트 선택" );
                                }}
                              >
                                DB에서 선택
                              </Button>
                            </div>
                          </>
                        ) : (
                          <div className="rounded-2xl border bg-muted/20 p-4 text-sm text-muted-foreground">
                            선택된 실험의 리포트가 없어요. 실험을 실행하거나 DB에서 Done 실험을 선택해 주세요.
                          </div>
                        )}
                      </CardContent>
                    </Card>

                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">리포트 출력(UX)</CardTitle>
                        <CardDescription>
                          “보고서 출력 + DB 저장”을 한 번에 처리하는 버튼.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">출력 옵션</div>
                          <div className="mt-2 flex flex-wrap gap-2">
                            <Badge variant="secondary">PDF</Badge>
                            <Badge variant="secondary">HTML</Badge>
                            <Badge variant="secondary">Markdown</Badge>
                            <Badge variant="secondary">JSON(구조화)</Badge>
                          </div>
                          <div className="mt-3 text-sm text-muted-foreground">
                            실제 구현에서는 “리포트 JSON”을 DB에 저장하고, PDF/HTML은 아티팩트로 연결하면 좋아요.
                          </div>
                        </div>

                        <Button
                          className="w-full rounded-2xl"
                          onClick={() => {
                            pushLog("INFO", "리포트 출력 요청(데모)" );
                            setChat((prev) => [
                              ...prev,
                              {
                                role: "assistant",
                                content:
                                  "리포트를 출력하고(예: PDF) DB에 저장된 구조화 결과(JSON)와 연결했어요. 이제 논문 생성에서 재사용 가능합니다.",
                                ts: nowStamp(),
                              },
                            ]);
                          }}
                        >
                          <FileText className="mr-2 h-4 w-4" /> 리포트 출력 + 저장
                        </Button>

                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">권장 스키마(요약)</div>
                          <div className="mt-2 text-sm text-muted-foreground whitespace-pre-wrap">
                            {`Experiment: id, domain, promptType, prompt, approachId, params(alpha/seed), status, createdAt
Report: expId, highlights[], metrics[], narrative, limitations[], tables[], figures[]
Artifacts: expId, type(pdf/html), uri, checksum, createdAt`}
                          </div>
                        </div>
                      </CardContent>
                    </Card>
                  </div>
                </TabsContent>

                {/* 5) DB */}
                <TabsContent value="db" className="m-0">
                  <div className="p-4 grid gap-4 lg:grid-cols-2">
                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">DB 탐색</CardTitle>
                        <CardDescription>
                          도메인/태그/상태로 필터링하고 체크해서 논문 재료로 모아요.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">빠른 필터</div>
                          <div className="mt-2 flex flex-wrap gap-2">
                            <Button
                              size="sm"
                              className="rounded-2xl"
                              variant="secondary"
                              onClick={() => setDbDomain("IT 서비스 운영/DevOps")}
                            >
                              DevOps
                            </Button>
                            <Button
                              size="sm"
                              className="rounded-2xl"
                              variant="secondary"
                              onClick={() => setDbDomain("사이버보안")}
                            >
                              보안
                            </Button>
                            <Button
                              size="sm"
                              className="rounded-2xl"
                              variant="secondary"
                              onClick={() => setDbDomain("all")}
                            >
                              전체
                            </Button>
                          </div>
                        </div>

                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">선택된 실험</div>
                          <div className="mt-2 space-y-2">
                            {selectedExperiments.length ? (
                              selectedExperiments.map((e) => (
                                <div key={e.id} className="flex items-center justify-between">
                                  <div className="min-w-0">
                                    <div className="truncate text-sm font-medium">
                                      {e.title}
                                    </div>
                                    <div className="truncate text-xs text-muted-foreground">
                                      {e.domain} · {e.id}
                                    </div>
                                  </div>
                                  <Button
                                    size="sm"
                                    variant="ghost"
                                    className="rounded-2xl"
                                    onClick={() => toggleSelectExperiment(e.id, false)}
                                  >
                                    제거
                                  </Button>
                                </div>
                              ))
                            ) : (
                              <div className="text-sm text-muted-foreground">
                                아직 선택된 실험이 없어요.
                              </div>
                            )}
                          </div>
                        </div>

                        <Button
                          className="w-full rounded-2xl"
                          onClick={() => goTo("paper")}
                          disabled={selectedExperimentIds.length === 0}
                        >
                          <BookOpen className="mr-2 h-4 w-4" /> 선택 실험으로 논문 생성
                        </Button>
                      </CardContent>
                    </Card>

                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">정렬 UX 아이디어</CardTitle>
                        <CardDescription>
                          사용자가 원하는 기준으로 “실험 컬렉션”을 만들게 해요.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">컬렉션</div>
                          <div className="mt-2 text-sm text-muted-foreground">
                            예: “DevOps/MTTR”, “보안/패치”, “스마트시티/교통”처럼 사용자가 폴더를 만들고 실험을 드래그로 묶기.
                          </div>
                        </div>
                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">정렬 키(권장)</div>
                          <div className="mt-2 flex flex-wrap gap-2">
                            <Badge variant="secondary">도메인</Badge>
                            <Badge variant="secondary">태그</Badge>
                            <Badge variant="secondary">상태</Badge>
                            <Badge variant="secondary">효과크기</Badge>
                            <Badge variant="secondary">유의성</Badge>
                            <Badge variant="secondary">데이터 버전</Badge>
                          </div>
                        </div>
                        <Button
                          variant="secondary"
                          className="w-full rounded-2xl"
                          onClick={() => {
                            pushLog("INFO", "컬렉션 생성(데모)" );
                            setChat((prev) => [
                              ...prev,
                              {
                                role: "assistant",
                                content:
                                  "컬렉션 UX 좋아요. ‘논문에 넣을 실험 묶음’을 컬렉션으로 저장하면, 다음엔 클릭 한 번으로 논문을 재생성할 수 있어요.",
                                ts: nowStamp(),
                              },
                            ]);
                          }}
                        >
                          컬렉션 만들기(데모)
                        </Button>
                      </CardContent>
                    </Card>
                  </div>
                </TabsContent>

                {/* 6) Paper */}
                <TabsContent value="paper" className="m-0">
                  <div className="p-4 grid gap-4 lg:grid-cols-2">
                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">논문 생성</CardTitle>
                        <CardDescription>
                          체크된 실험들을 종합해 IMRaD 구조 초안을 만들어요.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">선택 실험</div>
                          <div className="mt-2 flex flex-wrap gap-2">
                            {selectedExperiments.map((e) => (
                              <Badge key={e.id} variant="secondary">
                                {e.id}
                              </Badge>
                            ))}
                            {selectedExperiments.length === 0 ? (
                              <span className="text-sm text-muted-foreground">
                                없음
                              </span>
                            ) : null}
                          </div>
                        </div>

                        <div className="grid gap-3 sm:grid-cols-2">
                          <div className="space-y-2">
                            <div className="text-sm font-medium">템플릿</div>
                            <Select defaultValue="imrad">
                              <SelectTrigger className="rounded-2xl">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent className="rounded-2xl">
                                <SelectItem value="imrad">IMRaD</SelectItem>
                                <SelectItem value="ieee">IEEE</SelectItem>
                                <SelectItem value="acm">ACM</SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                          <div className="space-y-2">
                            <div className="text-sm font-medium">언어</div>
                            <Select defaultValue="ko">
                              <SelectTrigger className="rounded-2xl">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent className="rounded-2xl">
                                <SelectItem value="ko">한국어</SelectItem>
                                <SelectItem value="en">English</SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                        </div>

                        <Button
                          className="w-full rounded-2xl"
                          disabled={selectedExperimentIds.length === 0}
                          onClick={() => {
                            pushLog("INFO", "논문 초안 생성(데모)" );
                            setChat((prev) => [
                              ...prev,
                              {
                                role: "assistant",
                                content:
                                  "선택한 실험들을 종합해 논문 초안을 만들었어요. 다음은 ‘논의/한계/향후연구’를 자동으로 더 탄탄하게 채우면 됩니다.",
                                ts: nowStamp(),
                              },
                            ]);
                          }}
                        >
                          <Wand2 className="mr-2 h-4 w-4" /> 초안 생성
                        </Button>

                        <div className="rounded-2xl border p-3">
                          <div className="text-sm font-semibold">내보내기(데모)</div>
                          <div className="mt-2 flex flex-wrap gap-2">
                            <Badge variant="secondary">.docx</Badge>
                            <Badge variant="secondary">.tex</Badge>
                            <Badge variant="secondary">PDF</Badge>
                          </div>
                          <div className="mt-3 flex gap-2">
                            <Button
                              className="rounded-2xl"
                              variant="secondary"
                              onClick={() => pushLog("INFO", "DOCX 내보내기(데모)" )}
                            >
                              DOCX
                            </Button>
                            <Button
                              className="rounded-2xl"
                              variant="secondary"
                              onClick={() => pushLog("INFO", "PDF 내보내기(데모)" )}
                            >
                              PDF
                            </Button>
                            <Button
                              className="rounded-2xl"
                              variant="secondary"
                              onClick={() => pushLog("INFO", "TEX 내보내기(데모)" )}
                            >
                              TEX
                            </Button>
                          </div>
                        </div>
                      </CardContent>
                    </Card>

                    <Card className="rounded-2xl">
                      <CardHeader>
                        <CardTitle className="text-base">논문 미리보기(데모)</CardTitle>
                        <CardDescription>
                          선택 실험들의 결과 문장을 자동으로 엮어줘요.
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <div className="rounded-2xl border bg-muted/30 p-3">
                          <div className="text-xs text-muted-foreground">Title</div>
                          <div className="mt-1 text-sm font-semibold">
                            {paperDraft.title}
                          </div>
                        </div>

                        <div className="rounded-2xl border bg-muted/30 p-3">
                          <div className="text-xs text-muted-foreground">Abstract</div>
                          <div className="mt-2 text-sm text-muted-foreground">
                            {paperDraft.abstract}
                          </div>
                        </div>

                        <div className="rounded-2xl border bg-muted/30 p-3">
                          <div className="text-xs text-muted-foreground">Methods</div>
                          <div className="mt-2 space-y-1 text-sm text-muted-foreground">
                            {paperDraft.methods.filter(Boolean).slice(0, 4).map((m, i) => (
                              <div key={i}>- {m}</div>
                            ))}
                          </div>
                        </div>

                        <div className="rounded-2xl border bg-muted/30 p-3">
                          <div className="text-xs text-muted-foreground">Results</div>
                          <div className="mt-2 space-y-1 text-sm text-muted-foreground">
                            {paperDraft.results.filter(Boolean).slice(0, 4).map((r, i) => (
                              <div key={i}>- {r}</div>
                            ))}
                          </div>
                        </div>

                        <div className="rounded-2xl border bg-muted/30 p-3">
                          <div className="text-xs text-muted-foreground">Limitations</div>
                          <div className="mt-2 space-y-1 text-sm text-muted-foreground">
                            {paperDraft.limitations.filter(Boolean).slice(0, 4).map((l, i) => (
                              <div key={i}>- {l}</div>
                            ))}
                          </div>
                        </div>
                      </CardContent>
                    </Card>
                  </div>
                </TabsContent>
              </Tabs>

              <div className="p-4 pt-0 flex items-center justify-end gap-2">
                <Button
                  variant="secondary"
                  className="rounded-2xl"
                  onClick={() => {
                    const idx = stepOrder.findIndex((s) => s.key === activeStep);
                    if (idx > 0) setActiveStep(stepOrder[idx - 1].key);
                  }}
                >
                  이전
                </Button>
                <Button className="rounded-2xl" onClick={goNext}>
                  다음
                </Button>
              </div>
            </CardContent>
          </Card>
        </main>

        {/* Right: Copilot + Logs */}
        <section className="col-span-12 lg:col-span-3">
          <Card className="rounded-2xl">
            <CardHeader className="pb-3">
              <CardTitle className="text-base">코파일럿</CardTitle>
              <CardDescription>
                추천/선택/실행/저장/논문까지 단계별로 안내해요.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="h-[320px] space-y-2 overflow-auto rounded-2xl border bg-muted/20 p-3">
                <AnimatePresence initial={false}>
                  {chat.map((m, idx) => (
                    <motion.div
                      key={idx}
                      initial={{ opacity: 0, y: 6 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0 }}
                      className={
                        "flex " + (m.role === "user" ? "justify-end" : "justify-start")
                      }
                    >
                      <div
                        className={
                          "max-w-[88%] rounded-2xl px-3 py-2 text-sm " +
                          (m.role === "user"
                            ? "bg-primary text-primary-foreground"
                            : "bg-card border")
                        }
                      >
                        <div className="whitespace-pre-wrap leading-5">{m.content}</div>
                        <div
                          className={
                            "mt-1 text-[11px] " +
                            (m.role === "user"
                              ? "text-primary-foreground/80"
                              : "text-muted-foreground")
                          }
                        >
                          {m.ts}
                        </div>
                      </div>
                    </motion.div>
                  ))}
                </AnimatePresence>
              </div>

              <div className="flex gap-2">
                <Input
                  className="rounded-2xl"
                  value={chatDraft}
                  onChange={(e) => setChatDraft(e.target.value)}
                  placeholder="예: 접근 방식 추천 기준을 알려줘"
                  onKeyDown={(e) => {
                    if (e.key === "Enter") sendChat();
                  }}
                />
                <Button className="rounded-2xl" onClick={sendChat}>
                  전송
                </Button>
              </div>

              <div className="rounded-2xl border p-3">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Sparkles className="h-4 w-4" /> 추천 액션
                </div>
                <div className="mt-2 grid gap-2">
                  <Button
                    variant="secondary"
                    className="justify-start rounded-2xl"
                    onClick={() => {
                      goTo("seed");
                      setChatDraft("가설을 더 검증 가능하게 다듬어줘");
                    }}
                  >
                    가설 문장 다듬기
                  </Button>
                  <Button
                    variant="secondary"
                    className="justify-start rounded-2xl"
                    onClick={() => {
                      goTo("approach");
                      setChatDraft("접근 방식 후보를 데이터 요건으로 비교해줘");
                    }}
                  >
                    후보 비교
                  </Button>
                  <Button
                    variant="secondary"
                    className="justify-start rounded-2xl"
                    onClick={() => {
                      goTo("paper");
                      setChatDraft("선택 실험으로 논문 초안 구조를 잡아줘");
                    }}
                  >
                    논문 구조 잡기
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card className="mt-4 rounded-2xl">
            <CardHeader className="pb-3">
              <CardTitle className="text-base">실행 로그</CardTitle>
              <CardDescription>실험/저장/논문 생성 흐름을 추적해요.</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="h-[220px] overflow-auto rounded-2xl border bg-muted/20 p-3 text-xs">
                {logs.map((l, i) => (
                  <div key={i} className="mb-2 flex gap-2">
                    <span className="text-muted-foreground">[{l.ts}]</span>
                    <span
                      className={
                        l.level === "ERROR"
                          ? "text-destructive"
                          : l.level === "WARN"
                          ? "text-amber-600"
                          : "text-foreground"
                      }
                    >
                      {l.level}
                    </span>
                    <span className="text-muted-foreground">·</span>
                    <span className="text-foreground">{l.msg}</span>
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>
        </section>
      </div>
    </div>
  );
}
