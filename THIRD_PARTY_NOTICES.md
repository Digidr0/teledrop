# Third-party notices

TeleDrop uses the official `@hugeicons/svelte` SVG renderer and `@hugeicons/core-free-icons`. Icon names and module exports were checked against the installed packages. The application icon derives from `TelegramIcon`; other icons are bundled by named per-icon imports. No Pro assets are used.

## Hugeicons free icons — MIT

Copyright (c) 2025 Hugeicons

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Other components

Tauri, Svelte, Reqwest, Tokio, shadcn-svelte, SQLite and other libraries retain
their respective upstream licenses. Versioned dependency inventories are
`package-lock.json` and `src-tauri/Cargo.lock`; source packages carry their
upstream license files. `shadcn-svelte` Button is generated source adapted to
the TeleDrop theme; it is not a separate web service or runtime icon library.

WebView2 is a Microsoft system runtime subject to Microsoft's distribution
terms. Development tools and the Hugeicons MCP server are not part of the
application installation.
