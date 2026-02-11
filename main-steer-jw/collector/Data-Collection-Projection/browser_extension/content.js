const CONTENT_SUMMARY_MAX = 600;
const CONTENT_MAX = 4000;
const HEADINGS_MAX = 5;

const DOMAIN_ALLOWLIST = ["*"]; // Example: ["notion.so"] to limit capture.
const BLOCKLIST = ["accounts.google.com", "bank", "login"];

function isHttpUrl(url) {
  return typeof url === "string" && (url.startsWith("http://") || url.startsWith("https://"));
}

function getDomain(url) {
  try {
    return new URL(url).hostname || "";
  } catch {
    return "";
  }
}

function shouldCapture(url) {
  if (!isHttpUrl(url)) {
    return false;
  }
  const domain = getDomain(url);
  if (!domain) {
    return false;
  }
  if (BLOCKLIST.some((item) => domain.includes(item))) {
    return false;
  }
  if (DOMAIN_ALLOWLIST.includes("*")) {
    return true;
  }
  return DOMAIN_ALLOWLIST.some((item) => domain === item || domain.endsWith(`.${item}`));
}

function normalizeText(value) {
  if (!value) {
    return "";
  }
  return value.replace(/\s+/g, " ").trim();
}

function extractHeadings() {
  const nodes = Array.from(document.querySelectorAll("h1, h2"));
  const headings = [];
  for (const node of nodes) {
    const text = normalizeText(node.textContent || "");
    if (!text) {
      continue;
    }
    headings.push(text);
    if (headings.length >= HEADINGS_MAX) {
      break;
    }
  }
  return headings;
}

function extractContentSummary() {
  const selection = normalizeText(window.getSelection?.().toString() || "");
  const headings = extractHeadings();
  const title = normalizeText(document.title || "");
  const bodyText = normalizeText(document.body?.innerText || "");

  const parts = [];
  if (title) {
    parts.push(title);
  }
  if (headings.length) {
    parts.push(headings.join(" | "));
  }
  if (selection) {
    parts.push(`Selection: ${selection}`);
  }
  if (bodyText) {
    parts.push(bodyText.slice(0, CONTENT_SUMMARY_MAX));
  }
  return parts.join(" // ").slice(0, CONTENT_SUMMARY_MAX);
}

function extractContentFull() {
  const bodyText = normalizeText(document.body?.innerText || "");
  if (!bodyText) {
    return "";
  }
  return bodyText.slice(0, CONTENT_MAX);
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (!message || message.type !== "COLLECT_CONTENT") {
    return;
  }
  const url = message.url || window.location.href;
  if (!shouldCapture(url)) {
    sendResponse({ ok: false });
    return;
  }

  const summary = extractContentSummary();
  const full = extractContentFull();
  sendResponse({
    ok: true,
    content_summary: summary,
    content: full,
    content_len: normalizeText(document.body?.innerText || "").length,
  });
});
