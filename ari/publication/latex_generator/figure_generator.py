"""
Figure Generator

논문용 TikZ 다이어그램 및 플레이스홀더 이미지 생성
"""

from typing import Dict, List, Any, Optional


class FigureGenerator:
    """
    논문용 그림/다이어그램 생성
    - 시스템 아키텍처 다이어그램
    - 결과 비교 표
    - 플로우차트
    """
    
    def generate_system_architecture(
        self,
        title: str,
        components: List[str] = None,
        domain: str = "Machine Learning"
    ) -> str:
        """
        시스템 아키텍처 다이어그램 (TikZ)
        """
        components = components or ["Input", "Processing", "Model", "Output"]
        
        # 도메인별 컴포넌트 예시
        DOMAIN_COMPONENTS = {
            "Machine Learning": ["Data Preprocessing", "Feature Extraction", "Neural Network", "Prediction"],
            "통신": ["Transmitter", "Channel", "IRS Controller", "Receiver"],
            "Computer Vision": ["Image Input", "CNN Backbone", "Feature Maps", "Classification"],
            "NLP": ["Text Input", "Tokenizer", "Transformer", "Output"],
        }
        
        components = DOMAIN_COMPONENTS.get(domain, components)
        
        # TikZ 코드 생성
        tikz = r"""
\begin{figure}[h]
\centering
\begin{tikzpicture}[
    node distance=2.5cm,
    box/.style={rectangle, draw, minimum width=2.5cm, minimum height=1cm, align=center},
    arrow/.style={->, >=stealth, thick}
]
"""
        
        # 노드 배치
        for i, comp in enumerate(components):
            x_pos = i * 3.5
            tikz += f"\\node[box] (n{i}) at ({x_pos}, 0) {{{comp}}};\n"
        
        # 화살표 연결
        for i in range(len(components) - 1):
            tikz += f"\\draw[arrow] (n{i}) -- (n{i+1});\n"
        
        tikz += r"""
\end{tikzpicture}
\caption{System architecture of the proposed method.}
\label{fig:architecture}
\end{figure}
"""
        return tikz
    
    def generate_comparison_figure_placeholder(self, title: str = "Results Comparison") -> str:
        """
        결과 비교 그림 플레이스홀더
        """
        return r"""
\begin{figure}[h]
\centering
\fbox{\parbox{0.8\textwidth}{
\centering
\vspace{2cm}
\textbf{[Performance Comparison Chart]}\\
\vspace{0.5cm}
A bar chart comparing the proposed method against baselines would be placed here.
\vspace{2cm}
}}
\caption{Performance comparison of proposed method vs. baselines.}
\label{fig:comparison}
\end{figure}
"""
    
    def generate_methodology_flowchart(self, steps: List[str] = None) -> str:
        """
        방법론 플로우차트
        """
        steps = steps or ["Initialize", "Train Model", "Evaluate", "Optimize"]
        
        tikz = r"""
\begin{figure}[h]
\centering
\begin{tikzpicture}[
    node distance=1.5cm,
    box/.style={rectangle, draw, rounded corners, minimum width=3cm, minimum height=0.8cm, align=center},
    arrow/.style={->, >=stealth, thick}
]
"""
        
        for i, step in enumerate(steps):
            y_pos = -i * 1.5
            tikz += f"\\node[box] (s{i}) at (0, {y_pos}) {{{step}}};\n"
        
        for i in range(len(steps) - 1):
            tikz += f"\\draw[arrow] (s{i}) -- (s{i+1});\n"
        
        tikz += r"""
\end{tikzpicture}
\caption{Flowchart of the proposed methodology.}
\label{fig:flowchart}
\end{figure}
"""
        return tikz
    
    def get_tikz_preamble(self) -> str:
        """
        TikZ 사용을 위한 LaTeX 프리앰블
        """
        return r"""
\usepackage{tikz}
\usetikzlibrary{shapes,arrows,positioning}
"""
