# =============================================================================
# T_lab Agent - Experiment Runner (Statistical Simulation)
# Monte Carlo simulation from v_lab
# =============================================================================

from typing import Dict, Any
import json
import numpy as np
import asyncio
import random

# Visualization & Stats Imports (Safe & Robust)
import matplotlib
matplotlib.use('Agg') # Ensure non-GUI backend
import matplotlib.pyplot as plt
import seaborn as sns
from scipy import stats

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from state import ScientificState
from core import get_settings, get_logger
from connection_manager import ws_manager


settings = get_settings()
logger = get_logger("agents.experiment_runner")


PARAM_EXTRACTION_PROMPT = """You are a Data Scientist extracting simulation parameters from scientific literature.

TASK: Analyze the hypothesis and literature to determine if the hypothesis is SUPPORTED or REFUTED by existing research.

CRITICAL RULES:
1. CAREFULLY read the literature abstracts and understand their findings.
2. If the literature clearly REFUTES the hypothesis (e.g., shows opposite effect), you MUST:
   - Set "literature_supports" to false
   - Set experimental_group_mean EQUAL TO or LOWER THAN control_group_mean (effect_size ≈ 0 or negative)
   - Provide a clear "contradiction_reason" explaining why literature refutes the hypothesis
3. If the literature SUPPORTS the hypothesis, you may set a positive effect size.
4. If the literature is INCONCLUSIVE, set effect_size close to 0.

COMMON SCIENTIFIC FACTS TO CONSIDER:
- Air-entraining agents (AE) in concrete DECREASE strength (약 4-6% per 1% air content)
- Increased water-cement ratio DECREASES concrete strength
- Exercise generally REDUCES stress (positive effect)
- MIMO technologies significantly INCREASE wireless capacity

Return JSON:
{
    "control_group_mean": float,
    "control_group_std": float,
    "experimental_group_mean": float,
    "experimental_group_std": float,
    "sample_size": int,
    "effect_size": float,
    "power_analysis": float,
    "literature_supports": true/false,
    "contradiction_reason": "If literature_supports is false, explain WHY the hypothesis is refuted by literature. Otherwise null."
}
"""


