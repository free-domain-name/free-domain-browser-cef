# Verification

This folder checks what a user actually sees. It starts the real browser
build, drives the running window and looks at the result. There is no mock
browser, no fake network and no snapshot of what the code hoped would
happen: the checks run against the application binary with the real
Chromium, the real Rust and the real network.

The whole suite takes about half a minute.

## Running it

```sh
scripts/verify.sh                # every journey
scripts/verify.sh first-launch   # one journey
node verification/run.mjs --list # the names of the journeys
```

The runner needs a build of the browser. `scripts/verify.sh` makes one if it
is missing; `scripts/build.sh` builds it on its own.

`FDB_CEF_BINARY` overrides the path of the build it starts. The run uses its
own profile and its own settings file, so it never touches the browser you
are using and can start next to it.

On a machine without a screen, set `FDB_VERIFY_HIDDEN=1`. The browser then
starts hidden, the way the smoke test does, and the pages can still be read.
That is how the journeys run in CI.

## The journeys

A journey is a JSON file in `journeys/`. It is the written-down version of
what the browser must do, and the runner checks exactly that.

```json
{
  "name": "first-launch",
  "description": "The browser opens on its own start page, never on a search engine.",
  "steps": [
    { "do": "evaluate", "expression": "return document.getElementById('url').value;",
      "expect": { "equals": "" }, "why": "the address bar starts empty" }
  ]
}
```

Every step says what it does and why it matters. When a step fails the
runner prints the reason and writes a screenshot of the window to
`verification/artifacts/`.

The steps are:

| step | what it does |
| --- | --- |
| `evaluate` | runs an expression in the window and checks the value |
| `click` | presses a button |
| `type` | types into a field and presses Enter |
| `key` | presses a key inside the window |
| `waitFor` | waits until an expression is true |
| `waitForSelector` | waits until an element exists |
| `waitForInPage` | waits until an expression is true in another page of the same window |
| `targets` | waits until a page with the expected title is open |
| `inPage` | runs an expression in another page of the same window |
| `screenshot` | saves a picture of the window |
| `pause` | waits a moment |

The expectations are `equals`, `notEquals`, `contains`, `notContains`,
`matches`, `truthy`, `falsy`, `above` and `atLeast`.

A journey may carry a `platform` list. It is then only run on those systems
and reported as skipped elsewhere.

## Why it exists

The unit tests check the parts. They cannot tell you that the browser opens
on a search engine, that the settings button does nothing, or that the home
button takes you somewhere else. Those are the bugs that reach a user, and
they are the ones a journey catches.

Add a journey when a behaviour matters. A journey that fails is a bug that
would have reached someone.

## What is covered today

| journey | what it holds down |
| --- | --- |
| `first-launch` | the browser starts on its own start page, not on a search engine |
| `chrome-controls` | the settings, chat and home buttons do what they say |
| `navigation` | a name typed into the address bar resolves and loads |
| `registry` | a name nobody registered is looked up in the registry and in public DNS, and the browser offers to claim it |
| `conflict` | a name that is in both the registry and public DNS opens the chooser, and the choice is remembered |
| `search` | words without a dot are searched, not treated as a name |
| `settings` | a change in the settings tab reaches the browser |
| `appearance` | the light and the dark theme show the same chrome: the selected tab and the settings button read in both |
| `settings-tab` | the settings get a tab of their own and leave the other tabs alone |
| `tabs` | a tab can be opened and closed again |
| `chat` | the chat panel opens, asks for the 18+ confirmation and connects to the room |
| `menu` | the browser menu window opens and its entries work (Linux, Windows) |
| `menu-macos` | the entries of the native macOS menu bar run their commands (macOS) |
| `shortcuts` | the keyboard shortcuts of the chrome work: the address bar, the tabs, the closed tab and the settings |

## What is not covered yet

- The conversation itself. The chat reaches the room, but nothing about a
  message between two clients is checked.
- The review buttons and the "Open in System Browser" button. Both hand the
  address to the system browser, which cannot be checked from inside the app.
- The unavailable page, which appears when the registry cannot be reached.
- The native menu bar on macOS and Windows as the user sees it. The menu
  bar is an OS menu and cannot be driven by the DevTools protocol, so the
  journeys check the commands its entries run instead.
- The F10 key itself. A key press is an event of the operating system and
  cannot be delivered to the window by the DevTools protocol, so the
  journeys open the menu the way the key does instead. The Linux smoke test
  presses the real key with `xdotool`.
- The density and the custom CSS. The journeys cover the theme only; the
  defects of both are listed in `PLAN.md`, under Phase 9.
- The keyboard shortcuts of a content page. The journeys send the key events
  into the chrome page, which is where the chrome listens for them; the
  content handler needs a real key.
- The downloads and the popup to tab.

## Running it on Linux and Windows

The journeys run against the real build of every platform:

```sh
scripts/verify.sh                # macOS, on the build of this tree
scripts/verify-linux.sh          # inside the container of Dockerfile.linux
pwsh scripts/verify-windows.ps1  # Windows, against the packaged build
```

The Linux script builds the browser inside the container, starts a virtual
display and runs the journeys there. The Windows script runs them against
the packaged build, because that is the one that carries the CEF runtime
next to the executable. Both are also jobs in the CI workflow.
