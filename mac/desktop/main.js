import { invoke } from '@tauri-apps/api/core';
import { attachConsole } from '@tauri-apps/plugin-log';
import './style.css';

// --- Element References ---
const logContainer = document.getElementById('log-container');
const goalInput = document.getElementById('goal-input');
const runBtn = document.getElementById('run-btn');
const tabBtns = document.querySelectorAll('.tab-btn');
const tabContents = document.querySelectorAll('.tab-content');
const refreshDashBtn = document.getElementById('refresh-dash');
const recList = document.getElementById('rec-list');
const statSessions = document.getElementById('stat-sessions');
const statTime = document.getElementById('stat-time');
const statPending = document.getElementById('stat-pending');

// Minimal UI Toggle
const toggleDetailsBtn = document.getElementById('toggle-details');
const detailsPanel = document.getElementById('details-panel');

// Routines
const refreshRoutinesBtn = document.getElementById('refresh-routines');
const routineList = document.getElementById('routine-list');

// Settings
const settingsBtn = document.getElementById('settings-btn');
const settingsModal = document.getElementById('settings-modal');
const closeSettingsBtn = document.getElementById('close-settings');
const saveSettingsBtn = document.getElementById('save-settings');
const cfgTelegramToken = document.getElementById('cfg-telegram-token');
const cfgTelegramUser = document.getElementById('cfg-telegram-user');
const cfgMistralKey = document.getElementById('cfg-mistral-key');

// --- Logging System ---
async function setupLogging() {
  await attachConsole();
  const originalInfo = console.info;
  const originalWarn = console.warn;
  const originalError = console.error;

  function appendLog(level, message) {
    if (!logContainer) return;
    const div = document.createElement('div');
    div.className = `log-entry ${level}`;

    if (message.includes('❌')) div.classList.add('error');
    if (message.includes('✅')) div.classList.add('success');
    if (message.includes('🌊')) div.style.color = '#60a5fa'; // Blue
    if (message.includes('🧠')) div.style.color = '#c084fc'; // Purple
    if (message.includes('💡')) div.style.color = '#fbbf24'; // Yellow

    div.textContent = message;
    logContainer.appendChild(div);
    logContainer.scrollTop = logContainer.scrollHeight;
  }

  console.info = (...args) => { originalInfo(...args); appendLog('info', args.join(' ')); };
  console.warn = (...args) => { originalWarn(...args); appendLog('warn', args.join(' ')); };
  console.error = (...args) => { originalError(...args); appendLog('error', args.join(' ')); };
}

function appendLog(level, msg) {
  if (!logContainer) return;
  const div = document.createElement('div');
  div.className = `log-entry ${level}`;
  div.textContent = msg;
  logContainer.appendChild(div);
  logContainer.scrollTop = logContainer.scrollHeight;
}

// --- Agent Logic ---
async function runAgent() {
  const goal = goalInput.value.trim();
  if (!goal) return;

  logContainer.innerHTML = '';
  goalInput.disabled = true;
  runBtn.disabled = true;

  try {
    appendLog('info', `🚀 Launching: "${goal}"`);
    const response = await invoke('run_agent_task', { goal });

    if (response === "Task Completed.") {
      appendLog('success', '✅ Task Completed.');
    } else {
      appendLog('success', `🧠 ${response}`);
    }
  } catch (error) {
    appendLog('error', `🔥 System Error: ${error}`);
  } finally {
    goalInput.disabled = false;
    runBtn.disabled = false;
    goalInput.focus();
  }
}

if (runBtn) runBtn.addEventListener('click', runAgent);
if (goalInput) goalInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') runAgent();
});

// Toggle Details Panel
if (toggleDetailsBtn && detailsPanel) {
  toggleDetailsBtn.addEventListener("click", () => {
    const isVisible = detailsPanel.classList.toggle("visible");
    toggleDetailsBtn.innerHTML = isVisible ? "▼" : "☰";
    // Resize window if needed (optional advanced feature)
  });
}


// --- Dashboard & Tabs ---

tabBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    // Remove active
    tabBtns.forEach(b => b.classList.remove('active'));
    tabContents.forEach(c => c.classList.remove('active'));

    // Add active
    btn.classList.add('active');
    const tabId = btn.getAttribute('data-tab');
    const content = document.getElementById(`content-${tabId}`);
    if (content) content.classList.add('active');

    // Logic
    if (tabId === 'dashboard') refreshDashboard();
    if (tabId === 'routines') refreshRoutines();
  });
});

if (refreshDashBtn) refreshDashBtn.addEventListener('click', refreshDashboard);
if (refreshRoutinesBtn) refreshRoutinesBtn.addEventListener('click', refreshRoutines);

