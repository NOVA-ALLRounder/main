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
    }[];
    missing_proofs: {
        date: string;
        merchant: string;
        amount: number;
        type: string;
    }[];
    factors: { name: string; status: "high" | "warning" | "good" | "low"; value: string }[];
}

export interface SimulationReq {
    salary_increase: boolean;
    vehicle_expense: boolean;
    rnd_credit: boolean;
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

    getRecommendations: async (): Promise<Recommendation[]> => {
        const res = await axios.get(`${API_BASE}/recommendations`);
        return res.data.recommendations;
    },

    getCompetitions: async (): Promise<Competition[]> => {
        const res = await axios.get(`${API_BASE}/competitions`);
        return res.data.competitions;
    },

    getDashboard: async (bizNum?: string, activeMCPs: string[] = []) => {
        const res = await axios.get(`${API_BASE}/dashboard`, { params: { biz_num: bizNum, active_mcps: activeMCPs.join(",") } });
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
    }
};
