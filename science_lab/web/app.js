/**
 * Science Lab - Frontend Application
 * Handles user interactions and API communication
 */

// ===== Configuration =====
const API_BASE = '';  // Same origin

// ===== State =====
let currentSession = null;
let selectedMethodIndex = -1;

// ===== DOM Elements =====
const elements = {
    // Inputs
    domainInput: document.getElementById('domain-input'),
    hypothesisInput: document.getElementById('hypothesis-input'),
    submitBtn: document.getElementById('submit-btn'),

    // Sections
    inputSection: document.getElementById('input-section'),
    loadingSection: document.getElementById('loading-section'),
    classificationSection: document.getElementById('classification-section'),
    literatureSection: document.getElementById('literature-section'),
    methodsSection: document.getElementById('methods-section'),
    experimentSection: document.getElementById('experiment-section'),
    reportSection: document.getElementById('report-section'),
    feasibilitySection: document.getElementById('feasibility-section'),

    // Dynamic content
    loadingMessage: document.getElementById('loading-message'),
    intentType: document.getElementById('intent-type'),
    confidenceValue: document.getElementById('confidence-value'),
    noveltyCard: document.getElementById('novelty-card'),
    noveltyValue: document.getElementById('novelty-value'),
    feasibilityCard: document.getElementById('feasibility-card'),
    feasibilityValue: document.getElementById('feasibility-value'),
    literatureList: document.getElementById('literature-list'),
    methodsList: document.getElementById('methods-list'),
    progressFill: document.getElementById('progress-fill'),
    experimentStatus: document.getElementById('experiment-status'),
    codePreview: document.getElementById('code-preview'),
    codeContent: document.getElementById('code-content'),
    reportContent: document.getElementById('report-content'),
    feasibilityContent: document.getElementById('feasibility-content'),

    // Actions
    downloadBtn: document.getElementById('download-btn'),
    newResearchBtn: document.getElementById('new-research-btn'),

    // History
    historyBtn: document.getElementById('history-btn'),
    closeHistoryBtn: document.getElementById('close-history-btn'),
    historySection: document.getElementById('history-section'),
    historyList: document.getElementById('history-list')
};

// ===== Event Listeners =====
document.addEventListener('DOMContentLoaded', () => {
    elements.submitBtn.addEventListener('click', startResearch);
    elements.downloadBtn?.addEventListener('click', downloadReport);
    elements.newResearchBtn?.addEventListener('click', resetApp);

    // History events
    elements.historyBtn?.addEventListener('click', showHistory);
    elements.closeHistoryBtn?.addEventListener('click', () => {
        elements.historySection.classList.add('hidden');
        if (!currentSession) showSection('input');
        else showSection('classification', 'report'); // Return to last view if possible
    });

    // Enter key support
    elements.hypothesisInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && e.ctrlKey) {
            startResearch();
        }
    });
});

// ===== API Functions =====

async function startResearch() {
    const userInput = elements.hypothesisInput.value.trim();
    const domain = elements.domainInput.value.trim();

    if (!userInput) {
        alert('가설 또는 질문을 입력해 주세요.');
        elements.hypothesisInput.focus();
        return;
    }

    try {
        // Show loading
        showSection('loading');
        updateLoadingMessage('의도 분류 및 문헌 검색 중...');
        elements.submitBtn.disabled = true;

        // API call
        const response = await fetch(`${API_BASE}/api/research`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                user_input: userInput,
                domain: domain
            })
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.detail || 'API 오류');
        }

        const data = await response.json();
        currentSession = data;

        // Display results
        displayClassificationResult(data);
        displayLiterature(data.literature_context);

        if (data.intent === 'hypothesis' && data.proposed_methods?.length > 0) {
            // Hypothesis with methods
            displayMethods(data.proposed_methods);
            showSection('classification', 'literature', 'methods');
        } else if (data.intent === 'question') {
            // Question - show feasibility
            displayFeasibility(data);
            showSection('classification', 'literature', 'feasibility');
        } else if (data.novelty_score < 0.85) {
            // Non-novel hypothesis
            displayExistingResearch(data);
            showSection('classification', 'literature');
        }

    } catch (error) {
        console.error('Research error:', error);
        alert(`오류 발생: ${error.message}`);
        showSection('input');
    } finally {
        elements.submitBtn.disabled = false;
    }
}

