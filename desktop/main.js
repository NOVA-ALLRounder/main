import './style.css'

// Agent API URL
const API_BASE = 'http://localhost:5679/api';

document.querySelector('#app').innerHTML = `
  <header class="header">
    <div class="logo">
      <div class="logo-icon">🤖</div>
      <span class="logo-text">Steer Agent</span>
    </div>
    <div class="status-badge" id="statusBadge">
      <span class="status-dot"></span>
      <span id="statusText">Connecting...</span>
    </div>
  </header>

  <main class="main-content">
    <section class="chat-section">
      <div class="chat-messages" id="messages">
        <div class="message agent">
          <span class="emoji">👋</span> 안녕하세요! 무엇을 도와드릴까요?
          <br><br>
          자연어로 말씀해주세요:
          <br>• "이메일 보여줘"
          <br>• "오늘 일정 뭐야?"
          <br>• "시스템 상태 알려줘"
        </div>
      </div>
      <div class="input-section">
        <div class="input-wrapper">
          <input 
            type="text" 
            class="chat-input" 
            id="userInput"
            placeholder="자연어로 명령을 입력하세요..."
            autocomplete="off"
          />
        </div>
        <button class="send-btn" id="sendBtn">전송</button>
      </div>
    </section>

    <aside class="sidebar">
      <div class="card">
        <div class="card-title">빠른 실행</div>
        <div class="quick-actions">
          <button class="action-btn" data-cmd="이메일 보여줘">
            <span class="icon">📧</span> 이메일 확인
          </button>
          <button class="action-btn" data-cmd="오늘 일정 뭐야?">
            <span class="icon">📅</span> 오늘 일정
          </button>
          <button class="action-btn" data-cmd="시스템 상태 알려줘">
            <span class="icon">📊</span> 시스템 상태
          </button>
          <button class="action-btn" data-cmd="analyze_patterns">
            <span class="icon">🔍</span> 패턴 분석
          </button>
        </div>
      </div>

      <div class="card">
        <div class="card-title">시스템</div>
        <div class="system-stats">
          <div>
            <div class="stat-row">
              <span class="stat-label">CPU</span>
              <span class="stat-value" id="cpuValue">--</span>
            </div>
            <div class="stat-bar">
              <div class="stat-bar-fill" id="cpuBar" style="width: 0%"></div>
            </div>
          </div>
          <div>
            <div class="stat-row">
              <span class="stat-label">RAM</span>
              <span class="stat-value" id="ramValue">--</span>
            </div>
            <div class="stat-bar">
              <div class="stat-bar-fill" id="ramBar" style="width: 0%"></div>
            </div>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-title">추천 워크플로우</div>
        <div class="recommendations" id="recsContainer">
          <div class="loading">로딩 중...</div>
        </div>
      </div>

      <div class="card">
        <div class="card-title">연동 서비스</div>
        <div class="integrations">
          <div class="integration-badge">
            <span class="integration-dot"></span> Gmail
          </div>
          <div class="integration-badge">
            <span class="integration-dot"></span> Calendar
          </div>
          <div class="integration-badge">
            <span class="integration-dot"></span> Telegram
          </div>
          <div class="integration-badge">
            <span class="integration-dot"></span> Notion
          </div>
          <div class="integration-badge">
            <span class="integration-dot"></span> n8n
          </div>
        </div>
      </div>
      <div class="card">
        <div class="card-title">Dev Tools</div>
        <div class="dev-toggle">
          <span>Debug Log</span>
          <label class="switch">
            <input type="checkbox" id="debugToggle" />
            <span class="slider"></span>
          </label>
        </div>
        <div class="dev-log hidden" id="devLog"></div>
      </div>
    </aside>
  </main>
`;

// Elements
const messagesEl = document.getElementById('messages');
const inputEl = document.getElementById('userInput');
const sendBtn = document.getElementById('sendBtn');
const statusBadge = document.getElementById('statusBadge');
const statusText = document.getElementById('statusText');
const recsContainer = document.getElementById('recsContainer');
const debugToggle = document.getElementById('debugToggle');
const devLog = document.getElementById('devLog');

const debugStorageKey = 'steerDebugEnabled';
let debugEnabled = false;

