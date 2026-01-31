/**
 * Science Lab - Frontend Application
 * Step-by-step Screen Flow
 */

// ===== Configuration =====
const API_BASE = '';

// ===== State =====
let currentSession = null;
let selectedMethodIndex = -1;
let uploadedFiles = [];

// Views: 'input' -> 'analysis' -> 'methods' -> 'experiment' -> 'report'
let currentView = 'input';

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
    originContent: document.getElementById('origin-content'),
    progressFill: document.getElementById('progress-fill'),
    experimentStatus: document.getElementById('experiment-status'),
    codePreview: document.getElementById('code-preview'),
    codeContent: document.getElementById('code-content'),
    reportContent: document.getElementById('report-content'),
    feasibilityContent: document.getElementById('feasibility-content'),

    // Enhancement
    enhancementPanel: document.getElementById('enhancement-panel'),
    enhancementInput: document.getElementById('enhancement-input'),
    enhanceBtn: document.getElementById('enhance-btn'),

    // File Upload
    fileUploadArea: document.getElementById('file-upload-area'),
    fileInput: document.getElementById('file-input'),
    uploadedFilesContainer: document.getElementById('uploaded-files'),

    // Actions
    downloadBtn: document.getElementById('download-btn'),
    newResearchBtn: document.getElementById('new-research-btn'),
    newResearchSidebarBtn: document.getElementById('new-research-sidebar-btn'),

    // Sidebar
    sidebarHistoryList: document.getElementById('sidebar-history-list')
};

// ===== Event Listeners =====
document.addEventListener('DOMContentLoaded', () => {
    elements.submitBtn?.addEventListener('click', startResearch);
    elements.downloadBtn?.addEventListener('click', downloadReport);
    elements.newResearchBtn?.addEventListener('click', resetApp);
    elements.newResearchSidebarBtn?.addEventListener('click', resetApp);
    elements.enhanceBtn?.addEventListener('click', enhanceHypothesis);

    // Step navigation
    elements.goToMethodsBtn?.addEventListener('click', () => switchView('methods'));

    setupFileUpload();

    elements.hypothesisInput?.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && e.ctrlKey) startResearch();
    });

    loadSidebarHistory();
});

// ===== File Upload =====
function setupFileUpload() {
    const area = elements.fileUploadArea;
    const input = elements.fileInput;

    if (!area || !input) return;

    area.addEventListener('click', () => input.click());
    input.addEventListener('change', (e) => handleFiles(e.target.files));

    area.addEventListener('dragover', (e) => {
        e.preventDefault();
        area.style.borderColor = 'var(--color-primary)';
    });

    area.addEventListener('dragleave', () => {
        area.style.borderColor = '';
    });

    area.addEventListener('drop', (e) => {
        e.preventDefault();
        area.style.borderColor = '';
        handleFiles(e.dataTransfer.files);
    });
}

async function handleFiles(files) {
    for (const file of files) {
        if (file.size > 10 * 1024 * 1024) {
            alert(`${file.name}: 파일 크기가 10MB를 초과합니다.`);
            continue;
        }

        try {
            const formData = new FormData();
            formData.append('file', file);

            const response = await fetch(`${API_BASE}/api/upload`, {
                method: 'POST',
                body: formData
            });

            if (response.ok) {
                const result = await response.json();
                uploadedFiles.push(result);
                renderUploadedFiles();
            }
        } catch (error) {
            console.error('Upload error:', error);
        }
    }
}

function renderUploadedFiles() {
    if (!elements.uploadedFilesContainer) return;

    elements.uploadedFilesContainer.innerHTML = uploadedFiles.map((file, index) => `
        <div class="uploaded-file">
            <span>${file.filename}</span>
            <button onclick="removeFile(${index})">×</button>
        </div>
    `).join('');
}

function removeFile(index) {
    uploadedFiles.splice(index, 1);
    renderUploadedFiles();
}
window.removeFile = removeFile;

// ===== Sidebar with Domain Folders =====
async function loadSidebarHistory() {
    try {
        const response = await fetch(`${API_BASE}/api/sessions`);
        if (!response.ok) return;

        const sessions = await response.json();
        renderSidebarWithFolders(sessions);
    } catch (error) {
        console.error('Failed to load sidebar history:', error);
    }
}