class ExperimentRunnerAgent:
    """Runs statistical simulations based on literature data."""
    
    def __init__(self):
        self.llm = None
        if LANGCHAIN_AVAILABLE and settings.openai_api_key:
            self.llm = ChatOpenAI(
                model=settings.default_model,
                temperature=0.2,
                api_key=settings.openai_api_key
            )
    
    async def run(self, state: ScientificState) -> ScientificState:
        """Run experiment based on selected methodology type."""
        hypothesis = state.get('user_input', '')
        literature = state.get('literature_context', [])
        session_id = state.get('session_id', 'unknown')
        selected_method = state.get('selected_method', {})
        experiment_results = state.get('experiment_results', {})
        
        # Get methodology type
        method_type = selected_method.get('type', 'simulation')
        
        logger.info(f"Running experiment with methodology: {method_type}", source="experiment_runner")
        
        # Check if Engineer already produced SIMULATION_RESULTS
        engineer_sim_results = experiment_results.get('simulation_results')
        
        if engineer_sim_results:
            # Use Engineer's domain-specific simulation results
            logger.info("Using Engineer's SIMULATION_RESULTS (Polymorphic)", source="experiment_runner")
            
            results = {
                "p_value": engineer_sim_results.get('p_value', 0.05),
                "significant_difference": engineer_sim_results.get('significant', False),
                "effect_size": engineer_sim_results.get('effect_size', 0.0),
                "test_statistic": engineer_sim_results.get('test_statistic', 0.0),
                "conclusion": engineer_sim_results.get('conclusion', ''),
                "source": "engineer_code",
                "method_type": method_type
            }
            
            state['simulation_params'] = {
                "method_type": method_type,  # Add method_type for Frontend
                "source": "engineer_code",
                "model_type": engineer_sim_results.get('model_type', 'domain_specific'),
                "effect_size": engineer_sim_results.get('effect_size', 0.0)
            }
            
        else:
            # Route to appropriate methodology pipeline
            if method_type == 'analytical':
                results = await self._run_analytical(state, session_id)
            elif method_type == 'data_driven':
                results = await self._run_data_driven(state, session_id)
            else:  # 'simulation' or default
                # Extract parameters and run Monte Carlo
                logger.info("Running Monte Carlo simulation", source="experiment_runner")
                
                if self.llm:
                    params = self._extract_params_with_llm(hypothesis, literature, selected_method)
                else:
                    params = self._extract_mock_params()
                
                state['simulation_params'] = params
                state['simulation_params']['method_type'] = 'simulation'  # Add for Frontend
                results = await self._run_monte_carlo(params, session_id)
                results["source"] = "monte_carlo"
                results["method_type"] = "simulation"
        
        # Store results
        state['simulation_results'] = results
        state['current_step'] = 'simulation_completed'
        
        # Add to logic chain
        state['logic_chain'] = state.get('logic_chain', [])
        state['logic_chain'].append({
            "step": "experiment",
            "method_type": method_type,
            "p_value": results.get('p_value'),
            "significant": results.get('significant_difference'),
            "source": results.get('source', 'unknown')
        })
        
        return state
    
    
    def _extract_params_with_llm(self, hypothesis: str, literature: list, method: Dict[str, Any] = None) -> Dict[str, Any]:
        """Extract simulation parameters using LLM."""
        lit_text = "\n".join([f"- {p.get('title', '')}: {p.get('abstract', '')[:200]}" for p in literature[:5]])
        
        prompt_content = f"User Input: {hypothesis}\n"
        if method:
             h1 = method.get('hypothesis', {}).get('h1', '')
             if h1:
                 prompt_content += f"Specific Hypothesis (H1): {h1}\n"
        
        prompt_content += f"\nLiterature:\n{lit_text}"
        
        messages = [
            SystemMessage(content=PARAM_EXTRACTION_PROMPT),
            HumanMessage(content=prompt_content)
        ]
        
        try:
            response = self.llm.invoke(messages)
            content = response.content
            if '```json' in content:
                content = content.split('```json')[1].split('```')[0]
            params = json.loads(content.strip())
            return params
        except:
            return self._extract_mock_params()
    
    def _extract_mock_params(self) -> Dict[str, Any]:
        """Mock parameter extraction with randomized support."""
        # 70% chance of hypothesis support
        supports = random.random() < 0.7
        
        control_mean = 50.0
        control_std = 10.0
        
        # Calculate effect size (Cohen's d)
        # If supports, draw from Normal(0.5, 0.2), else from Normal(0.0, 0.1)
        if supports:
            effect_size = max(0.1, np.random.normal(0.5, 0.2))
        else:
            effect_size = np.random.normal(0.0, 0.1)
            
        diff = effect_size * control_std
        
        return {
            "control_group_mean": float(control_mean),
            "control_group_std": float(control_std),
            "experimental_group_mean": float(control_mean + diff),
            "experimental_group_std": float(control_std),
            "sample_size": 1000, # Target N
            "effect_size": float(effect_size),
            "literature_supports": supports
        }
    
    async def _run_analytical(self, state: ScientificState, session_id: str) -> Dict[str, Any]:
        """Run Analytical (Theoretical) Analysis using mathematical models."""
        hypothesis = state.get('user_input', '')
        selected_method = state.get('selected_method', {})
        domain = state.get('domain', 'General')
        
        logger.info("Running analytical (theoretical) analysis", source="experiment_runner")
        
        # Broadcast start
        await ws_manager.broadcast(session_id, {
            "type": "analytical_start",
            "message": "이론적 분석을 시작합니다..."
        })
        
        if self.llm:
            # Use LLM to derive mathematical model and calculate
            analytical_prompt = f"""당신은 수학적 모델링 전문가입니다.

가설: {hypothesis}
도메인: {domain}
방법론: {selected_method.get('methodology', '')}

다음 형식의 JSON으로 이론적 분석 결과를 제공하세요:
{{
    "model_name": "사용된 수학 모델 이름",
    "formula": "수학 공식 (LaTeX 형식)",
    "variables": {{"var1": "설명", "var2": "설명"}},
    "theoretical_prediction": "이론적 예측값 또는 범위",
    "calculation_steps": ["단계 1", "단계 2", "단계 3"],
    "conclusion": "이론적 결론 (가설 지지/기각)",
    "hypothesis_supported": true/false,
    "confidence": 0.0-1.0,
    "effect_magnitude": "small/medium/large",
    "scientific_basis": "이론적 근거 설명"
}}

실제 과학적 지식을 바탕으로 분석하세요."""

            try:
                from langchain_core.messages import SystemMessage, HumanMessage
                response = self.llm.invoke([
                    SystemMessage(content="You are a mathematical modeling expert."),
                    HumanMessage(content=analytical_prompt)
                ])
                content = response.content
                if '```json' in content:
                    content = content.split('```json')[1].split('```')[0]
                analysis = json.loads(content.strip())
            except Exception as e:
                logger.warning(f"LLM analytical analysis failed: {e}", source="experiment_runner")
                analysis = self._mock_analytical_result()
        else:
            analysis = self._mock_analytical_result()
        
        # Calculate pseudo p-value based on confidence
        confidence = analysis.get('confidence', 0.5)
        p_value = max(0.001, 1 - confidence) if analysis.get('hypothesis_supported') else min(0.99, 1 - confidence)
        
        # Broadcast result
        await ws_manager.broadcast(session_id, {
            "type": "analytical_result",
            "model": analysis.get('model_name'),
            "formula": analysis.get('formula'),
            "conclusion": analysis.get('conclusion')
        })
        
        # Set simulation params for consistency
        state['simulation_params'] = {
            "method_type": "analytical",
            "model_name": analysis.get('model_name'),
            "formula": analysis.get('formula')
        }
        
        return {
            "method_type": "analytical",
            "source": "theoretical_analysis",
            "model_name": analysis.get('model_name', 'Unknown Model'),
            "formula": analysis.get('formula', ''),
            "variables": analysis.get('variables', {}),
            "theoretical_prediction": analysis.get('theoretical_prediction', ''),
            "calculation_steps": analysis.get('calculation_steps', []),
            "conclusion": analysis.get('conclusion', ''),
            "p_value": float(p_value),
            "significant_difference": bool(analysis.get('hypothesis_supported', False)),
            "effect_size": {"small": 0.2, "medium": 0.5, "large": 0.8}.get(analysis.get('effect_magnitude', 'medium'), 0.5),
            "confidence": float(confidence),
            "scientific_basis": analysis.get('scientific_basis', '')
        }
    
    def _mock_analytical_result(self) -> Dict[str, Any]:
        """Mock analytical result when LLM is unavailable."""
        return {
            "model_name": "일반 선형 모델 (General Linear Model)",
            "formula": "Y = β₀ + β₁X + ε",
            "variables": {"Y": "종속 변수", "X": "독립 변수", "β₁": "효과 계수"},
            "theoretical_prediction": "β₁ > 0 (양의 효과 예측)",
            "calculation_steps": [
                "1. 이론적 모델 설정",
                "2. 파라미터 추정",
                "3. 효과 크기 계산"
            ],
            "conclusion": "이론적 분석 결과, 가설이 지지됩니다.",
            "hypothesis_supported": True,
            "confidence": 0.75,
            "effect_magnitude": "medium",
            "scientific_basis": "일반적인 과학적 원리에 기반"
        }
    
    async def _run_data_driven(self, state: ScientificState, session_id: str) -> Dict[str, Any]:
        """Run Data-driven Analysis using synthetic dataset and regression."""
        hypothesis = state.get('user_input', '')
        selected_method = state.get('selected_method', {})
        domain = state.get('domain', 'General')
        
        logger.info("Running data-driven (regression) analysis", source="experiment_runner")
        
        # Broadcast start
        await ws_manager.broadcast(session_id, {
            "type": "data_driven_start",
            "message": "데이터 기반 분석을 시작합니다..."
        })
        
        if self.llm:
            # Use LLM to generate synthetic data parameters
            data_prompt = f"""당신은 데이터 과학자입니다.

가설: {hypothesis}
도메인: {domain}

이 가설을 검증하기 위한 합성 데이터셋의 파라미터를 JSON으로 제공하세요:
{{
    "independent_var": "독립 변수 이름",
    "dependent_var": "종속 변수 이름",
    "sample_size": 100-500 사이의 정수,
    "true_slope": -2.0에서 2.0 사이의 실수 (가설이 지지되면 양수, 아니면 0에 가깝거나 음수),
    "intercept": 기준값,
    "noise_level": 0.1-1.0 사이의 노이즈 수준,
    "expected_r_squared": 0.0-1.0 사이의 예상 R² 값,
    "hypothesis_supported": true/false,
    "scientific_reasoning": "데이터 분석 결과에 대한 과학적 해석"
}}

실제 과학적 지식을 바탕으로 파라미터를 설정하세요."""

            try:
                from langchain_core.messages import SystemMessage, HumanMessage
                response = self.llm.invoke([
                    SystemMessage(content="You are a data scientist."),
                    HumanMessage(content=data_prompt)
                ])
                content = response.content
                if '```json' in content:
                    content = content.split('```json')[1].split('```')[0]
                params = json.loads(content.strip())
            except Exception as e:
                logger.warning(f"LLM data params failed: {e}", source="experiment_runner")
                params = self._mock_data_params()
        else:
            params = self._mock_data_params()
        
        # Generate synthetic data
        n = int(params.get('sample_size', 200))
        true_slope = float(params.get('true_slope', 0.5))
        intercept = float(params.get('intercept', 10.0))
        noise_level = float(params.get('noise_level', 0.5))
        
        np.random.seed(42)  # Reproducibility
        X = np.random.uniform(0, 100, n)
        noise = np.random.normal(0, noise_level * np.std(X), n)
        Y = intercept + true_slope * X + noise
        
        # Broadcast data generation
        await ws_manager.broadcast(session_id, {
            "type": "data_point",
            "iteration": n,
            "message": f"합성 데이터 {n}개 생성 완료"
        })
        
        # Run Linear Regression
        from scipy import stats as sp_stats
        slope, intercept_fit, r_value, p_value, std_err = sp_stats.linregress(X, Y)
        r_squared = r_value ** 2
        
        # Effect size (standardized slope)
        effect_size = abs(slope) / np.std(Y)
        
        # Broadcast regression result
        await ws_manager.broadcast(session_id, {
            "type": "regression_result",
            "r_squared": float(r_squared),
            "slope": float(slope),
            "p_value": float(p_value)
        })
        
        # Generate regression plot
        try:
            plt.figure(figsize=(10, 6))
            plt.scatter(X, Y, alpha=0.5, label='Data Points')
            plt.plot(X, intercept_fit + slope * X, color='red', linewidth=2, label=f'Regression Line (R²={r_squared:.3f})')
            plt.xlabel(params.get('independent_var', 'X'))
            plt.ylabel(params.get('dependent_var', 'Y'))
            plt.title(f"Data-driven Analysis: {params.get('independent_var', 'X')} vs {params.get('dependent_var', 'Y')}")
            plt.legend()
            plt.savefig(f"static/{session_id}_result.png")
            plt.close()
        except Exception as e:
            logger.error(f"Failed to generate regression plot: {e}", source="experiment_runner")
        
        # Set simulation params
        state['simulation_params'] = {
            "method_type": "data_driven",
            "sample_size": n,
            "independent_var": params.get('independent_var'),
            "dependent_var": params.get('dependent_var')
        }
        
        significant = p_value < 0.05 and (true_slope > 0 if params.get('hypothesis_supported') else true_slope <= 0)
        
        return {
            "method_type": "data_driven",
            "source": "regression_analysis",
            "sample_size": int(n),
            "independent_var": params.get('independent_var', 'X'),
            "dependent_var": params.get('dependent_var', 'Y'),
            "regression_coefficient": float(slope),
            "intercept": float(intercept_fit),
            "r_squared": float(r_squared),
            "r_value": float(r_value),
            "p_value": float(p_value),
            "std_error": float(std_err),
            "significant_difference": bool(p_value < 0.05),
            "effect_size": float(effect_size),
            "conclusion": params.get('scientific_reasoning', ''),
            "n": int(n)
        }
    
    def _mock_data_params(self) -> Dict[str, Any]:
        """Mock data parameters when LLM is unavailable."""
        return {
            "independent_var": "X (독립 변수)",
            "dependent_var": "Y (종속 변수)",
            "sample_size": 200,
            "true_slope": 0.5,
            "intercept": 10.0,
            "noise_level": 0.3,
            "expected_r_squared": 0.7,
            "hypothesis_supported": True,
            "scientific_reasoning": "회귀 분석 결과, 독립 변수와 종속 변수 간에 통계적으로 유의미한 관계가 확인되었습니다."
        }
    
    async def _run_monte_carlo(self, params: Dict[str, Any], session_id: str) -> Dict[str, Any]:
        """Run Sequential Analysis Simulation."""
        target_n = int(params.get('sample_size', 100))
        control_mean = float(params.get('control_group_mean', 50.0))
        control_std = float(params.get('control_group_std', 10.0))
        exp_mean = float(params.get('experimental_group_mean', 55.0))
        exp_std = float(params.get('experimental_group_std', 10.0))
        
        # Simulation parameters
        start_n = 10
        current_n = start_n
        
        # Arrays to store streaming data
        p_values = []
        significant = False
        
        # Stream updates
        while current_n <= target_n:
            # Generate cumulative samples
            control = np.random.normal(control_mean, control_std, current_n)
            experimental = np.random.normal(exp_mean, exp_std, current_n)
            
            # T-test
            t_stat, p_val = stats.ttest_ind(control, experimental)
            
            # Calculate Metrics
            # 1. Effect Size (Cohen's d)
            mean_diff = np.mean(experimental) - np.mean(control)
            pooled_std = np.sqrt((np.std(control, ddof=1)**2 + np.std(experimental, ddof=1)**2) / 2)
            current_d = mean_diff / pooled_std if pooled_std != 0 else 0
            
            # 2. Statistical Power (Post-hoc)
            # Power = P(Reject H0 | H1 is true)
            try:
                # Non-centrality parameter (NCP) for t-test
                # For equal sample sizes n1=n2=n: ncp = d * sqrt(n/2)
                ncp = current_d * np.sqrt(current_n / 2)
                
                # Critical t-value for alpha=0.05 (two-tailed)
                # Degrees of freedom = 2n - 2
                df = 2 * current_n - 2
                crit_t = stats.t.ppf(1 - 0.05/2, df=df)
                
                # Power calculation using Non-central T distribution
                # Power = 1 - beta = P(T > crit_t | delta) + P(T < -crit_t | delta)
                power_right = 1 - stats.nct.cdf(crit_t, df, nc=ncp)
                power_left = stats.nct.cdf(-crit_t, df, nc=ncp)
                current_power = power_right + power_left
                
            except Exception as e:
                # Fallback if calculation fails (e.g. overflow)
                current_power = 0.0
            
            # Broadcast update - ensure JSON serializable types
            await ws_manager.broadcast(session_id, {
                "type": "data_point",
                "iteration": int(current_n), # Using 'iteration' as 'N' for frontend compat
                "p_value": float(p_val) if not np.isnan(p_val) and not np.isinf(p_val) else 0.0,
                "power": float(current_power) if not np.isnan(current_power) else 0.0,
                "effect_size": float(current_d) if not np.isnan(current_d) else 0.0,
                "significant_count": 1 if p_val < 0.05 else 0 # Just for visual compatibility
            })
            
            p_values.append(float(p_val))
            
            # Step size
            current_n += 2 # Add 2 subjects per step
            await asyncio.sleep(0.1) # Visualize the process
            
        final_p = p_values[-1] if p_values else 0.5
        
        # Generate a static plot for the report
        try:
            plt.figure(figsize=(10, 6))
            sns.histplot(control, color='gray', label='Control', kde=True, alpha=0.5)
            sns.histplot(experimental, color='blue', label='Experimental', kde=True, alpha=0.5)
            plt.axvline(np.mean(control), color='gray', linestyle='--')
            plt.axvline(np.mean(experimental), color='blue', linestyle='--')
            plt.title(f"Final Distribution (N={target_n})\np={final_p:.4f}, Cohen's d={params['effect_size']:.2f}")
            plt.legend()
            
            # Save plot
            save_path = f"static/{session_id}_result.png"
            plt.savefig(save_path)
            plt.close()
        except Exception as e:
            logger.error(f"Failed to generate plot: {e}", source="experiment_runner")
            try:
                # Create a dummy plot to prevent frontend 404
                plt.figure()
                plt.text(0.5, 0.5, "Plot Generation Failed", ha='center')
                plt.savefig(f"static/{session_id}_result.png")
                plt.close()
            except:
                pass
        
        return {
            "simulation_type": "Sequential Analysis (T-Test)",
            "iterations": int(target_n),
            "p_value": float(final_p),
            "confidence_interval": "95%",
            "significant_difference": bool(final_p < 0.05),
            "control_stats": {"mean": float(control_mean), "std": float(control_std)},
            "experimental_stats": {"mean": float(exp_mean), "std": float(exp_std)}
        }


async def run_simulation(state: ScientificState) -> ScientificState:
    """Entry point for experiment runner."""
    agent = ExperimentRunnerAgent()
    return await agent.run(state)