// Add message to chat
function addMessage(text, type = 'agent') {
  const msg = document.createElement('div');
  msg.className = `message ${type}`;
  msg.innerHTML = text;
  messagesEl.appendChild(msg);
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

function setDebugEnabled(enabled) {
  debugEnabled = enabled;
  if (debugToggle) {
    debugToggle.checked = enabled;
  }
  if (devLog) {
    devLog.classList.toggle('hidden', !enabled);
    if (!enabled) {
      devLog.innerHTML = '';
    }
  }
  try {
    localStorage.setItem(debugStorageKey, enabled ? 'true' : 'false');
  } catch (_) {
    // Ignore storage failures in dev mode.
  }
}

function debugLog(label, detail) {
  if (!debugEnabled || !devLog) {
    return;
  }

  const line = document.createElement('div');
  line.className = 'dev-log-line';

  const timeEl = document.createElement('span');
  timeEl.className = 'dev-time';
  timeEl.textContent = new Date().toLocaleTimeString();

  const labelEl = document.createElement('span');
  labelEl.className = 'dev-label';
  labelEl.textContent = label;

  const detailEl = document.createElement('span');
  detailEl.className = 'dev-detail';
  detailEl.textContent = detail;

  line.append(timeEl, labelEl, detailEl);
  devLog.appendChild(line);
  devLog.scrollTop = devLog.scrollHeight;
}

function nowMs() {
  if (typeof performance !== 'undefined' && typeof performance.now === 'function') {
    return performance.now();
  }
  return Date.now();
}

// Check API health
async function checkHealth() {
  try {
    debugLog('health:check', 'ping');
    const resp = await fetch(`${API_BASE}/health`);
    if (resp.ok) {
      statusBadge.classList.add('connected');
      statusText.textContent = 'Running';
      debugLog('health:ok', 'connected');
      return true;
    }
  } catch (e) {
    statusBadge.classList.remove('connected');
    statusText.textContent = 'Disconnected';
    debugLog('health:error', String(e));
  }
  debugLog('health:fail', 'disconnected');
  return false;
}

// Fetch system status
async function fetchStatus() {
  const startedAt = nowMs();
  debugLog('status:fetch', 'request');
  try {
    const resp = await fetch(`${API_BASE}/status`);
    if (resp.ok) {
      const data = await resp.json();
      document.getElementById('cpuValue').textContent = data.cpu.toFixed(1) + '%';
      document.getElementById('ramValue').textContent = data.ram.toFixed(1) + '%';
      document.getElementById('cpuBar').style.width = Math.min(data.cpu, 100) + '%';
      document.getElementById('ramBar').style.width = Math.min(data.ram, 100) + '%';
      const elapsed = Math.round(nowMs() - startedAt);
      debugLog('status:ok', `cpu=${data.cpu.toFixed(1)} ram=${data.ram.toFixed(1)} ${elapsed}ms`);
    } else {
      const elapsed = Math.round(nowMs() - startedAt);
      debugLog('status:fail', `status=${resp.status} ${elapsed}ms`);
    }
  } catch (e) {
    const elapsed = Math.round(nowMs() - startedAt);
    debugLog('status:error', `${e} ${elapsed}ms`);
    console.error('Failed to fetch status:', e);
  }
}

// Fetch recommendations
async function fetchRecommendations() {
  const startedAt = nowMs();
  debugLog('recs:fetch', 'request');
  try {
    const resp = await fetch(`${API_BASE}/recommendations`);
    if (resp.ok) {
      const recs = await resp.json();
      const elapsed = Math.round(nowMs() - startedAt);
      debugLog('recs:ok', `count=${recs.length} ${elapsed}ms`);
      if (recs.length === 0) {
        recsContainer.innerHTML = '<div class="empty">추천 없음</div>';
      } else {
        recsContainer.innerHTML = recs.map(rec => `
          <div class="rec-item">
            <div class="rec-title">${rec.title}</div>
            <div class="rec-confidence">${(rec.confidence * 100).toFixed(0)}%</div>
            <div class="rec-actions">
              <button onclick="approveRec(${rec.id})" class="rec-btn approve">✓</button>
              <button onclick="rejectRec(${rec.id})" class="rec-btn reject">✗</button>
            </div>
          </div>
        `).join('');
      }
    } else {
      const elapsed = Math.round(nowMs() - startedAt);
      debugLog('recs:fail', `status=${resp.status} ${elapsed}ms`);
    }
  } catch (e) {
    const elapsed = Math.round(nowMs() - startedAt);
    debugLog('recs:error', `${e} ${elapsed}ms`);
    recsContainer.innerHTML = '<div class="error">연결 실패</div>';
  }
}

// Approve/reject recommendations
window.approveRec = async (id) => {
  const startedAt = nowMs();
  debugLog('recs:approve', `id=${id}`);
  try {
    const resp = await fetch(`${API_BASE}/recommendations/${id}/approve`, { method: 'POST' });
    if (resp.ok) {
      addMessage(`✅ 추천 #${id} 승인됨! n8n 워크플로우 생성 중...`, 'agent');
      const elapsed = Math.round(nowMs() - startedAt);
      debugLog('recs:approve:ok', `id=${id} ${elapsed}ms`);
    } else {
      addMessage('❌ 승인 처리에 실패했습니다.', 'agent');
      const elapsed = Math.round(nowMs() - startedAt);
      debugLog('recs:approve:fail', `id=${id} status=${resp.status}`);
    }
  } catch (e) {
    addMessage(`❌ 네트워크 오류 발생: ${e}`, 'agent');
    const elapsed = Math.round(nowMs() - startedAt);
    debugLog('recs:approve:error', `id=${id} ${e}`);
  }
  fetchRecommendations();
};

window.rejectRec = async (id) => {
  const startedAt = nowMs();
  debugLog('recs:reject', `id=${id}`);
  try {
    const resp = await fetch(`${API_BASE}/recommendations/${id}/reject`, { method: 'POST' });
    if (resp.ok) {
      addMessage(`🗑️ 추천 #${id} 거절됨`, 'agent');
      const elapsed = Math.round(nowMs() - startedAt);
      debugLog('recs:reject:ok', `id=${id} ${elapsed}ms`);
    } else {
      addMessage('❌ 거절 처리에 실패했습니다.', 'agent');
      const elapsed = Math.round(nowMs() - startedAt);
      debugLog('recs:reject:fail', `id=${id} status=${resp.status}`);
    }
  } catch (e) {
    addMessage(`❌ 네트워크 오류 발생: ${e}`, 'agent');
    const elapsed = Math.round(nowMs() - startedAt);
    debugLog('recs:reject:error', `id=${id} ${e}`);
  }
  fetchRecommendations();
};

// Send chat message
async function sendCommand(message) {
  const startedAt = nowMs();
  addMessage(message, 'user');
  debugLog('chat:send', message);

  try {
    const resp = await fetch(`${API_BASE}/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message }),
    });

    if (resp.ok) {
      const data = await resp.json();
      addMessage(data.response, 'agent');
      debugLog('chat:ok', `command=${data.command || 'none'}`);
      debugLog('chat:latency', `${Math.round(nowMs() - startedAt)}ms`);

      // Update status after certain commands
      if (data.command === 'system_status') {
        fetchStatus();
      }
    } else {
      debugLog('chat:fail', `status=${resp.status}`);
      debugLog('chat:latency', `${Math.round(nowMs() - startedAt)}ms`);
      addMessage('❌ 서버 오류가 발생했습니다.', 'agent');
    }
  } catch (e) {
    debugLog('chat:error', String(e));
    debugLog('chat:latency', `${Math.round(nowMs() - startedAt)}ms`);
    addMessage('❌ 서버에 연결할 수 없습니다. 에이전트가 실행 중인지 확인하세요.', 'agent');
  }
}

async function runPatternAnalysis() {
  const startedAt = nowMs();
  addMessage('🔍 패턴 분석을 시작합니다... (잠시만 기다려주세요)', 'agent');
  debugLog('patterns:run', 'request');

  try {
    const resp = await fetch(`${API_BASE}/patterns/analyze`, { method: 'POST' });
    if (!resp.ok) {
      debugLog('patterns:fail', `status=${resp.status}`);
      debugLog('patterns:latency', `${Math.round(nowMs() - startedAt)}ms`);
      addMessage('❌ 패턴 분석에 실패했습니다. 잠시 후 다시 시도해주세요.', 'agent');
      return;
    }

    const data = await resp.json();
    debugLog('patterns:ok', `count=${Array.isArray(data) ? data.length : 0}`);
    debugLog('patterns:latency', `${Math.round(nowMs() - startedAt)}ms`);

    if (Array.isArray(data) && data.length > 0) {
      const lines = data.map(item => `• ${item}`).join('<br>');
      addMessage(`✅ 패턴 분석 완료:<br>${lines}`, 'agent');
    } else {
      addMessage('✅ 패턴 분석 완료. 새로 감지된 패턴은 없습니다.', 'agent');
    }
  } catch (e) {
    debugLog('patterns:error', String(e));
    debugLog('patterns:latency', `${Math.round(nowMs() - startedAt)}ms`);
    addMessage(`❌ 네트워크 오류로 패턴 분석에 실패했습니다: ${e}`, 'agent');
  }

  fetchRecommendations();
}

// Event Listeners
sendBtn.addEventListener('click', () => {
  const text = inputEl.value.trim();
  if (text) {
    sendCommand(text);
    inputEl.value = '';
  }
});

inputEl.addEventListener('keypress', (e) => {
  if (e.key === 'Enter') {
    const text = inputEl.value.trim();
    if (text) {
      sendCommand(text);
      inputEl.value = '';
    }
  }
});

// Quick action buttons
document.querySelectorAll('.action-btn').forEach(btn => {
  btn.addEventListener('click', () => {
    const cmd = btn.dataset.cmd;
    if (cmd === 'analyze_patterns') {
      runPatternAnalysis();
      return;
    }
    sendCommand(cmd);
  });
});

if (debugToggle) {
  const stored = (() => {
    try {
      return localStorage.getItem(debugStorageKey);
    } catch (_) {
      return null;
    }
  })();
  setDebugEnabled(stored === 'true');
  debugToggle.addEventListener('change', () => {
    setDebugEnabled(debugToggle.checked);
  });
} else {
  setDebugEnabled(false);
}

// Initial load
checkHealth();
fetchStatus();
fetchRecommendations();

// Periodic updates
setInterval(fetchStatus, 5000);
setInterval(fetchRecommendations, 10000);
setInterval(checkHealth, 30000);
