"""
Code Executor

안전한 코드 실행 환경
"""

import subprocess
import tempfile
import os
import json
import signal
from pathlib import Path
from typing import Dict, Any, Optional, Tuple, List
from dataclasses import dataclass, field
from datetime import datetime
import threading


@dataclass
class ExecutionResult:
    """코드 실행 결과"""
    success: bool
    return_code: int
    stdout: str
    stderr: str
    execution_time: float
    
    # 추출된 결과
    metrics: Dict[str, float] = field(default_factory=dict)
    output_files: List[str] = field(default_factory=list)
    
    # 메타데이터
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    timed_out: bool = False
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "success": self.success,
            "return_code": self.return_code,
            "stdout": self.stdout[:5000],  # Truncate
            "stderr": self.stderr[:2000],
            "execution_time": self.execution_time,
            "metrics": self.metrics,
            "output_files": self.output_files,
            "timed_out": self.timed_out
        }


class CodeExecutor:
    """코드 실행기"""
    
    def __init__(
        self,
        timeout: int = 300,
        working_dir: str = None,
        python_path: str = "python3"
    ):
        """
        코드 실행기 초기화
        
        Args:
            timeout: 실행 제한 시간 (초)
            working_dir: 작업 디렉토리
            python_path: Python 인터프리터 경로
        """
        self.timeout = timeout
        self.working_dir = Path(working_dir) if working_dir else Path(tempfile.mkdtemp())
        self.python_path = python_path
        
        self.working_dir.mkdir(parents=True, exist_ok=True)
    
    def execute(
        self,
        code: str,
        filename: str = "experiment.py",
        env_vars: Dict[str, str] = None,
        capture_metrics: bool = True
    ) -> ExecutionResult:
        """
        코드 실행
        
        Args:
            code: 실행할 Python 코드
            filename: 파일 이름
            env_vars: 환경 변수
            capture_metrics: 메트릭 캡처 여부
        
        Returns:
            ExecutionResult 객체
        """
        # 코드 파일 저장
        code_file = self.working_dir / filename
        
        # 메트릭 캡처 코드 추가
        if capture_metrics:
            code = self._add_metric_capture(code)
        
        with open(code_file, 'w', encoding='utf-8') as f:
            f.write(code)
        
        # 환경 변수 설정
        env = os.environ.copy()
        if env_vars:
            env.update(env_vars)
        
        # 실행
        started_at = datetime.now()
        timed_out = False
        
        try:
            process = subprocess.Popen(
                [self.python_path, str(code_file)],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                cwd=str(self.working_dir),
                env=env
            )
            
            try:
                stdout, stderr = process.communicate(timeout=self.timeout)
            except subprocess.TimeoutExpired:
                process.kill()
                stdout, stderr = process.communicate()
                timed_out = True
            
            completed_at = datetime.now()
            execution_time = (completed_at - started_at).total_seconds()
            
            stdout_str = stdout.decode('utf-8', errors='replace')
            stderr_str = stderr.decode('utf-8', errors='replace')
            
            # 메트릭 추출
            metrics = {}
            if capture_metrics:
                metrics = self._extract_metrics(stdout_str)
            
            # 출력 파일 확인
            output_files = self._find_output_files()
            
            return ExecutionResult(
                success=process.returncode == 0 and not timed_out,
                return_code=process.returncode,
                stdout=stdout_str,
                stderr=stderr_str,
                execution_time=execution_time,
                metrics=metrics,
                output_files=output_files,
                started_at=started_at,
                completed_at=completed_at,
                timed_out=timed_out
            )
        
        except Exception as e:
            completed_at = datetime.now()
            return ExecutionResult(
                success=False,
                return_code=-1,
                stdout="",
                stderr=str(e),
                execution_time=(completed_at - started_at).total_seconds(),
                started_at=started_at,
                completed_at=completed_at,
                timed_out=False
            )
    
    def execute_with_debug(
        self,
        code: str,
        debugger,
        max_attempts: int = 5
    ) -> Tuple[ExecutionResult, str]:
        """
        디버깅과 함께 실행
        
        Args:
            code: 실행할 코드
            debugger: AutoDebugger 인스턴스
            max_attempts: 최대 시도 횟수
        
        Returns:
            (ExecutionResult, 최종 코드)
        """
        current_code = code
        
        for attempt in range(max_attempts):
            result = self.execute(current_code)
            
            if result.success:
                return result, current_code
            
            # 디버깅 시도
            debug_result = debugger.debug(current_code, result.stderr)
            
            if debug_result.was_fixed and debug_result.confidence > 0.3:
                current_code = debug_result.fixed_code
            else:
                break
        
        return result, current_code
    
    def install_packages(self, packages: List[str]) -> bool:
        """
        패키지 설치
        
        Args:
            packages: 설치할 패키지 리스트
        
        Returns:
            성공 여부
        """
        if not packages:
            return True
        
        try:
            subprocess.run(
                [self.python_path, "-m", "pip", "install"] + packages,
                capture_output=True,
                timeout=120,
                cwd=str(self.working_dir)
            )
            return True
        except Exception:
            return False
    
    def _add_metric_capture(self, code: str) -> str:
        """메트릭 캡처 코드 추가"""
        capture_code = '''
# Metric capture helper
import json
import sys

_metrics = {}

def report_metric(name, value):
    """Report a metric value that will be captured."""
    _metrics[name] = float(value)
    print(f"METRIC:{name}={value}")

def save_metrics():
    """Save all metrics to JSON file."""
    with open("metrics.json", "w") as f:
        json.dump(_metrics, f)
    print(f"METRICS_JSON:{json.dumps(_metrics)}")

# Register atexit to save metrics
import atexit
atexit.register(save_metrics)

'''
        return capture_code + code
    
    def _extract_metrics(self, stdout: str) -> Dict[str, float]:
        """stdout에서 메트릭 추출"""
        metrics = {}
        
        for line in stdout.split('\n'):
            # METRIC:name=value 형식
            if line.startswith('METRIC:'):
                try:
                    part = line[7:]  # Remove 'METRIC:'
                    name, value = part.split('=', 1)
                    metrics[name.strip()] = float(value.strip())
                except (ValueError, IndexError):
                    pass
            
            # METRICS_JSON:{...} 형식
            elif line.startswith('METRICS_JSON:'):
                try:
                    json_str = line[13:]  # Remove 'METRICS_JSON:'
                    metrics.update(json.loads(json_str))
                except (json.JSONDecodeError, ValueError):
                    pass
        
        # metrics.json 파일에서도 읽기
        metrics_file = self.working_dir / "metrics.json"
        if metrics_file.exists():
            try:
                with open(metrics_file) as f:
                    metrics.update(json.load(f))
            except (json.JSONDecodeError, ValueError):
                pass
        
        return metrics
    
    def _find_output_files(self) -> List[str]:
        """출력 파일 찾기"""
        output_patterns = ['*.png', '*.jpg', '*.pdf', '*.csv', '*.json', '*.txt']
        output_files = []
        
        for pattern in output_patterns:
            for file in self.working_dir.glob(pattern):
                if file.name not in ['experiment.py', 'plot.py', 'requirements.txt']:
                    output_files.append(str(file))
        
        return output_files
    
    def cleanup(self):
        """임시 파일 정리"""
        import shutil
        if self.working_dir.exists():
            shutil.rmtree(self.working_dir, ignore_errors=True)