// --- Settings Logic ---
if (settingsBtn) settingsBtn.addEventListener('click', openSettings);
if (closeSettingsBtn) closeSettingsBtn.addEventListener('click', closeSettings);
if (saveSettingsBtn) saveSettingsBtn.addEventListener('click', saveSettings);

async function openSettings() {
  settingsModal.classList.remove('hidden');
  settingsModal.classList.add('visible');

  // Load config
  try {
    const cfg = await invoke('get_config');
    if (cfgTelegramToken) cfgTelegramToken.value = cfg['TELEGRAM_BOT_TOKEN'] || '';
    if (cfgTelegramUser) cfgTelegramUser.value = cfg['TELEGRAM_USER_ID'] || '';
    if (cfgMistralKey) cfgMistralKey.value = cfg['MISTRAL_API_KEY'] || '';
  } catch (e) {
    console.error("Config Load Failed", e);
  }
}

function closeSettings() {
  settingsModal.classList.remove('visible');
  settingsModal.classList.add('hidden');
}

async function saveSettings() {
  try {
    if (cfgTelegramToken.value) await invoke('set_config', { key: 'TELEGRAM_BOT_TOKEN', value: cfgTelegramToken.value });
    if (cfgTelegramUser.value) await invoke('set_config', { key: 'TELEGRAM_USER_ID', value: cfgTelegramUser.value });
    if (cfgMistralKey.value) await invoke('set_config', { key: 'MISTRAL_API_KEY', value: cfgMistralKey.value });

    alert('Settings Saved! Restart app to apply changes.');
    closeSettings();
  } catch (e) {
    alert('Failed to save: ' + e);
  }
}


// --- Data Fetching ---

async function refreshDashboard() {
  try {
    const stats = await invoke('get_dashboard_data');
    if (statSessions) statSessions.textContent = stats.total_sessions;
    if (statTime) statTime.textContent = `${stats.total_time_mins}m`;
    if (statPending) statPending.textContent = stats.rec_pending;

    const recs = await invoke('get_recommendations');
    renderRecommendations(recs);
  } catch (e) { console.error(e); }
}

async function refreshRoutines() {
  try {
    const routines = await invoke('list_routines');
    if (!routineList) return;
    routineList.innerHTML = '';

    if (routines.length === 0) {
      routineList.innerHTML = '<div class="empty-state">No routines created yet. Use the Architect!</div>';
      return;
    }

    routines.forEach(r => {
      const card = document.createElement('div');
      card.className = 'rec-card';
      card.innerHTML = `
                <div class="rec-content">
                    <h4>${r.name}</h4>
                    <p>Created: ${new Date(r.created_at).toLocaleString()}</p>
                </div>
                <div class="rec-actions">
                    <button class="btn-sm" style="background:var(--error)" onclick="window.deleteRoutine(${r.id})">Delete</button>
                    <button class="btn-sm btn-outline">Run</button>
                </div>
            `;
      routineList.appendChild(card);
    });
  } catch (e) { console.error(e); }
}

function renderRecommendations(recs) {
  if (!recList) return;
  recList.innerHTML = '';
  if (recs.length === 0) {
    recList.innerHTML = '<div class="empty-state">No suggestions yet.</div>';
    return;
  }
  recs.forEach(rec => {
    const card = document.createElement('div');
    card.className = 'rec-card';
    card.innerHTML = `
            <div class="rec-content">
                <h4>${rec.title}</h4>
                <p>${rec.summary}</p>
                <div class="rec-meta">
                    <span>⚡️ ${rec.trigger}</span>
                    <span>🧠 ${(rec.confidence * 100).toFixed(0)}%</span>
                </div>
            </div>
            <div class="rec-actions">
                <button class="btn-sm" style="background:var(--success)" onclick="window.startArchitect(${rec.id})">Build</button>
            </div>
        `;
    recList.appendChild(card);
  });
}

// Global Actions
window.startArchitect = async function (recId) {
  try {
    document.querySelector('[data-tab="chat"]').click();
    appendLog('info', '🏗️ Initializing Architect Mode...');
    const intro = await invoke('start_architect_mode', { rec_id: recId });
    appendLog('info', `🧠 Architect: ${intro}`);
  } catch (e) {
    appendLog('error', `Failed to start Architect: ${e}`);
  }
};

window.deleteRoutine = async function (id) {
  if (!confirm('Are you sure you want to delete this routine?')) return;
  try {
    await invoke('delete_routine', { id });
    refreshRoutines();
  } catch (e) { alert(e); }
};

// Init
setupLogging();