async function selectMethod(index) {
    if (!currentSession) return;

    selectedMethodIndex = index;

    // Update UI
    document.querySelectorAll('.method-card').forEach((card, i) => {
        card.classList.toggle('selected', i === index);
    });

    // Confirm selection
    const method = currentSession.proposed_methods[index];
    if (!confirm(`"${method.title}" 방법론으로 실험을 진행하시겠습니까?`)) {
        return;
    }

    try {
        // Show experiment section
        showSection('classification', 'experiment');
        simulateProgress();

        // API call
        const response = await fetch(`${API_BASE}/api/select-method`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                session_id: currentSession.session_id,
                method_index: index
            })
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.detail || 'API 오류');
        }

        const data = await response.json();
        currentSession = { ...currentSession, ...data };

        // Complete progress
        elements.progressFill.style.width = '100%';
        elements.experimentStatus.textContent = '실험 완료!';

        // Show report
        setTimeout(() => {
            displayReport(data.final_report);
            showSection('classification', 'report');
        }, 500);

    } catch (error) {
        console.error('Experiment error:', error);
        elements.experimentStatus.textContent = `오류: ${error.message}`;
    }
}

async function downloadReport() {
    if (!currentSession?.session_id) return;

    try {
        window.open(`${API_BASE}/api/report/${currentSession.session_id}`, '_blank');
    } catch (error) {
        alert('보고서 다운로드 실패');
    }
}

// ===== Display Functions =====

async function showHistory() {
    try {
        // Hide other sections
        showSection('loading');
        updateLoadingMessage('기록 불러오는 중...');

        const response = await fetch(`${API_BASE}/api/sessions`);
        if (!response.ok) throw new Error('Failed to fetch sessions');

        const sessions = await response.json();
        renderHistoryList(sessions);

        showSection('history');

    } catch (error) {
        console.error('History error:', error);
        alert('기록을 불러오는데 실패했습니다.');
        showSection('input');
    }
}

function renderHistoryList(sessions) {
    if (!sessions || sessions.length === 0) {
        elements.historyList.innerHTML = '<p class="no-data">저장된 연구 기록이 없습니다.</p>';
        return;
    }

    elements.historyList.innerHTML = sessions.map(session => {
        const date = new Date(session.created_at).toLocaleString();
        const statusClass = session.status === 'completed' ? 'status-completed' : 'status-processing';
        const statusText = session.status === 'completed' ? '완료' : '진행 중';

        return `
        <div class="history-item glass-card" onclick="loadSession('${session.id}')">
            <div class="history-header">
                <span class="history-date">${date}</span>
                <span class="history-status ${statusClass}">${statusText}</span>
            </div>
            <div class="history-query">${escapeHtml(session.user_query)}</div>
            <div class="history-meta">
                ${session.intent ? `<span class="intent-tag ${session.intent}">${session.intent === 'hypothesis' ? '가설' : '질문'}</span>` : ''}
                ${session.domain ? `<span>${escapeHtml(session.domain)}</span>` : ''}
            </div>
        </div>
        `;
    }).join('');
}

async function loadSession(sessionId) {
    try {
        showSection('loading');
        updateLoadingMessage('세션 데이터 로드 중...');

        const response = await fetch(`${API_BASE}/api/session/${sessionId}`);
        if (!response.ok) throw new Error('Session load failed');

        const data = await response.json();
        currentSession = data;

        // Restore view based on state
        displayClassificationResult(data);
        if (data.literature_context) displayLiterature(data.literature_context);

        if (data.final_report) {
            displayReport(data.final_report);
            showSection('classification', 'literature', 'report');
        } else if (data.proposed_methods) {
            displayMethods(data.proposed_methods);
            showSection('classification', 'literature', 'methods');
        } else if (data.intent === 'question' && data.feasibility_grade) {
            displayFeasibility(data);
            showSection('classification', 'literature', 'feasibility');
        } else {
            showSection('classification', 'literature');
        }

    } catch (error) {
        console.error('Load session error:', error);
        alert('세션을 불러올 수 없습니다.');
        showHistory();
    }
}

// Make loadSession globally available for onclick
window.loadSession = loadSession;

