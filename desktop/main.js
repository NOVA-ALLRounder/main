import { invoke } from '@tauri-apps/api/core';
import { attachConsole } from '@tauri-apps/plugin-log';
import './style.css';

// DOM Elements
document.querySelector('#app').innerHTML = `
  <header>
    <h1>Steer Agent (Alpha)</h1>
    <span id="status" style="font-size: 0.8rem; color: var(--text-secondary);">Ready</span>
  </header>
  <div id="log-container"></div>
  <div class="chat-input-area">
    <input type="text" id="goal-input" placeholder="What should I do? (e.g., 'Check emails')" autofocus />
    <button id="run-btn">Run</button>
  </div>
`;

const logContainer = document.getElementById('log-container');
const goalInput = document.getElementById('goal-input');
const runBtn = document.getElementById('run-btn');
const statusSpan = document.getElementById('status');

// 1. Setup Logging (Intercept Console)
async function setupLogging() {
  await attachConsole(); // Link Rust logs to WebView console

  // Monkey-patch console to display logs in UI
  const originalLog = console.log;
  const originalInfo = console.info;
  const originalWarn = console.warn;
  const originalError = console.error;

  function appendLog(level, message) {
    const div = document.createElement('div');
    div.className = `log-entry ${level}`;
    
    // Simple parsing to colorize emoji logs from Rust
    if (message.includes('❌')) div.classList.add('error');
    if (message.includes('✅')) div.classList.add('success');
    if (message.includes('🌊')) div.style.color = '#60a5fa'; // Blue
    if (message.includes('🧠')) div.style.color = '#c084fc'; // Purple
    if (message.includes('💡')) div.style.color = '#fbbf24'; // Yellow

    div.textContent = message;
    logContainer.appendChild(div);
    logContainer.scrollTop = logContainer.scrollHeight;
  }

  console.info = (...args) => {
    originalInfo(...args);
    // Filter internal Tauri messages if needed
    appendLog('info', args.join(' '));
  };
  
  console.log = (...args) => {
      originalLog(...args); // Standard logs might be noise
  }

  console.warn = (...args) => {
    originalWarn(...args);
    appendLog('warn', args.join(' '));
  };

  console.error = (...args) => {
    originalError(...args);
    appendLog('error', args.join(' '));
  };
}

// 2. Run Logic
async function runAgent() {
  const goal = goalInput.value.trim();
  if (!goal) return;

  // Reset
  logContainer.innerHTML = '';
  goalInput.disabled = true;
  runBtn.disabled = true;
  statusSpan.textContent = 'Running...';
  statusSpan.style.color = '#4ade80';

  try {
    appendLog('info', `🚀 Launching: "${goal}"`);
    await invoke('run_agent_task', { goal });
    appendLog('success', '✅ Task Completed.');
  } catch (error) {
    appendLog('error', `🔥 System Error: ${error}`);
  } finally {
    goalInput.disabled = false;
    runBtn.disabled = false;
    goalInput.focus();
    statusSpan.textContent = 'Ready';
    statusSpan.style.color = 'var(--text-secondary)';
  }
}

// Helper to access appendLog outside
function appendLog(level, msg) {
    const div = document.createElement('div');
    div.className = `log-entry ${level}`;
    div.textContent = msg;
    logContainer.appendChild(div);
}

runBtn.addEventListener('click', runAgent);
goalInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') runAgent();
});

// Init
setupLogging();
appendLog('info', 'System Initialized. Connected to Local Core.');