function renderSidebarWithFolders(sessions) {
    if (!elements.sidebarHistoryList) return;

    if (!sessions || sessions.length === 0) {
        elements.sidebarHistoryList.innerHTML = '<p style="color: var(--color-text-muted); text-align: center; padding: 20px;">연구 기록이 없습니다.</p>';
        return;
    }

    // Group by domain
    const grouped = {};
    sessions.forEach(session => {
        const domain = session.domain || '미분류';
        if (!grouped[domain]) grouped[domain] = [];
        grouped[domain].push(session);
    });

    // Render folders
    let html = '';
    for (const [domain, items] of Object.entries(grouped)) {
        html += `
            <div class="sidebar-folder" data-domain="${escapeHtml(domain)}">
                <div class="sidebar-folder-header" onclick="toggleFolder(this)">
                    <span class="sidebar-folder-icon">▼</span>
                    <span>${escapeHtml(domain)}</span>
                    <span style="margin-left: auto; color: var(--color-text-muted); font-size: 0.75rem;">${items.length}</span>
                </div>
                <div class="sidebar-folder-items">
                    ${items.map(session => renderSidebarItem(session)).join('')}
                </div>
            </div>
        `;
    }

    elements.sidebarHistoryList.innerHTML = html;
}

function renderSidebarItem(session) {
    const date = new Date(session.created_at).toLocaleDateString('ko-KR', {
        month: 'short',
        day: 'numeric'
    });
    const statusClass = session.status === 'completed' ? 'completed' : 'processing';
    const statusText = session.status === 'completed' ? '완료' : '진행중';
    const isActive = currentSession?.session_id === session.id;

    return `
        <div class="sidebar-item ${isActive ? 'active' : ''}" onclick="loadSession('${session.id}')">
            <button class="sidebar-item-delete" onclick="event.stopPropagation(); deleteSession('${session.id}')" title="삭제">×</button>
            <div class="sidebar-item-header">
                <span class="sidebar-item-date">${date}</span>
                <span class="sidebar-item-status ${statusClass}">${statusText}</span>
            </div>
            <div class="sidebar-item-query">${escapeHtml(session.user_query || '')}</div>
        </div>
    `;
}

function toggleFolder(headerElement) {
    const folder = headerElement.parentElement;
    folder.classList.toggle('collapsed');
}
window.toggleFolder = toggleFolder;

async function deleteSession(sessionId) {
    if (!confirm('이 연구 기록을 삭제하시겠습니까?')) return;

    try {
        const response = await fetch(`${API_BASE}/api/session/${sessionId}`, {
            method: 'DELETE'
        });

        if (response.ok) {
            if (currentSession?.session_id === sessionId) resetApp();
            loadSidebarHistory();
        } else {
            alert('삭제에 실패했습니다.');
        }
    } catch (error) {
        console.error('Delete error:', error);
    }
}
window.deleteSession = deleteSession;