function displayClassificationResult(data) {
    // Intent
    elements.intentType.textContent = data.intent === 'hypothesis' ? '가설' : '질문';
    elements.intentType.className = `result-value intent-badge ${data.intent}`;

    // Confidence
    const confidence = Math.round((data.intent_confidence || 0) * 100);
    elements.confidenceValue.textContent = `${confidence}%`;

    // Novelty (hypothesis only)
    if (data.intent === 'hypothesis' && data.novelty_score !== undefined) {
        elements.noveltyCard.classList.remove('hidden');
        const novelty = Math.round(data.novelty_score * 100);
        elements.noveltyValue.textContent = `${novelty}%`;
        elements.noveltyValue.style.color = novelty >= 85 ? '#10b981' : '#f59e0b';
    } else {
        elements.noveltyCard.classList.add('hidden');
    }

    // Feasibility (question only)
    if (data.intent === 'question' && data.feasibility_grade) {
        elements.feasibilityCard.classList.remove('hidden');
        const gradeMap = {
            'high': '높음',
            'medium': '중간',
            'low': '낮음',
            'uncertain': '불확실'
        };
        elements.feasibilityValue.textContent = gradeMap[data.feasibility_grade] || data.feasibility_grade;
    } else {
        elements.feasibilityCard.classList.add('hidden');
    }
}

function displayLiterature(literature) {
    if (!literature || literature.length === 0) {
        elements.literatureList.innerHTML = '<p class="no-data">관련 문헌을 찾지 못했습니다.</p>';
        return;
    }

    elements.literatureList.innerHTML = literature.map(item => `
        <div class="literature-item" ${item.url ? `onclick="window.open('${item.url}', '_blank')"` : ''} style="${item.url ? 'cursor: pointer;' : ''}">
            <div class="literature-title">
                ${escapeHtml(item.title || 'Unknown Title')}
                ${item.url ? '<span style="font-size: 0.8em; margin-left: 5px;">🔗</span>' : ''}
            </div>
            <div class="literature-meta">
                ${item.authors?.slice(0, 2).join(', ') || 'Unknown'} 
                ${item.year ? `(${item.year})` : ''} 
                • ${item.source || 'unknown'}
            </div>
        </div>
    `).join('');
}

function displayMethods(methods) {
    const icons = {
        'analytical': '📊',
        'simulation': '🔬',
        'data_driven': '🤖'
    };

    elements.methodsList.innerHTML = methods.map((method, index) => `
        <div class="method-card" onclick="selectMethod(${index})">
            <div class="method-header">
                <div class="method-icon ${method.type}">${icons[method.type] || '🧪'}</div>
                <div>
                    <div class="method-title">${escapeHtml(method.title)}</div>
                    <div class="method-type">${method.type}</div>
                </div>
            </div>
            <p class="method-description">${escapeHtml(method.description)}</p>
            <div class="method-meta">
                <span>⏱️ ${method.estimated_time || 'N/A'}</span>
                <span>📦 ${(method.required_libraries || []).slice(0, 3).join(', ')}</span>
            </div>
        </div>
    `).join('');
}

function displayFeasibility(data) {
    const gradeColors = {
        'high': '#10b981',
        'medium': '#f59e0b',
        'low': '#ef4444',
        'uncertain': '#6b7280'
    };

    let content = `<h3 style="color: ${gradeColors[data.feasibility_grade] || '#fff'}">
        실현 가능성: ${data.feasibility_grade?.toUpperCase() || 'N/A'}
    </h3>`;

    if (data.final_report) {
        content += `<div class="feasibility-analysis">${renderMarkdown(data.final_report)}</div>`;
    }

    elements.feasibilityContent.innerHTML = content;
}

function displayExistingResearch(data) {
    const message = `
        <div class="existing-research-notice">
            <h3>⚠️ 유사한 기존 연구 발견</h3>
            <p>입력하신 가설과 유사한 연구가 이미 존재합니다. (독창성: ${Math.round(data.novelty_score * 100)}%)</p>
            ${data.novelty_analysis ? `<p>${data.novelty_analysis}</p>` : ''}
            <p style="margin-top: 1rem; font-size: 0.9em; color: rgba(255,255,255,0.7);">
                시스템 설계상 중복 연구 방지를 위해 연구가 중단되었습니다. 하지만 원하신다면 강제로 실험을 진행할 수 있습니다.
            </p>
            <button onclick="forceContinue('${data.session_id}')" class="btn-primary" style="margin-top: 1rem; background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);">
                🧪 그래도 실험 진행하기
            </button>
        </div>
    `;

    // Add to classification section
    elements.classificationSection.querySelector('.result-cards').insertAdjacentHTML('afterend', message);
}

