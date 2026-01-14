import axios from 'axios';

const API_BASE = '/api';

export interface ChatMessage {
    role: 'user' | 'assistant';
    content: string;
    context?: Array<{ content: string; source: string }>;
}

export interface Recommendation {
    title: string;
    predicted_date: string;
    range: string;
    confidence: string;
    reason: string;
}

export interface Competition {
    platform: string;
    title: string;
    description: string;
    deadline: string;
    link: string;
    tags: string[];
}

export interface RiskAnalysis {
    level: "critical" | "warning" | "safe";
    score: number;
    title: string;
    reason: string;
    estimated_penalty: number;
    action_required: string;
    action_items: {
        task: string;
        amount: number;
        deadline: string;
        risk_reduction: number;
        description?: string;
        references?: { title: string; url: string }[];
    }[];
    missing_proofs: {
        date: string;
        merchant: string;
        amount: number;
        type: string;
    }[];
    factors: { name: string; status: "high" | "warning" | "good" | "low"; value: string }[];
}

// 업종별 다양한 절세 항목 지원을 위한 동적 타입
export interface SimulationReq {
    [key: string]: boolean;  // 동적 키 지원 (salary_increase, equipment_depreciation, inventory_valuation 등)
}

export interface SimulationRes {
    total_saving: number;
    details: Array<{ item: string, amount: number }>;
    message: string;
}

export interface CalendarAlert {
    date: string;
    d_day: number;
    title: string;
    type: "mandatory" | "critical" | "routine";
}

export const api = {
    chat: async (message: string, history: ChatMessage[]) => {
        const res = await axios.post(`${API_BASE}/chat`, { message, history });
        return res.data;
    },

    getRecommendations: async (type: string = 'startup'): Promise<Recommendation[]> => {
        const res = await axios.get(`${API_BASE}/recommendations?type=${type}`);
        return res.data.recommendations;
    },

    getCompetitions: async (): Promise<Competition[]> => {
        const res = await axios.get(`${API_BASE}/competitions`);
        return res.data.competitions;
    },

    getDashboard: async (bizNum?: string, activeMCPs: string[] = [], targetRevenue?: number, teamSize?: number, monthlyBudget?: number) => {
        const res = await axios.get(`${API_BASE}/dashboard`, {
            params: {
                biz_num: bizNum,
                active_mcps: activeMCPs.join(","),
                target_revenue: targetRevenue,
                team_size: teamSize || 0,
                monthly_budget: monthlyBudget || 0
            }
        });
        return res.data;
    },

    // Advanced SaaS
    getTaxRisk: async (bizNum?: string, activeMCPs: string[] = []) => {
        const res = await axios.get(`${API_BASE}/analysis/risk`, { params: { biz_num: bizNum, active_mcps: activeMCPs.join(",") } });
        return res.data;
    },

    simulateTax: async (req: SimulationReq): Promise<SimulationRes> => {
        const res = await axios.post(`${API_BASE}/tools/simulator`, req);
        return res.data;
    },

    getCalendarAlerts: async (): Promise<{ alerts: CalendarAlert[] }> => {
        const res = await axios.get(`${API_BASE}/calendar/alerts`);
        return res.data;
    },

    // 공공데이터 사업자조회 API
    lookupBusiness: async (bizNum: string) => {
        const res = await axios.get(`${API_BASE}/business/lookup`, { params: { biz_num: bizNum } });
        return res.data;
    },

    // 재무제표 분석 API
    getFinancialAnalysis: async (revenue: number, industry: string = 'startup') => {
        const res = await axios.get(`${API_BASE}/financial/analysis`, {
            params: { revenue, industry }
        });
        return res.data;
    },

    getFinancialHealth: async (revenue: number, industry: string = 'startup') => {
        const res = await axios.get(`${API_BASE}/financial/health`, {
            params: { revenue, industry }
        });
        return res.data;
    },

    // Auth APIs
    register: async (email: string, password: string, name: string, company: string) => {
        const res = await axios.post(`${API_BASE}/auth/register`, { email, password, name, company });
        return res.data;
    },

    login: async (email: string, password: string) => {
        const res = await axios.post(`${API_BASE}/auth/login`, { email, password });
        return res.data;
    },

    completeOnboarding: async (email: string, bizNum: string, type: string, targetRevenue?: number) => {
        const res = await axios.post(`${API_BASE}/auth/complete-onboarding`, {
            email,
            biz_num: bizNum,
            type,
            target_revenue: targetRevenue
        });
        return res.data;
    },

    changePassword: async (email: string, currentPassword: string, newPassword: string) => {
        const res = await axios.post(`${API_BASE}/auth/change-password`, {
            email,
            current_password: currentPassword,
            new_password: newPassword
        });
        return res.data;
    },

    updateMCPs: async (email: string, activeMCPs: string[]) => {
        const res = await axios.post(`${API_BASE}/auth/update-mcps`, {
            email,
            active_mcps: activeMCPs
        });
        return res.data;
    },

    // 국세청 전자문서 API
    uploadNTSDocument: async (file: File, password: string = "") => {
        const formData = new FormData();
        formData.append("file", file);
        formData.append("password", password);
        const res = await axios.post(`${API_BASE}/nts/upload-document`, formData, {
            headers: { "Content-Type": "multipart/form-data" }
        });
        return res.data;
    },

    getNTSDocumentTypes: async () => {
        const res = await axios.get(`${API_BASE}/nts/document-types`);
        return res.data;
    }
};
