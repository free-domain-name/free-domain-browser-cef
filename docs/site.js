// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Free Domain Name contributors

// The copy buttons of the install commands. The markup and the styling come
// from the project home page, so both pages look the same.
(() => {
  async function copyText(text) {
    if (navigator.clipboard && window.isSecureContext) {
      try {
        await navigator.clipboard.writeText(text);
        return true;
      } catch {
        // Fall through to the execCommand path.
      }
    }
    return copyWithExecCommand(text);
  }

  function copyWithExecCommand(text) {
    try {
      const area = document.createElement("textarea");
      area.value = text;
      area.setAttribute("readonly", "");
      area.style.position = "fixed";
      area.style.top = "0";
      area.style.left = "-9999px";
      document.body.append(area);
      area.select();
      const copied = document.execCommand("copy");
      area.remove();
      return copied;
    } catch {
      return false;
    }
  }

  function selectText(node) {
    const range = document.createRange();
    range.selectNodeContents(node);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  }

  function flashCopy(button, label) {
    const previous = button.textContent;
    button.textContent = label;
    button.classList.add("is-copied");
    setTimeout(() => {
      button.textContent = previous;
      button.classList.remove("is-copied");
    }, 1600);
  }

  for (const button of document.querySelectorAll(".copy-button")) {
    const selector = button.dataset.copy;
    if (!selector) continue;

    button.addEventListener("click", async () => {
      const target = document.querySelector(selector);
      if (!target) return;

      const text = (target.textContent || "").trim();
      const copied = await copyText(text);
      if (!copied) selectText(target);
      flashCopy(button, copied ? "Copied" : "Selected");
    });
  }
})();

// The theme toggle of the project home page, so both pages share the key and
// the choice of the visitor.
(() => {
  const THEME_KEY = "fdn.docs.theme";
  const toggle = document.querySelector("#themeToggle");
  if (!toggle) return;

  function applyTheme(theme) {
    document.documentElement.dataset.theme = theme;
    const next = theme === "dark" ? "light" : "dark";
    toggle.setAttribute("aria-label", `Switch to ${next} theme`);
    toggle.title = `Switch to ${next} theme`;
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.content = theme === "dark" ? "#080b12" : "#f4f6fa";
  }

  let saved = null;
  try {
    saved = localStorage.getItem(THEME_KEY);
  } catch {
    // Storage can be disabled; the default theme still works.
  }
  const preferred =
    saved === "light" || saved === "dark"
      ? saved
      : window.matchMedia("(prefers-color-scheme: light)").matches
        ? "light"
        : "dark";
  applyTheme(preferred);

  toggle.addEventListener("click", () => {
    const next = document.documentElement.dataset.theme === "dark" ? "light" : "dark";
    applyTheme(next);
    try {
      localStorage.setItem(THEME_KEY, next);
    } catch {
      // Ignore storage failures.
    }
  });
})();
