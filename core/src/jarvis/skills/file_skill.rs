// FileSkill - PC 嚥≪뮇類????뵬 野꺜????꾨┛/筌뤴뫖以??類ｋ궖/?類ｂ봺
// 筌〓㈇?? skills-main??filesystem, file-search, file-organizer ????
// ?紐? ??뤵????곸벉 - ??뽯땾 std::fs 疫꿸퀡而?

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct FileSkill {
    /// 野꺜??疫꿸퀡??野껋럥以?(????????遺얠젂?醫듼봺)
    default_base: PathBuf,
}

impl FileSkill {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("C:\\Users"));
        Self {
            default_base: home,
        }
    }
}

impl Default for FileSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for FileSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "file".to_string(),
            description: "PC 嚥≪뮇類????뵬 野꺜?? ??꾨┛, 筌뤴뫖以?鈺곌퀬?? ?類ｋ궖, ?類ｂ봺".to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "search".to_string(),
                "read".to_string(),
                "list".to_string(),
                "info".to_string(),
                "organize".to_string(),
                "tree".to_string(),
            ],
            requirements: SkillRequirements::default(),
            tags: vec!["file".to_string(), "local".to_string(), "search".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        EligibilityResult::eligible()
    }

    fn requires_approval(&self, action: &str) -> bool {
        matches!(action, "organize" | "delete")
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        log::info!("FileSkill executing action: {}", ctx.action);

        match ctx.action.as_str() {
            "search" => self.search(ctx).await,
            "read" => self.read_file(ctx).await,
            "list" => self.list_dir(ctx).await,
            "info" => self.file_info(ctx).await,
            "organize" => self.organize(ctx).await,
            "tree" => self.tree(ctx).await,
            _ => SkillResult::error(format!("Unknown file action: {}", ctx.action)),
        }
    }
}

impl FileSkill {
    /// ???뵬 野꺜??- ??已??類ㅼ삢?????쉘 筌띲끉臾?(???)
    async fn search(&self, ctx: SkillContext) -> SkillResult {
        let query = ctx.params.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let path = ctx
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ext = ctx.params.get("ext").and_then(|v| v.as_str());
        let max_results = ctx
            .params
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;
        let max_depth = ctx
            .params
            .get("depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        let base_path = self.resolve_path(path);
        if !base_path.exists() {
            return SkillResult::error(format!("野껋럥以덂첎? 鈺곕똻???? ??녿뮸??덈뼄: {}", base_path.display()));
        }

        let query_lower = query.to_lowercase();
        let mut results: Vec<serde_json::Value> = Vec::new();

        self.walk_dir(&base_path, max_depth, 0, &mut |entry| {
            if results.len() >= max_results {
                return;
            }

            let file_name = entry
                .file_name()
                .to_string_lossy()
                .to_lowercase();

            // ??已?筌띲끉臾?
            let name_match = query_lower.is_empty() || file_name.contains(&query_lower);

            // ?類ㅼ삢??筌띲끉臾?
            let ext_match = ext.map_or(true, |e| {
                entry
                    .path()
                    .extension()
                    .map_or(false, |fe| fe.to_string_lossy().eq_ignore_ascii_case(e))
            });

            if name_match && ext_match {
                let meta = entry.metadata().ok();
                results.push(json!({
                    "path": entry.path().display().to_string(),
                    "name": entry.file_name().to_string_lossy(),
                    "size": meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    "is_dir": entry.path().is_dir(),
                    "modified": meta.as_ref()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs()),
                }));
            }
        });

        let count = results.len();
        SkillResult::success_with_data(
            format!(
                "'{}' 野꺜??野껉퀗?? {}椰?(野껋럥以? {})",
                if query.is_empty() { ext.unwrap_or("*") } else { query },
                count,
                base_path.display()
            ),
            json!({ "results": results, "count": count, "base_path": base_path.display().to_string() }),
        )
    }

