#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Free Domain Name contributors
//
// The verification runner.
//
// It reads the journeys from the journeys folder, starts the real browser
// build, walks every journey and reports what passed and what did not.
//
//   node verification/run.mjs                 every journey
//   node verification/run.mjs first-launch    one journey
//   node verification/run.mjs --list          the names of the journeys
//
// The exit code is 0 when every step passed and 1 when anything failed.

import { readdir, readFile, mkdir, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import {
  launchCef,
  evaluate,
  click,
  waitForExpression,
  screenshot,
  listTargets,
  attachToPage,
  startSite,
} from "./lib/harness.mjs";
import { check } from "./lib/assert.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const journeysDir = path.join(here, "journeys");
const artifactsDir = path.join(here, "artifacts");

const args = process.argv.slice(2);
if (args.includes("--list")) {
  for (const file of await journeyFiles()) {
    const spec = JSON.parse(await readFile(file, "utf8"));
    console.log(`${spec.name.padEnd(22)} ${spec.description ?? ""}`);
  }
  process.exit(0);
}

const wanted = args.filter((arg) => !arg.startsWith("-"));
const results = [];
const skipped = [];

for (const file of await journeyFiles()) {
  const spec = JSON.parse(await readFile(file, "utf8"));
  if (wanted.length && !wanted.includes(spec.name)) continue;
  // A journey may only make sense on some systems.
  if (spec.platform && !spec.platform.includes(process.platform)) {
    skipped.push(`${spec.name} (only on ${spec.platform.join(", ")})`);
    continue;
  }

  console.log(`\n${spec.name} — ${spec.description ?? ""}`);
  // The site of the journeys: a real page with a title, a favicon, a popup
  // and a file to download. It is started per journey, so the journeys do
  // not share a state.
  const site = await startSite();
  const harness = await launchCef({ url: spec.startUrl, env: spec.env });
  try {
    for (const step of spec.steps) {
      const description = step.why ?? step.do;
      try {
        await runStep(harness, step, site);
        results.push({ journey: spec.name, step: description, ok: true });
        console.log(`  ok    ${description}`);
      } catch (error) {
        results.push({ journey: spec.name, step: description, ok: false, error: error.message });
        console.log(`  FAIL  ${description}`);
        console.log(`        ${error.message}`);
        const shot = await screenshot(harness.client).catch(() => null);
        if (shot) {
          await mkdir(artifactsDir, { recursive: true });
          const file_name = `${spec.name}-${results.length}.png`;
          await writeFile(path.join(artifactsDir, file_name), shot);
          console.log(`        screenshot: verification/artifacts/${file_name}`);
        }
        // What the browser itself said: the reason a step failed is often in
        // the log of the process, not in the page.
        const said = (harness.log ?? []).join("").trim();
        if (said) {
          console.log(`        the browser said:\n${said.slice(-1500)}`);
        }
        break;
      }
    }
  } finally {
    await harness.close();
    await site.close();
  }
}

async function journeyFiles() {
  const names = await readdir(journeysDir);
  // Hidden files are skipped: macOS keeps a resource fork beside a file as a
  // `._name` sidecar, and that sidecar is not a journey.
  return names
    .filter((name) => name.endsWith(".json") && !name.startsWith("."))
    .sort()
    .map((name) => path.join(journeysDir, name));
}

/** Runs one step of a journey. */
async function runStep(harness, step, site) {
  // A journey may point at the site of the run: the port changes with every
  // run, so the address cannot be written down.
  const fill = (text) =>
    String(text ?? "").replaceAll("{{site}}", site?.origin ?? "");
  switch (step.do) {
    case "evaluate": {
      const value = await evaluate(harness.client, fill(step.expression));
      if (step.expect) check(value, step.expect, step.why ?? "the value");
      return value;
    }
    case "click": {
      await click(harness.client, step.selector);
      if (step.expect) {
        const value = await evaluate(harness.client, fill(step.expression ?? "return true;"));
        check(value, step.expect, step.why ?? "the click");
      }
      return true;
    }
    case "waitFor": {
      await waitForExpression(harness.client, fill(step.expression), step.timeout ?? 10000);
      if (step.expect) {
        const value = await evaluate(harness.client, fill(step.expression));
        check(value, step.expect, step.why ?? "the wait");
      }
      return true;
    }
    case "waitForSelector": {
      await waitForExpression(
        harness.client,
        `!!document.querySelector(${JSON.stringify(step.selector)})`,
        step.timeout ?? 10000,
      );
      return true;
    }
    case "type": {
      const typed = await evaluate(
        harness.client,
        `const element = document.querySelector(${JSON.stringify(step.selector)});
         if (!element) return false;
         element.focus();
         element.value = ${JSON.stringify(fill(step.text ?? ""))};
         element.dispatchEvent(new Event("input", { bubbles: true }));
         ${step.press === false ? "" : 'element.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));'}
         return true;`,
      );
      if (!typed) throw new Error(`there is no element for ${step.selector}`);
      return true;
    }
    case "inPage": {
      // Runs an expression inside another page of the same window: the
      // settings, the menu, the conflict page.
      const client = await attachToPage(harness.port, step.title);
      try {
        const value = await evaluate(client, fill(step.expression));
        if (step.expect) check(value, step.expect, step.why ?? `the ${step.title} page`);
        return value;
      } finally {
        client.close();
      }
    }
    case "waitForInPage": {
      // Waits for an expression in another page of the same window: the
      // chat reaching its connected state, for example.
      const client = await attachToPage(harness.port, step.title);
      // The expression may be written with or without a leading `return`, so
      // it is reduced to a value and handed back to the page.
      const expression = step.expression.trim().replace(/^return\s+/, "").replace(/;$/, "");
      try {
        const deadline = Date.now() + (step.timeout ?? 10000);
        let last = null;
        let ready = false;
        while (Date.now() < deadline) {
          try {
            last = await evaluate(client, `return (${expression});`);
            if (last) {
              ready = true;
              break;
            }
          } catch (error) {
            last = error.message;
          }
          await new Promise((resolve) => setTimeout(resolve, 200));
        }
        if (!ready) {
          throw new Error(
            `"${step.expression}" was still false in the ${step.title} page after ` +
              `${step.timeout ?? 10000} ms (last: ${last})`,
          );
        }
        if (step.expect) check(last, step.expect, step.why ?? `the ${step.title} page`);
        return last;
      } finally {
        client.close();
      }
    }
    case "key": {
      // A key press inside the window, the way the chrome listens for it.
      // The modifiers are named, because the shortcuts need them.
      await evaluate(
        harness.client,
        `document.dispatchEvent(new KeyboardEvent("keydown", {
           key: ${JSON.stringify(step.key)},
           code: ${JSON.stringify(step.code ?? step.key)},
           keyCode: ${step.keyCode ?? 0},
           metaKey: ${step.metaKey === true},
           ctrlKey: ${step.ctrlKey === true},
           altKey: ${step.altKey === true},
           shiftKey: ${step.shiftKey === true},
           bubbles: true,
           cancelable: true,
         }));
         return true;`,
      );
      return true;
    }
    case "pause": {
      await new Promise((resolve) => setTimeout(resolve, step.ms ?? 300));
      return true;
    }
    case "targets": {
      // A page takes a moment to open, so the check waits for it.
      const deadline = Date.now() + (step.timeout ?? 5000);
      let list = "";
      let problem = null;
      while (Date.now() < deadline) {
        list = await listTargets(harness.port);
        try {
          if (step.expect) check(list, step.expect, step.why ?? "the open pages");
          problem = null;
          break;
        } catch (error) {
          problem = error;
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
      if (problem) throw problem;
      return list;
    }
    case "clickInPage": {
      // Clicks inside another page: a link of the site the browser shows.
      const client = await attachToPage(harness.port, step.title);
      try {
        await click(client, step.selector);
        if (step.expect) {
          const value = await evaluate(client, fill(step.expression ?? "return true;"));
          check(value, step.expect, step.why ?? `the click in ${step.title}`);
        }
      } finally {
        client.close();
      }
      return true;
    }
    case "clickRealInPage": {
      // A real mouse click in another page. A scripted click has no user
      // gesture, so a browser refuses to open a popup for it.
      const client = await attachToPage(harness.port, step.title);
      try {
        const point = await evaluate(
          client,
          `const element = document.querySelector(${JSON.stringify(step.selector)});
           if (!element) return null;
           element.scrollIntoView({ block: "center" });
           const box = element.getBoundingClientRect();
           return { x: box.left + box.width / 2, y: box.top + box.height / 2 };`,
        );
        if (!point) throw new Error(`there is no element for ${step.selector}`);
        await client.send("Input.dispatchMouseEvent", {
          type: "mousePressed",
          x: point.x,
          y: point.y,
          button: "left",
          clickCount: 1,
        });
        await client.send("Input.dispatchMouseEvent", {
          type: "mouseReleased",
          x: point.x,
          y: point.y,
          button: "left",
          clickCount: 1,
        });
        if (step.expect) {
          const value = await evaluate(client, fill(step.expression ?? "return true;"));
          check(value, step.expect, step.why ?? `the click in ${step.title}`);
        }
      } finally {
        client.close();
      }
      return true;
    }
    case "keyInPage": {
      // A real key, sent to a page the way a keyboard sends it: the content
      // handler listens for keys, not for scripted events.
      const client = await attachToPage(harness.port, step.title);
      try {
        await client.send("Input.dispatchKeyEvent", {
          type: step.type ?? "keyDown",
          key: step.key,
          code: step.code ?? step.key,
          text: step.text,
          windowsVirtualKeyCode: step.keyCode ?? 0,
        });
        if (step.up !== false) {
          await client.send("Input.dispatchKeyEvent", {
            type: "keyUp",
            key: step.key,
            code: step.code ?? step.key,
            windowsVirtualKeyCode: step.keyCode ?? 0,
          });
        }
        if (step.expect) {
          const value = await evaluate(client, fill(step.expression));
          check(value, step.expect, step.why ?? `the key in ${step.title}`);
        }
      } finally {
        client.close();
      }
      return true;
    }
    case "logContains": {
      // What the browser itself said: some things are only visible in the
      // log of the process, like the place a download went to.
      const deadline = Date.now() + (step.timeout ?? 10000);
      let said = "";
      while (Date.now() < deadline) {
        said = (harness.log ?? []).join("");
        if (said.includes(step.text)) break;
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
      if (!said.includes(step.text)) {
        throw new Error(
          `the browser did not say "${step.text}" within ${step.timeout ?? 10000} ms`,
        );
      }
      if (step.expect) check(said, step.expect, step.why ?? "the log of the browser");
      return true;
    }
    case "screenshot": {
      await mkdir(artifactsDir, { recursive: true });
      const name = step.name ?? "shot";
      await writeFile(path.join(artifactsDir, `${name}.png`), await screenshot(harness.client));
      return true;
    }
    default:
      throw new Error(`unknown step "${step.do}"`);
  }
}

const failed = results.filter((result) => !result.ok);
for (const name of skipped) console.log(`skipped ${name}`);
console.log(
  `\n${results.length - failed.length} of ${results.length} steps passed` +
    (failed.length ? `, ${failed.length} failed` : ""),
);
process.exit(failed.length ? 1 : 0);