// ===== View/Screen Management =====
// Views: 'input' -> 'loading' -> 'analysis' -> 'methods' -> 'experiment' -> 'report'
function switchView(viewName) {
    currentView = viewName;

    // Hide ALL sections
    const allSections = [
        elements.inputSection,
        elements.loadingSection,
        elements.classificationSection,
        elements.literatureSection,
        elements.methodsSection,
        elements.experimentSection,
        elements.reportSection,
        elements.feasibilitySection
    ];

    allSections.forEach(section => {
        if (section) section.classList.add('hidden');
    });

    // Show ONLY the relevant section(s) for this view
    switch (viewName) {
        case 'input':
            elements.inputSection?.classList.remove('hidden');
            break;

        case 'loading':
            elements.loadingSection?.classList.remove('hidden');
            break;

        case 'analysis':
            // Screen 2: Classification + Literature ONLY (no methods)
            elements.classificationSection?.classList.remove('hidden');
            elements.literatureSection?.classList.remove('hidden');
            break;

        case 'methods':
            // Screen 3: Methods selection ONLY
            elements.methodsSection?.classList.remove('hidden');
            break;

        case 'experiment':
            // Screen 4: Experiment progress ONLY
            elements.experimentSection?.classList.remove('hidden');
            break;

        case 'report':
            // Screen 5: Report ONLY
            elements.reportSection?.classList.remove('hidden');
            break;

        case 'feasibility':
            elements.classificationSection?.classList.remove('hidden');
            elements.literatureSection?.classList.remove('hidden');
            elements.feasibilitySection?.classList.remove('hidden');
            break;
    }
}

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
        switchView('loading');
        updateLoadingMessage('의도 분류 및 문헌 검색 중...');
        elements.submitBtn.disabled = true;

        const response = await fetch(`${API_BASE}/api/research`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                user_input: userInput,
                domain: domain,
                attachments: uploadedFiles.map(f => f.path)
            })
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.detail || 'API 오류');
        }

        const data = await response.json();
        currentSession = data;

        loadSidebarHistory();
        displayClassificationResult(data);
        displayLiterature(data.literature_context);

        if (data.intent === 'hypothesis' && data.proposed_methods?.length > 0) {
            displayMethodsMindMap(data.proposed_methods, userInput);
            // Go directly to methods screen
            switchView('methods');
        } else if (data.intent === 'question') {
            displayFeasibility(data);
            switchView('feasibility');
        } else if (data.novelty_score < 0.85) {
            displayExistingResearch(data);
            switchView('analysis');
        }

    } catch (error) {
        console.error('Research error:', error);
        alert(`오류 발생: ${error.message}`);
        switchView('input');
    } finally {
        elements.submitBtn.disabled = false;
    }
}

async function selectMethod(index) {
    if (!currentSession) return;

    selectedMethodIndex = index;

    document.querySelectorAll('.method-card').forEach((card, i) => {
        card.classList.toggle('selected', i === index);
    });

    const method = currentSession.proposed_methods[index];
    if (!confirm(`"${method.title}" 방법론으로 실험을 진행하시겠습니까?`)) {
        return;
    }

    try {
        // Switch to experiment screen (ONLY experiment)
        switchView('experiment');
        simulateProgress();

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

        elements.progressFill.style.width = '100%';
        elements.experimentStatus.textContent = '실험 완료!';

        // After completion, switch to report screen (ONLY report)
        setTimeout(() => {
            displayReport(data.final_report);
            switchView('report');
            loadSidebarHistory();
        }, 1000);

    } catch (error) {
        console.error('Experiment error:', error);
        elements.experimentStatus.textContent = `오류: ${error.message}`;
    }
}
window.selectMethod = selectMethod;

async function enhanceHypothesis() {
    const enhancement = elements.enhancementInput?.value.trim();
    if (!enhancement) {
        alert('발전시킬 방향을 입력해주세요.');
        return;
    }

    const originalHypothesis = elements.hypothesisInput.value.trim();
    elements.hypothesisInput.value = `${originalHypothesis}\n\n추가 조건: ${enhancement}`;
    elements.enhancementInput.value = '';

    await startResearch();
}

async function downloadReport() {
    if (!currentSession?.session_id) return;
    window.open(`${API_BASE}/api/report/${currentSession.session_id}`, '_blank');
}

async function loadSession(sessionId) {
    try {
        switchView('loading');
        updateLoadingMessage('세션 데이터 로드 중...');

        const response = await fetch(`${API_BASE}/api/session/${sessionId}`);
        if (!response.ok) throw new Error('Session load failed');

        const data = await response.json();
        currentSession = data;

        loadSidebarHistory();
        displayClassificationResult(data);
        if (data.literature_context) displayLiterature(data.literature_context);

        if (data.final_report) {
            displayReport(data.final_report);
            switchView('report');
        } else if (data.proposed_methods) {
            displayMethodsMindMap(data.proposed_methods, data.user_input || '');
            switchView('methods');
        } else if (data.intent === 'question' && data.feasibility_grade) {
            displayFeasibility(data);
            switchView('feasibility');
        } else {
            switchView('analysis');
        }

    } catch (error) {
        console.error('Load session error:', error);
        alert('세션을 불러올 수 없습니다.');
        switchView('input');
    }
}
window.loadSession = loadSession;