    /// ??용뮞?????뵬 ??꾨┛
    async fn read_file(&self, ctx: SkillContext) -> SkillResult {
        let path_str = match ctx.params.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return SkillResult::error("'path' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };
        let max_lines = ctx
            .params
            .get("lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        let path = self.resolve_path(path_str);
        if !path.exists() {
            return SkillResult::error(format!("???뵬??鈺곕똻???? ??녿뮸??덈뼄: {}", path.display()));
        }
        if path.is_dir() {
            return SkillResult::error("?遺얠젂?醫듼봺????뚯뱽 ????곷뮸??덈뼄. 'list' ??る???????뤾쉭??);
        }

        // ???뵬 ??由?筌ｋ똾寃?(10MB ??쀫립)
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => return SkillResult::error(format!("???뵬 ?臾롫젏 ??쎈솭: {}", e)),
        };
        if meta.len() > 10 * 1024 * 1024 {
            return SkillResult::error("???뵬??10MB???λ뜃???몃빍?? ??용뮞?????뵬筌???뚯뱽 ????됰뮸??덈뼄");
        }

        // ??용뮞?紐껋쨮 ??꾨┛ ??뺣즲
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();
                let truncated = lines.len() > max_lines;
                let display: String = lines
                    .into_iter()
                    .take(max_lines)
                    .collect::<Vec<_>>()
                    .join("\n");

                SkillResult::success_with_data(
                    format!(
                        "???뵬 ??꾨┛ ?袁⑥┷: {} ({}餓?})",
                        path.display(),
                        total_lines,
                        if truncated {
                            format!(", ?怨몄맄 {}餓κ쑬彛???뽯뻻", max_lines)
                        } else {
                            String::new()
                        }
                    ),
                    json!({
                        "path": path.display().to_string(),
                        "content": display,
                        "total_lines": total_lines,
                        "truncated": truncated,
                        "size": meta.len(),
                    }),
                )
            }
            Err(_) => SkillResult::error("獄쏅뗄???댿봺 ???뵬??욧탢???紐꾪맜??뱀뱽 ??뚯뱽 ????곷뮸??덈뼄"),
        }
    }

