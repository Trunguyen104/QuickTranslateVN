(function () {
  function detectRepoInfo() {
    const host = window.location.hostname;
    const pathParts = window.location.pathname.split("/").filter(Boolean);
    if (host.endsWith(".github.io") && pathParts.length >= 1) {
      const owner = host.replace(".github.io", "");
      const repo = pathParts[0];
      return {
        owner,
        repo,
        repoUrl: `https://github.com/${owner}/${repo}`,
        releasesUrl: `https://github.com/${owner}/${repo}/releases/latest`
      };
    }
    return {
      owner: "Trunguyen104",
      repo: "QuickTranslateVN",
      repoUrl: "https://github.com/Trunguyen104/QuickTranslateVN",
      releasesUrl: "https://github.com/Trunguyen104/QuickTranslateVN/releases/latest"
    };
  }

  const repoInfo = detectRepoInfo();
  document.getElementById("navGithubLink").href = repoInfo.repoUrl;
  document.getElementById("heroGithubBtn").href = repoInfo.repoUrl;
  document.getElementById("downloadBtn").href = repoInfo.releasesUrl;
  document.getElementById("releaseCardBtn").href = repoInfo.releasesUrl;
  document.getElementById("footerRepoLink").href = repoInfo.repoUrl;
  document.getElementById("footerIssuesLink").href = `${repoInfo.repoUrl}/issues`;

  const buildCmdEl = document.getElementById("buildCmdText");
  if (buildCmdEl) {
    buildCmdEl.textContent = buildCmdEl.textContent.replace(
      "https://github.com/YOUR_USERNAME/QuickTranslateVN.git",
      `${repoInfo.repoUrl}.git`
    );
  }

  const copyCmdBtn = document.getElementById("copyCmdBtn");
  copyCmdBtn?.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(buildCmdEl.textContent);
      copyCmdBtn.textContent = "✓ Copied";
      setTimeout(() => (copyCmdBtn.textContent = "Copy"), 1300);
    } catch {}
  });

  const PRESET_DB = {
    "system-wide instant translation works seamlessly across word, excel, pdf readers, and code editors without stealing keyboard focus from your active window.": {
      src: "en",
      tgt: "vi",
      trans: "Dịch tức thì trên toàn hệ thống hoạt động mượt mà trên Word, Excel, trình đọc PDF và trình sửa code mà không chiếm mất quyền điều khiển bàn phím của cửa sổ đang mở.",
      ms: 118
    },
    "resilient": {
      src: "en",
      tgt: "vi",
      trans: "bền bỉ, có khả năng phục hồi nhanh",
      phonetic: "rɪˈzɪliənt",
      dict: [
        { pos: "tính từ", terms: ["bền bỉ", "kiên cường", "có khả năng tự phục hồi", "đàn hồi"] }
      ],
      ms: 94
    },
    "chào bạn, tôi xin gửi báo cáo tiến độ dự án tuần này để đội ngũ xem xét.": {
      src: "vi",
      tgt: "en",
      trans: "Hi there, I would like to share this week's project progress report for the team's review.",
      ms: 132
    }
  };

  const stage = document.getElementById("desktopStage");
  const selectableArea = document.getElementById("selectableArea");
  const simBadge = document.getElementById("simBadge");
  const simPopup = document.getElementById("simPopup");
  const simSwapBtn = document.getElementById("simSwapBtn");
  const simLatency = document.getElementById("simLatency");
  const simPhonetic = document.getElementById("simPhonetic");
  const simTranslated = document.getElementById("simTranslated");
  const simDictDivider = document.getElementById("simDictDivider");
  const simDict = document.getElementById("simDict");
  const simSpeakBtn = document.getElementById("simSpeakBtn");
  const simReplaceBtn = document.getElementById("simReplaceBtn");
  const simCopyBtn = document.getElementById("simCopyBtn");
  const svgCopyNormal = document.getElementById("svgCopyNormal");
  const svgCopyDone = document.getElementById("svgCopyDone");
  const simPinBtn = document.getElementById("simPinBtn");
  const simCloseBtn = document.getElementById("simCloseBtn");
  const sampleInput2 = document.getElementById("sampleInput2");

  let pendingText = "";
  let pendingAnchor = { x: 180, y: 120 };
  let activeSourceText = "";
  let activeTranslatedText = "";
  let activeSrcLang = "en";
  let activeTgtLang = "vi";
  let isPinned = false;
  let activeTextareaSelection = null;

  function looksVietnamese(str) {
    return /[ăâđêôơưáàảãạấầẩẫậắằẳẵặéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ]/i.test(str);
  }

  function clampInsideStage(x, y, boxW, boxH) {
    const rect = stage.getBoundingClientRect();
    const maxX = Math.max(12, rect.width - boxW - 12);
    const maxY = Math.max(12, rect.height - boxH - 12);
    return {
      x: Math.min(Math.max(12, x), maxX),
      y: Math.min(Math.max(12, y), maxY)
    };
  }

  function showBadgeAt(stageX, stageY, selectedStr) {
    pendingText = selectedStr;
    const pos = clampInsideStage(stageX + 8, stageY + 10, 32, 32);
    pendingAnchor = pos;
    simBadge.style.left = `${pos.x}px`;
    simBadge.style.top = `${pos.y}px`;
    simBadge.classList.remove("hidden");
  }

  function hideBadge() {
    simBadge.classList.add("hidden");
  }

  async function translateText(rawText, forceTarget) {
    const key = rawText.trim().toLowerCase();
    if (!forceTarget && PRESET_DB[key]) {
      return PRESET_DB[key];
    }

    const target = forceTarget || (looksVietnamese(rawText) ? "en" : "vi");
    const t0 = performance.now();
    try {
      const url = `https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=${target}&hl=vi&dt=t&dt=bd&dt=rm&dj=1&q=${encodeURIComponent(rawText)}`;
      const res = await fetch(url);
      const data = await res.json();
      let trans = "";
      let phonetic = "";
      if (Array.isArray(data.sentences)) {
        for (const s of data.sentences) {
          if (s.trans) trans += s.trans;
          if (!phonetic && s.src_translit) phonetic = s.src_translit;
        }
      }
      const dict = [];
      if (Array.isArray(data.dict)) {
        for (const d of data.dict.slice(0, 3)) {
          if (Array.isArray(d.terms)) {
            dict.push({ pos: d.pos || "", terms: d.terms.slice(0, 5) });
          }
        }
      }
      return {
        src: data.src || (target === "vi" ? "en" : "vi"),
        tgt: target,
        trans: trans || rawText,
        phonetic,
        dict,
        ms: Math.round(performance.now() - t0)
      };
    } catch {
      return {
        src: looksVietnamese(rawText) ? "vi" : "en",
        tgt: target,
        trans: target === "vi"
          ? "[Bản dịch mô phỏng] Công cụ dịch tức thì hoạt động mượt mà trên toàn hệ thống Windows."
          : "[Simulated translation] Instant translation works seamlessly across Windows.",
        ms: 110
      };
    }
  }

  async function openPopupWithText(text, anchorX, anchorY, forceTarget = null, keepPos = false) {
    if (!text || !text.trim()) return;
    hideBadge();
    activeSourceText = text.trim();

    simTranslated.textContent = "Đang dịch...";
    simPhonetic.classList.add("hidden");
    simDictDivider.classList.add("hidden");
    simDict.classList.add("hidden");
    simDict.innerHTML = "";
    svgCopyNormal.classList.remove("hidden");
    svgCopyDone.classList.add("hidden");
    simPopup.classList.remove("hidden");

    if (!keepPos) {
      const pos = clampInsideStage(anchorX, anchorY + 8, 368, 145);
      simPopup.style.left = `${pos.x}px`;
      simPopup.style.top = `${pos.y}px`;
    }

    const res = await translateText(activeSourceText, forceTarget);
    activeTranslatedText = res.trans;
    activeSrcLang = res.src;
    activeTgtLang = res.tgt;

    simSwapBtn.textContent = `${res.src.toUpperCase()} → ${res.tgt.toUpperCase()}  ⇄`;
    simLatency.textContent = `${res.ms}ms`;
    simTranslated.textContent = res.trans;

    if (res.phonetic) {
      simPhonetic.textContent = `/${res.phonetic}/`;
      simPhonetic.classList.remove("hidden");
    }

    if (res.dict && res.dict.length > 0) {
      simDict.innerHTML = res.dict
        .map(
          (d) =>
            `<div class="sim-dict-row">${
              d.pos ? `<span class="sim-pos-badge">${d.pos}</span>` : ""
            }<span>${d.terms.join(", ")}</span></div>`
        )
        .join("");
      simDictDivider.classList.remove("hidden");
      simDict.classList.remove("hidden");
    }
  }

  selectableArea.addEventListener("mouseup", (e) => {
    setTimeout(() => {
      let selected = "";
      if (document.activeElement === sampleInput2) {
        const start = sampleInput2.selectionStart;
        const end = sampleInput2.selectionEnd;
        if (end > start) {
          selected = sampleInput2.value.substring(start, end).trim();
          activeTextareaSelection = { start, end };
        }
      } else {
        activeTextareaSelection = null;
        selected = window.getSelection()?.toString().trim() || "";
      }

      if (selected.length > 0) {
        const stageRect = stage.getBoundingClientRect();
        showBadgeAt(e.clientX - stageRect.left, e.clientY - stageRect.top, selected);
      }
    }, 20);
  });

  simBadge.addEventListener("click", (e) => {
    e.stopPropagation();
    openPopupWithText(pendingText, pendingAnchor.x, pendingAnchor.y);
  });

  document.addEventListener("keydown", (e) => {
    if (e.altKey && e.key.toLowerCase() === "q") {
      const sel =
        document.activeElement === sampleInput2
          ? sampleInput2.value.substring(sampleInput2.selectionStart, sampleInput2.selectionEnd).trim()
          : window.getSelection()?.toString().trim() || pendingText;
      if (sel) {
        e.preventDefault();
        openPopupWithText(sel, pendingAnchor.x, pendingAnchor.y);
      }
    } else if (e.key === "Escape" && !isPinned) {
      hideBadge();
      simPopup.classList.add("hidden");
    }
  });

  document.addEventListener("mousedown", (e) => {
    if (!simBadge.contains(e.target)) hideBadge();
    if (!isPinned && !simPopup.contains(e.target) && !simBadge.contains(e.target) && !e.target.classList.contains("chip-btn")) {
      simPopup.classList.add("hidden");
    }
  });

  simSwapBtn.addEventListener("click", () => {
    const next = activeTgtLang === "vi" ? "en" : "vi";
    openPopupWithText(activeSourceText, 0, 0, next, true);
  });

  simSpeakBtn.addEventListener("click", () => {
    if ("speechSynthesis" in window && activeSourceText) {
      window.speechSynthesis.cancel();
      const utter = new SpeechSynthesisUtterance(activeSourceText);
      utter.lang = activeSrcLang === "vi" ? "vi-VN" : "en-US";
      window.speechSynthesis.speak(utter);
    }
  });

  simCopyBtn.addEventListener("click", async () => {
    if (!activeTranslatedText) return;
    try {
      await navigator.clipboard.writeText(activeTranslatedText);
      svgCopyNormal.classList.add("hidden");
      svgCopyDone.classList.remove("hidden");
      setTimeout(() => {
        svgCopyDone.classList.add("hidden");
        svgCopyNormal.classList.remove("hidden");
      }, 1000);
    } catch {}
  });

  simReplaceBtn.addEventListener("click", () => {
    if (!activeTranslatedText) return;
    if (activeTextareaSelection) {
      const v = sampleInput2.value;
      sampleInput2.value =
        v.slice(0, activeTextareaSelection.start) +
        activeTranslatedText +
        v.slice(activeTextareaSelection.end);
      activeTextareaSelection = null;
    } else {
      sampleInput2.value = activeTranslatedText;
    }
    if (!isPinned) simPopup.classList.add("hidden");
  });

  simPinBtn.addEventListener("click", () => {
    isPinned = !isPinned;
    simPinBtn.classList.toggle("pinned", isPinned);
  });

  simCloseBtn.addEventListener("click", () => {
    simPopup.classList.add("hidden");
  });

  document.querySelectorAll(".chip-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      const idx = btn.getAttribute("data-sample");
      const stageRect = stage.getBoundingClientRect();
      if (idx === "0") {
        const el = document.getElementById("samplePara0");
        const r = el.getBoundingClientRect();
        activeTextareaSelection = null;
        openPopupWithText(el.textContent.trim(), r.left - stageRect.left + 24, r.bottom - stageRect.top);
      } else if (idx === "1") {
        const el = document.getElementById("wordResilient");
        const r = el.getBoundingClientRect();
        activeTextareaSelection = null;
        openPopupWithText("resilient", r.left - stageRect.left, r.bottom - stageRect.top);
      } else if (idx === "2") {
        sampleInput2.focus();
        sampleInput2.select();
        activeTextareaSelection = { start: 0, end: sampleInput2.value.length };
        const r = sampleInput2.getBoundingClientRect();
        openPopupWithText(sampleInput2.value, r.left - stageRect.left + 30, r.top - stageRect.top - 130);
      }
    });
  });

  window.addEventListener("load", () => {
    const el = document.getElementById("wordResilient");
    const stageRect = stage.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    openPopupWithText("resilient", r.left - stageRect.left, r.bottom - stageRect.top);
  });

  const I18N = {
    en: {
      nav_demo: "Live Demo",
      nav_features: "Features",
      nav_architecture: "Under the Hood",
      nav_privacy: "Privacy",
      hero_tag: "100% Native Rust 🦀 • Supports Windows, macOS & Linux",
      hero_title: "Instant translation across every operating system.<br /><span class='gradient-text'>Minimalist. Ultra-light. Zero Focus Steal.</span>",
      hero_desc: "Brings the clean, effortless feel of Google Translate Extension and DeepL Desktop to Windows, macOS, and Linux: Word, Excel, PDFs, VS Code, and browsers. Native Rust binary — zero runtime bloat, tiny RAM.",
      cta_download: "Download Releases (Win / Mac / Linux)",
      cta_source: "View Rust Source & Build Guide",
      stat_size: "Win • macOS • Linux Native",
      stat_ram: "Idle background RAM",
      stat_deps: "Extra runtimes / API keys",
      stat_speed: "Translation response time",
      demo_heading: "Try It Live Right Here",
      demo_sub: "Drag your mouse to <strong>highlight any word or sentence</strong> inside the document below (or click a preset) to test the clean white <code>[文A]</code> badge and Popup.",
      demo_try: "Quick presets:",
      sample_btn_1: "English PDF sentence",
      sample_btn_2: "Dictionary lookup \"resilient\"",
      sample_btn_3: "Translate Vietnamese & Replace",
      editor_label: "Try In-Place Text Replacement inside an active editor:",
      feat_heading: "Pure Minimalism. Friendly From Second One.",
      feat_sub: "No confusing configuration dialogs. Lives quietly in your System Tray and appears right when needed.",
      f1_title: "Smart [文A] Floating Badge",
      f1_desc: "Detects Windows' native <code>IDC_IBEAM</code> text cursor: only appears when highlighting real text — never gets in your way when dragging files or scrollbars.",
      f2_title: "Alt + Q & Double Ctrl + C",
      f2_desc: "Prefer keyboard speed? Highlight text and hit <code>Alt + Q</code> or double-tap <code>Ctrl + C + C</code> (DeepL style) for instant popup translation.",
      f3_title: "In-Place Text Replacement",
      f3_desc: "Writing in Slack, Gmail, Word, or VS Code? Select your draft sentence, translate it, and click Replace to insert it right at your cursor.",
      f4_title: "Dictionary, Phonetics & Auto-Flip",
      f4_desc: "Automatically detects EN ↔ VI direction. Selecting single words displays IPA phonetics, part-of-speech tags, and audio pronunciation.",
      f5_title: "Smart PDF Line-Break Repair",
      f5_desc: "Automatically strips broken line-wraps and end-of-line hyphens copied from academic PDFs before translating.",
      f6_title: "Never Steals Window Focus",
      f6_desc: "Built with Win32 <code>WS_EX_NOACTIVATE</code> and <code>PerMonitorV2</code> DPI scaling: your active app keeps focus so your workflow stays uninterrupted.",
      arch_eyebrow: "ENGINEERING ARCHITECTURE",
      arch_title: "Captures text anywhere without polluting your Clipboard",
      arch_desc: "Windows apps render text in different ways. QuickTranslateVN runs a 3-stage <strong>Fallback Cascade</strong> on a single background STA thread:",
      arch_step1: "<strong>Stage 1 — Windows UI Automation:</strong> Reads directly via <code>TextPattern</code> (Word, Notepad, WPF) and <code>ValuePattern</code> (Excel cells) in &lt;15ms with zero clipboard contact.",
      arch_step2: "<strong>Stage 2 — Terminal Safety Guard:</strong> Detects <code>cmd</code>, <code>PowerShell</code>, and <code>Windows Terminal</code> so it never accidentally sends <code>SIGINT</code> via <code>Ctrl+C</code>.",
      arch_step3: "<strong>Stage 3 — Safe Clipboard Restoration:</strong> For browsers &amp; VS Code, backs up existing clipboard formats and restores them in ~25ms with <code>ExcludeClipboardContentFromMonitorProcessing</code> so <code>Win + V</code> stays clean.",
      cmp_title: "Resource Footprint Comparison",
      cmp_col_tool: "Tool",
      cmp_col_size: "Binary Size",
      cmp_col_ram: "Idle RAM",
      cmp_col_oss: "License",
      priv_title: "100% Transparent Privacy Guarantee",
      priv_desc: "<strong>Zero Telemetry • Zero History Logging • No Background Keylogging.</strong> Text is read strictly when you trigger translation and sent directly over HTTPS with no middleman servers.",
      inst_title: "Up and Running in 10 Seconds",
      inst_sub: "Grab the prebuilt native binary for your OS or compile directly with Cargo.",
      inst_c1_title: "Download Prebuilt Binaries (Win / Mac / Linux)",
      inst_c1_desc: "Download the native package for <strong>Windows (.exe)</strong>, <strong>macOS (.tar.gz)</strong>, or <strong>Linux (.tar.gz)</strong> from GitHub Releases — zero extra runtimes required.",
      inst_c1_btn: "Open GitHub Releases",
      inst_c2_title: "Build from Source (Cargo)",
      inst_c2_desc: "Requires Rust toolchain (<code>rustup</code>). Run on any platform:",
      footer_note: "Free & open-source software under the MIT License."
    }
  };

  const viSnapshot = {};
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    viSnapshot[el.getAttribute("data-i18n")] = el.innerHTML;
  });

  let currentLang = "vi";
  document.getElementById("langToggle")?.addEventListener("click", () => {
    currentLang = currentLang === "vi" ? "en" : "vi";
    document.documentElement.lang = currentLang;
    const dict = currentLang === "en" ? I18N.en : viSnapshot;
    document.querySelectorAll("[data-i18n]").forEach((el) => {
      const k = el.getAttribute("data-i18n");
      if (dict[k]) el.innerHTML = dict[k];
    });
  });
})();