// ===== Display Functions =====
function displayClassificationResult(data) {
    elements.intentType.textContent = data.intent === 'hypothesis' ? '가설' : '질문';
    elements.intentType.className = `result-value intent-badge ${data.intent}`;

    const confidence = Math.round((data.intent_confidence || 0) * 100);
    elements.confidenceValue.textContent = `${confidence}%`;

    if (data.intent === 'hypothesis' && data.novelty_score !== undefined) {
        elements.noveltyCard.classList.remove('hidden');
        const novelty = Math.round(data.novelty_score * 100);
        elements.noveltyValue.textContent = `${novelty}%`;
        elements.noveltyValue.style.color = novelty >= 85 ? '#10b981' : '#f59e0b';
    } else {
        elements.noveltyCard.classList.add('hidden');
    }

    if (data.intent === 'question' && data.feasibility_grade) {
        elements.feasibilityCard.classList.remove('hidden');
        const gradeMap = { 'high': '높음', 'medium': '중간', 'low': '낮음', 'uncertain': '불확실' };
        elements.feasibilityValue.textContent = gradeMap[data.feasibility_grade] || data.feasibility_grade;
    } else {
        elements.feasibilityCard.classList.add('hidden');
    }
}

function displayLiterature(literature) {
    if (!literature || literature.length === 0) {
        elements.literatureList.innerHTML = '<p style="color: var(--color-text-muted);">관련 문헌을 찾지 못했습니다.</p>';
        return;
    }

    elements.literatureList.innerHTML = literature.map(item => `
        <div class="literature-item" ${item.url ? `onclick="window.open('${item.url}', '_blank')"` : ''} style="${item.url ? 'cursor: pointer;' : ''}">
            <div class="literature-title">${escapeHtml(item.title || 'Unknown Title')}</div>
            <div class="literature-meta">
                ${item.authors?.slice(0, 2).join(', ') || 'Unknown'} 
                ${item.year ? `(${item.year})` : ''}
            </div>
        </div>
    `).join('');
}

function displayMethodsMindMap(methods, hypothesis) {
    const icons = { 'analytical': 'A', 'simulation': 'S', 'data_driven': 'D' };

    if (elements.originContent) {
        elements.originContent.textContent = hypothesis.length > 100 ? hypothesis.substring(0, 100) + '...' : hypothesis;
    }

    elements.methodsList.innerHTML = methods.map((method, index) => `
        <div class="method-branch">
            <div class="method-card" onclick="selectMethod(${index})">
                <div class="method-header">
                    <div class="method-icon ${method.type}">${icons[method.type] || 'M'}</div>
                    <div>
                        <div class="method-title">${escapeHtml(method.title)}</div>
                        <div class="method-type">${method.type}</div>
                    </div>
                </div>
                <p class="method-description">${escapeHtml(method.description)}</p>
                <div class="method-footer">
                    <div class="method-meta">
                        ${method.estimated_time || 'N/A'} · ${(method.required_libraries || []).slice(0, 2).join(', ')}
                    </div>
                    <button class="btn-enhance" onclick="event.stopPropagation(); showEnhancement(${index})">
                        가설 보완하기
                    </button>
                </div>
            </div>
            <button class="branch-action" onclick="event.stopPropagation(); showEnhancement(${index})">+</button>
        </div>
    `).join('');

    if (elements.enhancementPanel) {
        elements.enhancementPanel.classList.remove('hidden');
    }
}

function showEnhancement(methodIndex) {
    elements.enhancementPanel?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    elements.enhancementInput?.focus();
}
window.showEnhancement = showEnhancement;

function displayFeasibility(data) {
    const gradeColors = { 'high': '#10b981', 'medium': '#f59e0b', 'low': '#ef4444', 'uncertain': '#6b7280' };
    let content = `<h3 style="color: ${gradeColors[data.feasibility_grade] || '#fff'}">실현 가능성: ${data.feasibility_grade?.toUpperCase() || 'N/A'}</h3>`;
    if (data.final_report) {
        content += `<div>${renderMarkdown(data.final_report)}</div>`;
    }
    elements.feasibilityContent.innerHTML = content;
}

