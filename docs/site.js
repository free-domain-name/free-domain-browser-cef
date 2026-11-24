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
    // The pictures of the browser follow the theme of the page: the light
    // picture shows the light theme of the browser, the dark one the dark.
    for (const shot of document.querySelectorAll(".theme-shot")) {
      const picture = shot.dataset[theme];
      if (picture && shot.getAttribute("src") !== picture) shot.setAttribute("src", picture);
      const alt = shot.dataset[theme === "light" ? "altLight" : "altDark"];
      if (alt) shot.setAttribute("alt", alt);
    }
    for (const label of document.querySelectorAll("[data-theme-label]")) {
      label.textContent = theme === "dark" ? "Dark" : "Light";
    }
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

// The full size picture of the browser opens in a dialog of its own.
(() => {
  const zoom = document.querySelector("#heroZoom");
  if (!zoom) return;

  const dialog = document.createElement("dialog");
  dialog.className = "lightbox";
  const image = document.createElement("img");
  const caption = document.createElement("figcaption");
  const label = document.createElement("span");
  const close = document.createElement("button");
  close.type = "button";
  close.textContent = "Close";
  caption.append(label, close);
  dialog.append(image, caption);
  document.body.append(dialog);

  const theme = () => document.documentElement.dataset.theme === "light" ? "light" : "dark";

  zoom.addEventListener("click", () => {
    const picture = zoom.dataset[theme()];
    if (!picture) return;
    image.setAttribute("src", picture);
    image.setAttribute("alt", zoom.dataset.label || "");
    label.textContent = zoom.dataset.label || "";
    if (typeof dialog.showModal === "function") dialog.showModal();
    else dialog.setAttribute("open", "");
  });

  close.addEventListener("click", () => dialog.close());
  dialog.addEventListener("click", (event) => {
    if (event.target === dialog) dialog.close();
  });
})();
