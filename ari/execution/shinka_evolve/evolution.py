"""
ShinkaEvolve

LLM 기반 진화적 코드 최적화
"""

from typing import List, Dict, Any, Optional, Callable, Tuple
from dataclasses import dataclass, field
import random

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client
from core.logger import get_logger

logger = get_logger("shinka_evolve")


@dataclass
class CodeVariant:
    """코드 변형"""
    variant_id: str
    code: str
    description: str = ""
    
    # 성능
    fitness: float = 0.0
    metrics: Dict[str, float] = field(default_factory=dict)
    
    # 계보
    parent_ids: List[str] = field(default_factory=list)
    generation: int = 0
    mutation_type: str = ""  # mutation, crossover, original
    
    def __post_init__(self):
        if not self.variant_id:
            import hashlib
            self.variant_id = hashlib.md5(self.code.encode()).hexdigest()[:8]
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "variant_id": self.variant_id,
            "code": self.code,
            "description": self.description,
            "fitness": self.fitness,
            "metrics": self.metrics,
            "parent_ids": self.parent_ids,
            "generation": self.generation,
            "mutation_type": self.mutation_type
        }


class ShinkaEvolve:
    """LLM 기반 진화적 코드 최적화"""
    
    MUTATION_PROMPT = """You are an expert at optimizing code through evolutionary mutation.

Given this code that achieved a fitness score of {fitness}:

```python
{code}
```

Current metrics: {metrics}

Generate a MUTATED version that might improve performance.
Try one of these mutation strategies:
1. Change hyperparameters (learning rate, batch size, etc.)
2. Add/remove/modify a component
3. Change the algorithm or approach slightly
4. Optimize computational efficiency

Return as JSON:
{{
    "mutated_code": "the mutated Python code",
    "mutation_description": "what was changed and why",
    "expected_improvement": "what improvement is expected"
}}
"""
    
    CROSSOVER_PROMPT = """You are combining the best features of two code variants.

Parent 1 (fitness: {fitness1}):
```python
{code1}
```

Parent 2 (fitness: {fitness2}):
```python
{code2}
```

Create a CHILD that combines the best features of both parents.
Keep what works well from the higher-scoring parent while incorporating
useful elements from the other.

Return as JSON:
{{
    "child_code": "the combined Python code",
    "crossover_description": "what features were combined from each parent"
}}
"""
    
    def __init__(
        self,
        llm_client: Optional[LLMClient] = None,
        evaluator: Optional[Callable[[str], Tuple[float, Dict[str, float]]]] = None,
        population_size: int = 10,
        mutation_rate: float = 0.3,
        crossover_rate: float = 0.5,
        elitism_count: int = 2
    ):
        """
        ShinkaEvolve 초기화
        
        Args:
            llm_client: LLM 클라이언트
            evaluator: 적합도 평가 함수 (code -> (fitness, metrics))
            population_size: 인구 크기
            mutation_rate: 돌연변이 확률
            crossover_rate: 교차 확률
            elitism_count: 엘리트 보존 수
        """
        self.llm = llm_client or get_llm_client()
        self.evaluator = evaluator
        
        self.population_size = population_size
        self.mutation_rate = mutation_rate
        self.crossover_rate = crossover_rate
        self.elitism_count = elitism_count
        
        self.population: List[CodeVariant] = []
        self.generation = 0
        self.best_variant: Optional[CodeVariant] = None
        self.history: List[Dict[str, Any]] = []
    
    def set_evaluator(
        self,
        evaluator: Callable[[str], Tuple[float, Dict[str, float]]]
    ):
        """평가 함수 설정"""
        self.evaluator = evaluator
    
    def initialize_population(
        self,
        seed_code: str,
        num_variants: int = None
    ) -> List[CodeVariant]:
        """
        초기 인구 생성
        
        Args:
            seed_code: 시드 코드
            num_variants: 초기 변형 수
        
        Returns:
            초기 인구
        """
        num_variants = num_variants or self.population_size
        
        # 시드 코드를 첫 번째 개체로
        seed_variant = CodeVariant(
            variant_id="seed",
            code=seed_code,
            description="Original seed code",
            generation=0,
            mutation_type="original"
        )
        
        self.population = [seed_variant]
        
        # LLM으로 변형 생성
        for i in range(num_variants - 1):
            mutant = self._mutate(seed_variant)
            if mutant:
                self.population.append(mutant)
        
        # 적합도 평가
        self._evaluate_population()
        
        return self.population
    
    def _mutate(self, parent: CodeVariant) -> Optional[CodeVariant]:
        """돌연변이 생성"""
        prompt = self.MUTATION_PROMPT.format(
            code=parent.code[:5000],
            fitness=parent.fitness,
            metrics=parent.metrics
        )
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are an evolutionary optimization expert.",
                temperature=0.8
            )
            
            return CodeVariant(
                variant_id="",
                code=response.get("mutated_code", ""),
                description=response.get("mutation_description", ""),
                parent_ids=[parent.variant_id],
                generation=parent.generation + 1,
                mutation_type="mutation"
            )
        
        except Exception as e:
            logger.error(f"Mutation failed: {e}")
            return None
    
    def _crossover(
        self,
        parent1: CodeVariant,
        parent2: CodeVariant
    ) -> Optional[CodeVariant]:
        """교차 연산"""
        prompt = self.CROSSOVER_PROMPT.format(
            code1=parent1.code[:3000],
            fitness1=parent1.fitness,
            code2=parent2.code[:3000],
            fitness2=parent2.fitness
        )
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are combining code variants for optimization.",
                temperature=0.6
            )
            
            return CodeVariant(
                variant_id="",
                code=response.get("child_code", ""),
                description=response.get("crossover_description", ""),
                parent_ids=[parent1.variant_id, parent2.variant_id],
                generation=max(parent1.generation, parent2.generation) + 1,
                mutation_type="crossover"
            )
        
        except Exception as e:
            logger.error(f"Crossover failed: {e}")
            return None
    
    def _evaluate_population(self):
        """전체 인구 평가"""
        if not self.evaluator:
            logger.warning("No evaluator set, skipping evaluation")
            return
        
        for variant in self.population:
            if variant.fitness == 0:  # 아직 평가 안됨
                try:
                    fitness, metrics = self.evaluator(variant.code)
                    variant.fitness = fitness
                    variant.metrics = metrics
                except Exception as e:
                    logger.error(f"Evaluation failed for {variant.variant_id}: {e}")
                    variant.fitness = 0.0
        
        # 최고 개체 업데이트
        sorted_pop = sorted(self.population, key=lambda v: v.fitness, reverse=True)
        if sorted_pop and (not self.best_variant or sorted_pop[0].fitness > self.best_variant.fitness):
            self.best_variant = sorted_pop[0]
    
    def _select_parents(self, k: int = 2) -> List[CodeVariant]:
        """토너먼트 선택"""
        selected = []
        
        for _ in range(k):
            tournament_size = min(3, len(self.population))
            tournament = random.sample(self.population, tournament_size)
            winner = max(tournament, key=lambda v: v.fitness)
            selected.append(winner)
        
        return selected
    
    def evolve_generation(self) -> List[CodeVariant]:
        """한 세대 진화"""
        if len(self.population) < 2:
            return self.population
        
        # 적합도로 정렬
        sorted_pop = sorted(self.population, key=lambda v: v.fitness, reverse=True)
        
        # 엘리트 보존
        new_population = sorted_pop[:self.elitism_count]
        
        # 나머지 채우기
        while len(new_population) < self.population_size:
            if random.random() < self.crossover_rate and len(self.population) >= 2:
                # 교차
                parents = self._select_parents(2)
                child = self._crossover(parents[0], parents[1])
                if child:
                    new_population.append(child)
            
            elif random.random() < self.mutation_rate:
                # 돌연변이
                parent = self._select_parents(1)[0]
                mutant = self._mutate(parent)
                if mutant:
                    new_population.append(mutant)
            
            else:
                # 복사
                parent = self._select_parents(1)[0]
                new_population.append(parent)
        
        self.population = new_population[:self.population_size]
        self.generation += 1
        
        # 평가
        self._evaluate_population()
        
        # 히스토리 기록
        self.history.append({
            "generation": self.generation,
            "best_fitness": self.best_variant.fitness if self.best_variant else 0,
            "avg_fitness": sum(v.fitness for v in self.population) / len(self.population),
            "population_size": len(self.population)
        })
        
        return self.population
    
    def evolve(
        self,
        seed_code: str,
        num_generations: int = 10,
        target_fitness: float = None,
        early_stop_patience: int = 3
    ) -> CodeVariant:
        """
        진화 실행
        
        Args:
            seed_code: 시드 코드
            num_generations: 최대 세대 수
            target_fitness: 목표 적합도
            early_stop_patience: 개선 없을 시 조기 종료 인내
        
        Returns:
            최적 개체
        """
        # 초기화
        self.initialize_population(seed_code)
        
        no_improvement = 0
        prev_best = 0.0
        
        for gen in range(num_generations):
            self.evolve_generation()
            
            current_best = self.best_variant.fitness if self.best_variant else 0
            
            logger.info(f"Generation {gen + 1}: Best fitness = {current_best:.4f}")
            
            # 목표 도달
            if target_fitness and current_best >= target_fitness:
                logger.info(f"Target fitness {target_fitness} reached!")
                break
            
            # 조기 종료
            if current_best <= prev_best:
                no_improvement += 1
                if no_improvement >= early_stop_patience:
                    logger.info(f"Early stopping after {no_improvement} generations without improvement")
                    break
            else:
                no_improvement = 0
            
            prev_best = current_best
        
        return self.best_variant
    
    def get_stats(self) -> Dict[str, Any]:
        """진화 통계"""
        return {
            "generation": self.generation,
            "population_size": len(self.population),
            "best_fitness": self.best_variant.fitness if self.best_variant else 0,
            "best_variant_id": self.best_variant.variant_id if self.best_variant else None,
            "history": self.history
        }
