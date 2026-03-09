pub(super) fn idempotency_code() -> &'static str {
    r#"return items.map((item, index) => {
  const base = `${item.json.request_title || ""}::${item.json.request_text || ""}::${item.json.topic || ""}::${item.json.timeframe || ""}`;
  const safeBase64 = Buffer.from(base).toString("base64").replace(/[^a-zA-Z0-9]/g, "").slice(0, 32);
  return {
    json: {
      ...item.json,
      idempotency_key: `wf_${safeBase64 || "default"}_${index}`,
      run_started_at: new Date().toISOString(),
      pipeline_contract: "trigger -> fetch -> classify -> summarize -> validate -> publish -> observe"
    }
  };
});"#
}

pub(super) fn extract_code() -> &'static str {
    r#"const fetchPayload = items[0]?.json || {};
const envelope = ($items("Idempotency Guard", 0, 0)?.[0]?.json) || {};
const topN = Math.max(1, Math.min(10, Number(envelope.top_n || 5)));
const hits =
  Array.isArray(fetchPayload.hits) ? fetchPayload.hits :
  Array.isArray(fetchPayload.body?.hits) ? fetchPayload.body.hits :
  [];
const articles = hits
  .map((hit) => ({
    title: String(hit.title || hit.story_title || "").trim(),
    source_url: String(hit.url || hit.story_url || "").trim(),
    published_at: hit.created_at || "",
    source: "hn.algolia",
  }))
  .filter((a) => a.title && a.source_url)
  .slice(0, topN);
return [{
  json: {
    ...envelope,
    request_text: String(envelope.request_text || envelope.request_seed || "").trim(),
    topic: String(envelope.topic || "AI").trim() || "AI",
    notion_token: String(envelope.notion_token || "").trim(),
    notion_parent_page_id: String(envelope.notion_parent_page_id || "").trim(),
    telegram_chat_id: String(envelope.telegram_chat_id || "").trim(),
    telegram_bot_token: String(envelope.telegram_bot_token || "").trim(),
    has_notion_config:
      String(envelope.notion_token || "").trim() && String(envelope.notion_parent_page_id || "").trim()
        ? "true"
        : "false",
    has_telegram_config:
      String(envelope.telegram_bot_token || "").trim() && String(envelope.telegram_chat_id || "").trim()
        ? "true"
        : "false",
    fetch_ok: articles.length >= topN,
    article_count: articles.length,
    articles,
    source_provider: "hn.algolia.search_by_date"
  }
}];"#
}

pub(super) fn classify_code() -> &'static str {
    r#"const root = items[0]?.json || {};
const classifyStack = (title) => {
  const t = String(title || "").toLowerCase();
  const stacks = [];
  if (/(gpu|cuda|nvidia|chip|hardware|accelerator)/.test(t)) stacks.push("GPU/Accelerator");
  if (/(llm|model|transformer|agent)/.test(t)) stacks.push("LLM/Model Serving");
  if (/(vector|rag|embedding|retrieval)/.test(t)) stacks.push("RAG/Retrieval");
  if (/(api|sdk|platform|cloud)/.test(t)) stacks.push("API/Cloud Platform");
  if (/(security|privacy|governance|policy)/.test(t)) stacks.push("Security/Governance");
  return stacks.length ? stacks : ["General AI Application"];
};
const enriched = (root.articles || []).map((article) => ({
  ...article,
  trend_alignment: "current_trend",
  technical_stack: classifyStack(article.title),
}));
return [{ json: { ...root, articles: enriched } }];"#
}

pub(super) fn summarize_code() -> &'static str {
    r#"const root = items[0]?.json || {};
const summarized = (root.articles || []).map((article, idx) => {
  const prevParadigm = "Rule-based/static automation";
  const newParadigm = "LLM-native adaptive workflows";
  const diff = `${article.title}: ${prevParadigm} -> ${newParadigm}`;
  return {
    ...article,
    comparative_summary: {
      previous_paradigm: prevParadigm,
      new_paradigm: newParadigm,
      key_difference: diff
    },
    importance: `Why it matters #${idx + 1}: impacts tooling, deployment, and developer workflow.`
  };
});
return [{ json: { ...root, articles: summarized } }];"#
}

pub(super) fn validate_code() -> &'static str {
    r#"const root = items[0]?.json || {};
const errors = [];
if (!Array.isArray(root.articles) || root.articles.length < 5) {
  errors.push(`expected >=5 articles, got ${Array.isArray(root.articles) ? root.articles.length : 0}`);
}
for (const [idx, article] of (root.articles || []).entries()) {
  if (!article.title) errors.push(`article[${idx}] missing title`);
  if (!article.source_url) errors.push(`article[${idx}] missing source_url`);
  if (!Array.isArray(article.technical_stack) || article.technical_stack.length === 0) {
    errors.push(`article[${idx}] missing technical_stack`);
  }
}
return [{
  json: {
    ...root,
    validation_ok: errors.length === 0 ? "true" : "false",
    validation_errors: errors
  }
}];"#
}

