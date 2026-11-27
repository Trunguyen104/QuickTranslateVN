use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::cleaner::looks_like_vietnamese;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictEntry {
    pub pos: String,
    pub meanings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub original_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub phonetic: Option<String>,
    pub dict: Vec<DictEntry>,
    pub elapsed_ms: u64,
    pub from_cache: bool,
}

pub struct Translator {
    agent: ureq::Agent,
    cache: Mutex<HashMap<String, TranslationResult>>,
}

impl Translator {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(8))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
            .build();

        Self {
            agent,
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn translate(&self, text: &str, force_target: Option<&str>) -> Result<TranslationResult, String> {
        let trimmed: String = text.trim().chars().take(4500).collect();
        if trimmed.is_empty() {
            return Err("Văn bản trống".to_string());
        }

        let target_lang = match force_target {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => {
                if looks_like_vietnamese(&trimmed) {
                    "en".to_string()
                } else {
                    "vi".to_string()
                }
            }
        };

        let cache_key = format!("{}::{}", target_lang, trimmed);
        if let Ok(guard) = self.cache.lock() {
            if let Some(cached) = guard.get(&cache_key) {
                let mut cloned = cached.clone();
                cloned.from_cache = true;
                cloned.elapsed_ms = 0;
                return Ok(cloned);
            }
        }

        let start = Instant::now();
        let mut res = self
            .query_google("gtx", &trimmed, &target_lang, force_target.is_none())
            .or_else(|_| self.query_google("dict-chrome-ex", &trimmed, &target_lang, force_target.is_none()))?;

        res.elapsed_ms = start.elapsed().as_millis() as u64;
        res.from_cache = false;

        if let Ok(mut guard) = self.cache.lock() {
            if guard.len() > 80 {
                guard.clear();
            }
            guard.insert(cache_key, res.clone());
        }

        Ok(res)
    }

    fn query_google(
        &self,
        client: &str,
        text: &str,
        target_lang: &str,
        allow_auto_flip: bool,
    ) -> Result<TranslationResult, String> {
        let url = format!(
            "https://translate.googleapis.com/translate_a/single?client={}&sl=auto&tl={}&hl=vi&dt=t&dt=bd&dt=rm&dj=1&q={}",
            client,
            target_lang,
            urlencoding::encode(text)
        );

        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| format!("Lỗi kết nối mạng: {}", e))?;

        let root: Value = resp
            .into_json()
            .map_err(|e| format!("Lỗi đọc dữ liệu dịch: {}", e))?;

        let mut translated = String::new();
        let mut phonetic: Option<String> = None;

        if let Some(sentences) = root.get("sentences").and_then(|v| v.as_array()) {
            for item in sentences {
                if let Some(t) = item.get("trans").and_then(|v| v.as_str()) {
                    translated.push_str(t);
                }
                if phonetic.is_none() {
                    if let Some(p) = item.get("src_translit").and_then(|v| v.as_str()) {
                        let p_trim = p.trim();
                        if !p_trim.is_empty() {
                            phonetic = Some(p_trim.to_string());
                        }
                    }
                }
            }
        }

        let detected_src = root
            .get("src")
            .and_then(|v| v.as_str())
            .unwrap_or("en")
            .to_string();

        if allow_auto_flip && detected_src.eq_ignore_ascii_case(target_lang) && target_lang == "vi" {
            return self.query_google(client, text, "en", false);
        }

        let mut dict = Vec::new();
        if let Some(dict_arr) = root.get("dict").and_then(|v| v.as_array()) {
            for entry in dict_arr.iter().take(3) {
                let pos = entry
                    .get("pos")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let mut meanings = Vec::new();
                if let Some(terms) = entry.get("terms").and_then(|v| v.as_array()) {
                    for t in terms.iter().take(5) {
                        if let Some(s) = t.as_str() {
                            let st = s.trim();
                            if !st.is_empty() {
                                meanings.push(st.to_string());
                            }
                        }
                    }
                }

                if !meanings.is_empty() {
                    dict.push(DictEntry { pos, meanings });
                }
            }
        }

        let final_trans = translated.trim().to_string();
        if final_trans.is_empty() {
            return Err("Không nhận được nội dung dịch".to_string());
        }

        Ok(TranslationResult {
            original_text: text.to_string(),
            translated_text: final_trans,
            source_lang: detected_src,
            target_lang: target_lang.to_string(),
            phonetic,
            dict,
            elapsed_ms: 0,
            from_cache: false,
        })
    }

    /// Fetches Google TTS MP3 and returns a `data:audio/mpeg;base64,...` URI
    /// so the native WebView on Windows, macOS, and Linux plays audio cleanly with zero codec dependencies!
    pub fn fetch_tts_data_uri(&self, text: &str, lang: &str) -> Result<String, String> {
        let snippet: String = text.trim().chars().take(200).collect();
        if snippet.is_empty() {
            return Err("Empty TTS text".to_string());
        }
        let tl = if lang.trim().is_empty() { "en" } else { lang.trim() };
        let url = format!(
            "https://translate.googleapis.com/translate_tts?ie=UTF-8&client=tw-ob&tl={}&q={}",
            urlencoding::encode(tl),
            urlencoding::encode(&snippet)
        );

        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| e.to_string())?;

        let mut bytes = Vec::with_capacity(16384);
        resp.into_reader()
            .take(512 * 1024)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;

        if bytes.is_empty() {
            return Err("Empty audio response".to_string());
        }

        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(format!("data:audio/mpeg;base64,{}", b64))
    }
}
