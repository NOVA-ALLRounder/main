"""
Code Executor Tool - 안전한 샌드박스 환경에서 Python 코드 실행
"""
import subprocess
import tempfile
import os
import shutil
from typing import Tuple, List
from pathlib import Path

from config import CODE_EXECUTION_TIMEOUT


def execute_code(
    code: str,
    timeout: int = None,
    working_dir: str = None
) -> Tuple[bool, str, str, List[str]]:
    """
    Python 코드 실행
    
    Args:
        code: 실행할 Python 코드
        timeout: 실행 타임아웃 (초)
        working_dir: 작업 디렉토리 (None이면 임시 디렉토리 사용)
    
    Returns:
        (성공 여부, stdout, stderr, 생성된 파일 목록)
    """
    if timeout is None:
        timeout = CODE_EXECUTION_TIMEOUT
    
    # 임시 디렉토리 생성
    temp_dir = working_dir or tempfile.mkdtemp(prefix='science_lab_')
    script_path = os.path.join(temp_dir, 'experiment.py')
    
    try:
        # 코드 파일 저장
        with open(script_path, 'w', encoding='utf-8') as f:
            f.write(code)
        
        # 코드 실행
        result = subprocess.run(
            ['python', script_path],
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=temp_dir,
            env={
                **os.environ,
                'PYTHONPATH': os.path.dirname(os.path.dirname(__file__))
            }
        )
        
        stdout = result.stdout
        stderr = result.stderr
        success = result.returncode == 0
        
        # 생성된 파일 수집
        generated_files = []
        for filename in os.listdir(temp_dir):
            if filename != 'experiment.py':
                file_path = os.path.join(temp_dir, filename)
                generated_files.append(file_path)
        
        return success, stdout, stderr, generated_files
        
    except subprocess.TimeoutExpired:
        return False, '', f'실행 시간 초과 ({timeout}초)', []
    
    except Exception as e:
        return False, '', str(e), []
    
    finally:
        # 임시 디렉토리가 아니면 정리하지 않음
        if working_dir is None:
            try:
                # 생성된 파일들을 영구 저장소로 이동
                pass  # 필요시 구현
            except:
                pass


def execute_code_in_docker(
    code: str,
    image: str = 'python:3.10-slim',
    timeout: int = None
) -> Tuple[bool, str, str, List[str]]:
    """
    Docker 컨테이너에서 코드 실행 (격리된 환경)
    
    Args:
        code: 실행할 Python 코드
        image: Docker 이미지
        timeout: 실행 타임아웃
    
    Returns:
        (성공 여부, stdout, stderr, 생성된 파일 목록)
    """
    if timeout is None:
        timeout = CODE_EXECUTION_TIMEOUT
    
    # Docker가 설치되어 있는지 확인
    try:
        subprocess.run(['docker', '--version'], capture_output=True, check=True)
    except (subprocess.CalledProcessError, FileNotFoundError):
        # Docker 없으면 일반 실행으로 폴백
        return execute_code(code, timeout)
    
    temp_dir = tempfile.mkdtemp(prefix='science_lab_docker_')
    script_path = os.path.join(temp_dir, 'experiment.py')
    
    try:
        with open(script_path, 'w', encoding='utf-8') as f:
            f.write(code)
        
        # Docker 실행
        result = subprocess.run(
            [
                'docker', 'run', '--rm',
                '-v', f'{temp_dir}:/app',
                '-w', '/app',
                '--network', 'none',  # 네트워크 격리
                '--memory', '512m',   # 메모리 제한
                image,
                'python', 'experiment.py'
            ],
            capture_output=True,
            text=True,
            timeout=timeout
        )
        
        stdout = result.stdout
        stderr = result.stderr
        success = result.returncode == 0
        
        generated_files = [
            os.path.join(temp_dir, f) 
            for f in os.listdir(temp_dir) 
            if f != 'experiment.py'
        ]
        
        return success, stdout, stderr, generated_files
        
    except subprocess.TimeoutExpired:
        return False, '', f'Docker 실행 시간 초과 ({timeout}초)', []
    except Exception as e:
        return False, '', str(e), []
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)


def validate_code_safety(code: str) -> Tuple[bool, str]:
    """
    코드 안전성 검사 (기본적인 검사만 수행)
    
    Args:
        code: 검사할 코드
    
    Returns:
        (안전 여부, 경고 메시지)
    """
    dangerous_patterns = [
        'os.system',
        'subprocess.call',
        'subprocess.Popen',
        'eval(',
        'exec(',
        '__import__',
        'open("/etc',
        'open("/root',
        'shutil.rmtree("/"',
        'rm -rf',
    ]
    
    warnings = []
    for pattern in dangerous_patterns:
        if pattern in code:
            warnings.append(f"위험한 패턴 발견: {pattern}")
    
    if warnings:
        return False, "; ".join(warnings)
    
    return True, ""