function displayExistingResearch(data) {
    // Remove existing notice if any
    const existing = document.querySelector('.existing-research-notice');
    if (existing) existing.remove();

    // 유사 연구 목록 생성
    let similarList = '';
    if (data.existing_research && data.existing_research.length > 0) {
        similarList = '<ul style="margin-top: 12px; margin-bottom: 16px; padding-left: 20px; color: var(--color-text-secondary); font-size: var(--font-size-sm);">';
        data.existing_research.forEach(title => {
            similarList += `<li style="margin-bottom: 4px;">${escapeHtml(title)}</li>`;
        });
        similarList += '</ul>';
    }

    const message = `
        <div class="existing-research-notice">
            <h3>유사한 기존 연구 발견</h3>
            <p>입력하신 가설과 유사한 연구가 이미 존재합니다. (독창성: ${Math.round(data.novelty_score * 100)}%)</p>
            ${similarList}
            <button onclick="forceContinue('${data.session_id}')" class="btn-warning">
                그래도 실험 진행하기
            </button>
        </div>
    `;
    elements.classificationSection.querySelector('.result-cards').insertAdjacentHTML('afterend', message);
}

async function forceContinue(sessionId) {
    if (!confirm('독창성이 낮은 가설에 대한 실험을 진행하시겠습니까?')) return;

    try {
        switchView('loading');
        updateLoadingMessage('실험 방법론 설계 중...');

        const response = await fetch(`${API_BASE}/api/research/continue`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ user_input: sessionId, domain: '' })
        });

        if (!response.ok) throw new Error('API 오류');

        const data = await response.json();
        currentSession = { ...currentSession, ...data };

        displayMethodsMindMap(data.proposed_methods, elements.hypothesisInput.value.trim());
        switchView('methods');

    } catch (error) {
        console.error('Force continue error:', error);
        alert(`오류 발생: ${error.message}`);
        switchView('analysis');
    }
}
window.forceContinue = forceContinue;

function displayReport(report) {
    if (!report) {
        elements.reportContent.innerHTML = '<p>보고서 생성에 실패했습니다.</p>';
        return;
    }
    elements.reportContent.innerHTML = renderMarkdown(report);
}

// ===== UI Helpers =====
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

        if (progress < 20) elements.experimentStatus.textContent = '코드 생성 중...';
        else if (progress < 50) elements.experimentStatus.textContent = '실험 실행 중...';
        else if (progress < 80) elements.experimentStatus.textContent = '결과 분석 중...';
        else elements.experimentStatus.textContent = '보고서 작성 중...';
    }, 500);
}

function resetApp() {
    currentSession = null;
    selectedMethodIndex = -1;
    uploadedFiles = [];
    if (elements.hypothesisInput) elements.hypothesisInput.value = '';
    if (elements.domainInput) elements.domainInput.value = '';
    if (elements.uploadedFilesContainer) elements.uploadedFilesContainer.innerHTML = '';
    if (elements.progressFill) elements.progressFill.style.width = '0%';
    if (elements.experimentStatus) elements.experimentStatus.textContent = '';
    switchView('input');
    loadSidebarHistory();
}

function escapeHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

function renderMarkdown(text) {
    if (!text) return '';
    return text
        .replace(/^### (.*$)/gim, '<h3>$1</h3>')
        .replace(/^## (.*$)/gim, '<h2>$1</h2>')
        .replace(/^# (.*$)/gim, '<h1>$1</h1>')
        .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
        .replace(/\*(.*?)\*/g, '<em>$1</em>')
        .replace(/```([^`]+)```/g, '<pre><code>$1</code></pre>')
        .replace(/`([^`]+)`/g, '<code>$1</code>')
        .replace(/^> (.*$)/gim, '<blockquote>$1</blockquote>')
        .replace(/^- (.*$)/gim, '<li>$1</li>')
        .replace(/\n\n/g, '</p><p>')
        .replace(/\n/g, '<br>');
}