pub(super) fn markdown_code() -> &'static str {
    r#"const root = items[0]?.json || {};
const articles = Array.isArray(root.articles) ? root.articles : [];
const lines = [];
const notionChildren = [];
const nowIso = new Date().toISOString();
const digestDate = nowIso.slice(0, 10);
const topicLabel = String(root.topic || "AI").trim() || "AI";
const toText = (content, url = "") => {
  const text = { content: String(content || "").slice(0, 1800) };
  if (url) {
    text.link = { url };
  }
  return { type: "text", text };
};

lines.push(`# ${root.request_title || "AllvIa AI Trend Digest"}`);
lines.push(`Generated: ${nowIso}`);
lines.push("");
notionChildren.push({
  object: "block",
  type: "heading_1",
  heading_1: { rich_text: [toText(`AllvIa AI Trend Digest (${digestDate})`)] }
});
notionChildren.push({
  object: "block",
  type: "paragraph",
  paragraph: { rich_text: [toText(`주제: ${topicLabel}`)] }
});

for (const [idx, article] of articles.entries()) {
  lines.push(`## ${idx + 1}. [${article.title}](${article.source_url})`);
  lines.push(`- Source: ${article.source_url}`);
  lines.push(`- Published: ${article.published_at || "n/a"}`);
  lines.push(`- Technical Stack: ${(article.technical_stack || []).join(", ")}`);
  lines.push(`- Previous Paradigm: ${article.comparative_summary?.previous_paradigm || "n/a"}`);
  lines.push(`- New Paradigm: ${article.comparative_summary?.new_paradigm || "n/a"}`);
  lines.push(`- Difference: ${article.comparative_summary?.key_difference || "n/a"}`);
  lines.push(`- Why It Matters: ${article.importance || "n/a"}`);
  lines.push("");

  notionChildren.push({
    object: "block",
    type: "heading_2",
    heading_2: { rich_text: [toText(`${idx + 1}. ${article.title || "Untitled"}`)] }
  });
  notionChildren.push({
    object: "block",
    type: "paragraph",
    paragraph: {
      rich_text: [
        toText("원문 링크: "),
        toText("기사 원문 열기", String(article.source_url || ""))
      ]
    }
  });
  notionChildren.push({
    object: "block",
    type: "bulleted_list_item",
    bulleted_list_item: {
      rich_text: [toText(`핵심: ${article.importance || "n/a"}`)]
    }
  });
  notionChildren.push({
    object: "block",
    type: "bulleted_list_item",
    bulleted_list_item: {
      rich_text: [toText(`차이점: ${article.comparative_summary?.key_difference || "n/a"}`)]
    }
  });
  if (idx < articles.length - 1) {
    notionChildren.push({ object: "block", type: "divider", divider: {} });
  }
}

const markdown = lines.join("\n");
const topHeadlinesText = articles
  .slice(0, 5)
  .map((a, idx) => `${idx + 1}. ${a.title}`)
  .join("\n");

return [{
  json: {
    ...root,
    notion_title: `AllvIa AI Trend Digest ${digestDate}`,
    markdown_report: markdown,
    notion_excerpt: markdown.slice(0, 1800),
    notion_children: notionChildren.slice(0, 95),
    telegram_summary: topHeadlinesText,
    top_headlines_text: topHeadlinesText
  }
}];"#
}

pub(super) fn notion_result_code() -> &'static str {
    r#"const base = ($items("Build Markdown Report", 0, 0)?.[0]?.json) || {};
const current = items[0]?.json || {};
const notionError = current?.error?.message ? String(current.error.message) : "";
return [{
  json: {
    ...base,
    notion_status: notionError ? "failed" : "completed",
    notion_error: notionError,
    notion_page_id: String(current.id || current.page_id || ""),
    notion_page_url: String(current.url || "")
  }
}];"#
}

pub(super) fn telegram_result_code() -> &'static str {
    r#"const base = ($items("If Has Telegram Config", 0, 0)?.[0]?.json) || {};
const current = items[0]?.json || {};
const telegramError = current?.error?.message ? String(current.error.message) : "";
const messageId = current?.result?.message_id || current?.message_id || "";
return [{
  json: {
    ...base,
    telegram_status: telegramError ? "failed" : "completed",
    telegram_error: telegramError,
    telegram_message_id: String(messageId || "")
  }
}];"#
}
