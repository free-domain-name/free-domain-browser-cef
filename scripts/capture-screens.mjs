// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Free Domain Name contributors

// Renders the browser on a virtual display and saves the pictures the site
// page shows. The pictures are of the real browser: the same build, the same
// chrome, the same pages a user sees.
//
//   scripts/capture-screens.mjs            # every picture
//   scripts/capture-screens.mjs page-dark  # one picture
//
// The script needs an X display and ImageMagick. On Linux:
//
//   Xvfb :99 -screen 0 1400x900x24 &
//   DISPLAY=:99 scripts/capture-screens.mjs

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { launchCef, attachToChrome, evaluate } from "../verification/lib/harness.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT = path.join(ROOT, "docs", "assets");
const DISPLAY = process.env.DISPLAY ?? ":99";
// The page the pictures of a real site show. It is the page of the project
// itself, so the shot does not depend on a third party.
const PROJECT_PAGE = "https://free-domain-name.github.io/free-domain-browser-cef/";

/**
 * The pictures the page shows.
 *
 * Every scene names the theme the browser wears, the page it opens and what
 * the script does before the picture is taken.
 */
const SCENES = [
  {
    name: "browser-dark",
    theme: "dark",
    what: "the start page of the browser, in the dark theme",
  },
  {
    name: "browser-light",
    theme: "light",
    what: "the start page of the browser, in the light theme",
  },
  {
    name: "page-dark",
    theme: "dark",
    url: PROJECT_PAGE,
    what: "a real page in a tab, with the icon of the site",
  },
  {
    name: "page-light",
    theme: "light",
    url: PROJECT_PAGE,
    what: "a real page in a tab, in the light theme",
  },
  {
    name: "settings-light",
    theme: "light",
    open: "#settings",
    what: "the settings in a tab of their own",
  },
  {
    name: "chat-dark",
    theme: "dark",
    open: "#chat",
    what: "the community chat beside the page",
  },
];

/** Waits for a moment, so the window is painted before it is taken. */
function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Takes the picture of the window.
 *
 * The window is drawn on the virtual screen, so the screen is taken and the
 * empty border is cut away afterwards.
 */
function capture(name) {
  const file = path.join(OUT, `${name}.png`);
  execFileSync("import", ["-display", DISPLAY, "-window", "root", file], { stdio: "ignore" });
  execFileSync("convert", [file, "-trim", "+repage", file], { stdio: "ignore" });
  console.log(`  ${name}.png`);
}

async function run(scene) {
  console.log(`${scene.name} — ${scene.what}`);
  const browser = await launchCef({
    url: scene.url,
    settings: { theme: scene.theme, density: "comfortable" },
  });
  try {
    const chrome = await attachToChrome(browser.port);
    await wait(3000);
    if (scene.open) {
      await evaluate(chrome, `document.querySelector(${JSON.stringify(scene.open)}).click(); return true;`);
      await wait(3000);
    }
    chrome.close();
    capture(scene.name);
  } finally {
    await browser.close();
  }
}

async function main() {
  if (!process.env.DISPLAY) {
    console.error("capture-screens: DISPLAY must point at an X display");
    process.exit(1);
  }
  if (!existsSync("/usr/bin/import")) {
    console.error("capture-screens: ImageMagick (import) is needed to take the pictures");
    process.exit(1);
  }
  mkdirSync(OUT, { recursive: true });
  const wanted = process.argv.slice(2);
  const scenes = wanted.length ? SCENES.filter((scene) => wanted.includes(scene.name)) : SCENES;
  if (!scenes.length) {
    console.error(`capture-screens: no scene of that name. Known: ${SCENES.map((s) => s.name).join(", ")}`);
    process.exit(1);
  }
  for (const scene of scenes) {
    await run(scene);
  }
  console.log(`the pictures are in ${OUT}`);
}

await main();
process.exit(0);