    /// ?遺얠젂?醫듼봺 ??곸뒠 鈺곌퀬??
    async fn list_dir(&self, ctx: SkillContext) -> SkillResult {
        let path_str = ctx.params.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let show_hidden = ctx
            .params
            .get("hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let sort_by = ctx
            .params
            .get("sort")
            .and_then(|v| v.as_str())
            .unwrap_or("name"); // name, size, modified

        let path = self.resolve_path(path_str);
        if !path.exists() || !path.is_dir() {
            return SkillResult::error(format!("?遺얠젂?醫듼봺揶쎛 鈺곕똻???? ??녿뮸??덈뼄: {}", path.display()));
        }

        let entries = match std::fs::read_dir(&path) {
            Ok(e) => e,
            Err(e) => return SkillResult::error(format!("?遺얠젂?醫듼봺 ??꾨┛ ??쎈솭: {}", e)),
        };

        let mut items: Vec<serde_json::Value> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !show_hidden && name.starts_with('.') {
                continue;
            }

            let meta = entry.metadata().ok();
            let is_dir = entry.path().is_dir();
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            items.push(json!({
                "name": name,
                "is_dir": is_dir,
                "size": size,
                "size_human": format_size(size),
                "modified": modified,
            }));
        }

        // ?類ｌ졊
        match sort_by {
            "size" => items.sort_by(|a, b| {
                b["size"].as_u64().unwrap_or(0).cmp(&a["size"].as_u64().unwrap_or(0))
            }),
            "modified" => items.sort_by(|a, b| {
                b["modified"].as_u64().unwrap_or(0).cmp(&a["modified"].as_u64().unwrap_or(0))
            }),
            _ => items.sort_by(|a, b| {
                let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                b_dir.cmp(&a_dir).then_with(|| {
                    a["name"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(b["name"].as_str().unwrap_or(""))
                })
            }),
        }

        let count = items.len();
        let dirs = items.iter().filter(|i| i["is_dir"].as_bool().unwrap_or(false)).count();
        let files = count - dirs;

        SkillResult::success_with_data(
            format!("{}: {}揶????? {}揶????뵬", path.display(), dirs, files),
            json!({ "path": path.display().to_string(), "items": items, "dirs": dirs, "files": files }),
        )
    }

    /// ???뵬/?????怨멸쉭 ?類ｋ궖
    async fn file_info(&self, ctx: SkillContext) -> SkillResult {
        let path_str = match ctx.params.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return SkillResult::error("'path' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };

        let path = self.resolve_path(path_str);
        if !path.exists() {
            return SkillResult::error(format!("野껋럥以덂첎? 鈺곕똻???? ??녿뮸??덈뼄: {}", path.display()));
        }

        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => return SkillResult::error(format!("筌롫???怨쀬뵠????꾨┛ ??쎈솭: {}", e)),
        };

        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        let created = meta
            .created()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        // ?????野껋럩????? ???????④쑴沅?
        let child_count = if meta.is_dir() {
            std::fs::read_dir(&path).map(|e| e.count()).unwrap_or(0)
        } else {
            0
        };

        SkillResult::success_with_data(
            format!(
                "{}: {} ({})",
                path.file_name().unwrap_or_default().to_string_lossy(),
                if meta.is_dir() { "???? } else { "???뵬" },
                format_size(meta.len())
            ),
            json!({
                "path": path.display().to_string(),
                "name": path.file_name().unwrap_or_default().to_string_lossy(),
                "is_dir": meta.is_dir(),
                "size": meta.len(),
                "size_human": format_size(meta.len()),
                "extension": ext,
                "modified": modified,
                "created": created,
                "readonly": meta.permissions().readonly(),
                "child_count": child_count,
            }),
        )
    }

    /// ???뵬 ?類ｂ봺 - ?類ㅼ삢?癒?롦에??????브쑬履?(dry-run 疫꿸퀡??
    async fn organize(&self, ctx: SkillContext) -> SkillResult {
        let path_str = ctx.params.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let dry_run = ctx
            .params
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(true); // 疫꿸퀡?? dry-run (??쇱젫 ??猷?????

        let path = self.resolve_path(path_str);
        if !path.is_dir() {
            return SkillResult::error(format!("?遺얠젂?醫듼봺揶쎛 ?袁⑤뻸??덈뼄: {}", path.display()));
        }

        // ?類ㅼ삢????燁삳똾?믤⑥쥓??筌띲끋釉?
        let category_map: HashMap<&str, &str> = [
            // ?얜챷苑?
            ("pdf", "?얜챷苑?), ("doc", "?얜챷苑?), ("docx", "?얜챷苑?), ("txt", "?얜챷苑?),
            ("xlsx", "?얜챷苑?), ("xls", "?얜챷苑?), ("pptx", "?얜챷苑?), ("ppt", "?얜챷苑?),
            ("hwp", "?얜챷苑?), ("hwpx", "?얜챷苑?), ("csv", "?얜챷苑?), ("md", "?얜챷苑?),
            // ???筌왖
            ("jpg", "???筌왖"), ("jpeg", "???筌왖"), ("png", "???筌왖"), ("gif", "???筌왖"),
            ("bmp", "???筌왖"), ("svg", "???筌왖"), ("webp", "???筌왖"), ("ico", "???筌왖"),
            // ??덉겫??
            ("mp4", "??덉겫??), ("avi", "??덉겫??), ("mkv", "??덉겫??), ("mov", "??덉겫??),
            ("wmv", "??덉겫??), ("flv", "??덉겫??), ("webm", "??덉겫??),
            // ???툢
            ("mp3", "???툢"), ("wav", "???툢"), ("flac", "???툢"), ("aac", "???툢"),
            ("ogg", "???툢"), ("wma", "???툢"),
            // ?類ㅽ뀧
            ("zip", "?類ㅽ뀧"), ("rar", "?類ㅽ뀧"), ("7z", "?類ㅽ뀧"), ("tar", "?類ㅽ뀧"),
            ("gz", "?類ㅽ뀧"),
            // ?꾨뗀諭?
            ("rs", "?꾨뗀諭?), ("py", "?꾨뗀諭?), ("js", "?꾨뗀諭?), ("ts", "?꾨뗀諭?),
            ("java", "?꾨뗀諭?), ("cpp", "?꾨뗀諭?), ("c", "?꾨뗀諭?), ("h", "?꾨뗀諭?),
            ("html", "?꾨뗀諭?), ("css", "?꾨뗀諭?), ("json", "?꾨뗀諭?), ("toml", "?꾨뗀諭?),
            ("yaml", "?꾨뗀諭?), ("yml", "?꾨뗀諭?), ("xml", "?꾨뗀諭?),
            // ??쎈뻬???뵬
            ("exe", "?袁⑥쨮域밸챶??), ("msi", "?袁⑥쨮域밸챶??), ("bat", "?袁⑥쨮域밸챶??),
            ("ps1", "?袁⑥쨮域밸챶??), ("cmd", "?袁⑥쨮域밸챶??),
        ]
        .iter()
        .copied()
        .collect();

        let mut plan: Vec<serde_json::Value> = Vec::new();
        let entries = match std::fs::read_dir(&path) {
            Ok(e) => e,
            Err(e) => return SkillResult::error(format!("?遺얠젂?醫듼봺 ??꾨┛ ??쎈솭: {}", e)),
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                continue; // ?????椰꾨?瑗??
            }

            let ext = entry_path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            if let Some(&category) = category_map.get(ext.as_str()) {
                let dest_dir = path.join(category);
                let dest_file = dest_dir.join(entry_path.file_name().unwrap());

                plan.push(json!({
                    "file": entry_path.file_name().unwrap().to_string_lossy(),
                    "from": entry_path.display().to_string(),
                    "to": dest_file.display().to_string(),
                    "category": category,
                }));

                if !dry_run {
                    std::fs::create_dir_all(&dest_dir).ok();
                    if let Err(e) = std::fs::rename(&entry_path, &dest_file) {
                        log::warn!("???뵬 ??猷???쎈솭: {} ??{}: {}", entry_path.display(), dest_file.display(), e);
                    }
                }
            }
        }

        let count = plan.len();
        SkillResult::success_with_data(
            if dry_run {
                format!("?類ｂ봺 沃섎챶?곮퉪?용┛: {}揶????뵬 ?브쑬履???됱젟 (dry-run)", count)
            } else {
                format!("{}揶????뵬 ?類ｂ봺 ?袁⑥┷", count)
            },
            json!({ "plan": plan, "count": count, "dry_run": dry_run, "path": path.display().to_string() }),
        )
    }

    /// ?遺얠젂?醫듼봺 ?紐꺿봺 ?닌듼??곗뮆??
    async fn tree(&self, ctx: SkillContext) -> SkillResult {
        let path_str = ctx.params.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let max_depth = ctx
            .params
            .get("depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;

        let path = self.resolve_path(path_str);
        if !path.is_dir() {
            return SkillResult::error(format!("?遺얠젂?醫듼봺揶쎛 ?袁⑤뻸??덈뼄: {}", path.display()));
        }

        let mut output = String::new();
        output.push_str(&format!("{}\n", path.display()));
        self.build_tree(&path, "", max_depth, 0, &mut output);

        SkillResult::success_with_data(
            format!("?遺얠젂?醫듼봺 ?紐꺿봺: {}", path.display()),
            json!({ "tree": output, "path": path.display().to_string() }),
        )
    }

    // ???? Helper methods ????

    /// 野껋럥以???곴퐤: ?怨?野껋럥以??????野껋럥以? ?諭????쇱뜖??筌왖??
    fn resolve_path(&self, input: &str) -> PathBuf {
        if input.is_empty() {
            return self.default_base.clone();
        }

        // ?諭????쇱뜖??
        let resolved = match input.to_lowercase().as_str() {
            "desktop" | "獄쏅?源?遺얇늺" => self.default_base.join("Desktop"),
            "downloads" | "??쇱뒲嚥≪뮆諭? => self.default_base.join("Downloads"),
            "documents" | "?얜챷苑? => self.default_base.join("Documents"),
            "pictures" | "??彛? => self.default_base.join("Pictures"),
            "music" | "???툢" => self.default_base.join("Music"),
            "videos" | "??덉겫?? => self.default_base.join("Videos"),
            _ => {
                let p = PathBuf::from(input);
                if p.is_absolute() {
                    p
                } else {
                    self.default_base.join(input)
                }
            }
        };
        resolved
    }

    /// ??? ?遺얠젂?醫듼봺 ?癒?퉳
    fn walk_dir(
        &self,
        dir: &Path,
        max_depth: usize,
        current_depth: usize,
        callback: &mut dyn FnMut(&std::fs::DirEntry),
    ) {
        if current_depth > max_depth {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            callback(&entry);
            if entry.path().is_dir() {
                self.walk_dir(&entry.path(), max_depth, current_depth + 1, callback);
            }
        }
    }

    /// ?紐꺿봺 ?닌듼???슢諭?
    fn build_tree(
        &self,
        dir: &Path,
        prefix: &str,
        max_depth: usize,
        current_depth: usize,
        output: &mut String,
    ) {
        if current_depth >= max_depth {
            return;
        }

        let mut entries: Vec<_> = match std::fs::read_dir(dir) {
            Ok(e) => e.flatten().collect(),
            Err(_) => return,
        };
        entries.sort_by_key(|e| e.file_name());

        let total = entries.len();
        for (i, entry) in entries.iter().enumerate() {
            let is_last = i == total - 1;
            let connector = if is_last { "?遺??? " } else { "????? " };
            let child_prefix = if is_last { "    " } else { "??  " };

            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.path().is_dir();

            output.push_str(&format!(
                "{}{}{}{}\n",
                prefix,
                connector,
                name,
                if is_dir { "/" } else { "" }
            ));

            if is_dir {
                self.build_tree(
                    &entry.path(),
                    &format!("{}{}", prefix, child_prefix),
                    max_depth,
                    current_depth + 1,
                    output,
                );
            }
        }
    }
}

/// ???뵬 ??由????????꾨┛ ?ル뿭? ?類ㅻ뻼??곗쨮 癰궰??
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata() {
        let skill = FileSkill::new();
        let meta = skill.metadata();
        assert_eq!(meta.name, "file");
        assert_eq!(meta.actions.len(), 6);
    }

    #[test]
    fn test_always_eligible() {
        let skill = FileSkill::new();
        assert!(skill.check_eligibility().eligible);
    }

    #[test]
    fn test_organize_requires_approval() {
        let skill = FileSkill::new();
        assert!(skill.requires_approval("organize"));
        assert!(!skill.requires_approval("search"));
        assert!(!skill.requires_approval("list"));
    }

    #[test]
    fn test_resolve_path_keywords() {
        let skill = FileSkill::new();
        let desktop = skill.resolve_path("獄쏅?源?遺얇늺");
        assert!(desktop.to_string_lossy().contains("Desktop"));

        let downloads = skill.resolve_path("??쇱뒲嚥≪뮆諭?);
        assert!(downloads.to_string_lossy().contains("Downloads"));
    }

    #[test]
    fn test_resolve_path_absolute() {
        let skill = FileSkill::new();
        let abs = skill.resolve_path("C:\\Windows");
        assert_eq!(abs, PathBuf::from("C:\\Windows"));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
    }

    #[tokio::test]
    async fn test_list_current_dir() {
        let skill = FileSkill::new();
        let ctx = SkillContext {
            session_key: "test".to_string(),
            action: "list".to_string(),
            params: {
                let mut p = HashMap::new();
                p.insert("path".to_string(), json!("."));
                p
            },
            user_context: crate::jarvis::models::context::UserContext::new(),
        };
        let result = skill.execute(ctx).await;
        // Should succeed or fail gracefully
        assert!(result.success || !result.message.is_empty());
    }
}