async function forceContinue(sessionId) {
    if (!confirm('경고: 독창성이 낮은 가설에 대한 실험은 중복된 결과를 낳을 수 있습니다. 그래도 진행하시겠습니까?')) {
        return;
    }

    try {
        showSection('loading');
        updateLoadingMessage('실험 방법론 설계 중 (강제 진행)...');

        const response = await fetch(`${API_BASE}/api/research/continue`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                user_input: sessionId, // session_id 전달
                domain: ''
            })
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.detail || 'API 오류');
        }

        const data = await response.json();
        currentSession = { ...currentSession, ...data };

        // 방법론 표시
        displayMethods(data.proposed_methods);
        showSection('classification', 'literature', 'methods');

        // 기존 경고 메시지 제거 또는 업데이트 (선택사항)

    } catch (error) {
        console.error('Force continue error:', error);
        alert(`오류 발생: ${error.message}`);
        showSection('classification', 'literature');
    }
}

// Global scope for onclick
window.forceContinue = forceContinue;

function displayReport(report) {
    if (!report) {
        elements.reportContent.innerHTML = '<p>보고서 생성에 실패했습니다.</p>';
        return;
    }

    elements.reportContent.innerHTML = renderMarkdown(report);
}

// ===== UI Helpers =====

function showSection(...sectionNames) {
    // Hide all sections first
    const allSections = [
        'input', 'loading', 'classification', 'literature',
        'methods', 'experiment', 'report', 'feasibility', 'history'
    ];

    allSections.forEach(name => {
        const section = elements[`${name}Section`];
        if (section) {
            section.classList.add('hidden');
        }
    });

    // Show requested sections
    sectionNames.forEach(name => {
        const section = elements[`${name}Section`];
        if (section) {
            section.classList.remove('hidden');
        }
    });
}

function updateLoadingMessage(message) {
    elements.loadingMessage.textContent = message;
}

function simulateProgress() {
    let progress = 0;
    const interval = setInterval(() => {
        progress += Math.random() * 15;
        if (progress > 90) {
            clearInterval(interval);
            progress = 90;
        }
        elements.progressFill.style.width = `${progress}%`;

        // Update status
        if (progress < 20) {
            elements.experimentStatus.textContent = '코드 생성 중...';
        } else if (progress < 50) {
            elements.experimentStatus.textContent = '실험 실행 중...';
        } else if (progress < 80) {
            elements.experimentStatus.textContent = '결과 분석 중...';
        } else {
            elements.experimentStatus.textContent = '보고서 작성 중...';
        }
    }, 500);
}

function resetApp() {
    currentSession = null;
    selectedMethodIndex = -1;
    elements.hypothesisInput.value = '';
    elements.domainInput.value = '';
    showSection('input');

    // Reset progress
    elements.progressFill.style.width = '0%';
    elements.experimentStatus.textContent = '';
}

// ===== Utility Functions =====

function escapeHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

function renderMarkdown(text) {
    if (!text) return '';

    // Simple markdown rendering
    return text
        // Headers
        .replace(/^### (.*$)/gim, '<h3>$1</h3>')
        .replace(/^## (.*$)/gim, '<h2>$1</h2>')
        .replace(/^# (.*$)/gim, '<h1>$1</h1>')
        // Bold and Italic
        .replace(/\*\*\*(.*?)\*\*\*/g, '<strong><em>$1</em></strong>')
        .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
        .replace(/\*(.*?)\*/g, '<em>$1</em>')
        // Code blocks
        .replace(/```([^`]+)```/g, '<pre><code>$1</code></pre>')
        .replace(/`([^`]+)`/g, '<code>$1</code>')
        // Blockquote
        .replace(/^> (.*$)/gim, '<blockquote>$1</blockquote>')
        // Lists
        .replace(/^- (.*$)/gim, '<li>$1</li>')
        // Line breaks
        .replace(/\n\n/g, '</p><p>')
        .replace(/\n/g, '<br>');
}

// Make selectMethod globally available
window.selectMethod = selectMethod;
