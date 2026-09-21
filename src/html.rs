pub const HTML_BOILERPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Diff OVERview</title>
    <style>
        :root {
            --border: #d0d7de;
            --muted-background: #f6f8fa;
            --file-header-background: #24292f;
            --section-header-background: #d8dee4;
            --sidebar-width: 18rem;
        }

        * { box-sizing: border-box; }
        html { scroll-behavior: smooth; }

        body {
            margin: 0;
            color: #1f2328;
            background: #fff;
            font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
            font-size: 12px;
        }

        .page {
            display: grid;
            grid-template-columns: clamp(180px, var(--sidebar-width), calc(100vw - 240px)) minmax(0, 1fr);
            min-height: 100vh;
        }

        .page.sidebar-collapsed { grid-template-columns: 44px minmax(0, 1fr); }
        .page.sidebar-empty { display: block; }

        .sidebar {
            position: sticky;
            top: 0;
            height: 100vh;
            overflow: auto;
            padding: 16px 12px;
            border-right: 1px solid var(--border);
            background: var(--muted-background);
        }

        .sidebar-resizer {
            position: absolute;
            top: 0;
            right: 0;
            bottom: 0;
            width: 7px;
            padding: 0;
            border: 0;
            background: transparent;
            cursor: col-resize;
            touch-action: none;
        }

        .sidebar-resizer::after {
            position: absolute;
            top: 0;
            right: 0;
            bottom: 0;
            width: 2px;
            background: transparent;
            content: "";
        }

        .sidebar-resizer:hover::after,
        .sidebar-resizer:focus-visible::after,
        body.sidebar-resizing .sidebar-resizer::after { background: #0969da; }

        body.sidebar-resizing,
        body.sidebar-resizing * {
            cursor: col-resize !important;
            user-select: none !important;
        }

        .sidebar-title {
            margin: 0;
            font-family: system-ui, sans-serif;
            font-size: 13px;
            font-weight: 600;
        }

        .sidebar-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 8px;
            margin: 0 0 12px;
            padding: 0 6px;
        }

        .sidebar-toggle {
            flex: none;
            width: 24px;
            height: 24px;
            padding: 0;
            color: #57606a;
            border: 1px solid transparent;
            border-radius: 4px;
            background: transparent;
            cursor: pointer;
        }

        .sidebar-toggle:hover {
            border-color: var(--border);
            background: #fff;
        }

        .sidebar-collapsed .sidebar {
            padding-right: 9px;
            padding-left: 9px;
        }

        .sidebar-collapsed .sidebar-header { padding: 0; }
        .sidebar-collapsed .sidebar-title,
        .sidebar-collapsed .sidebar nav,
        .sidebar-collapsed .sidebar-resizer { display: none; }
        .file-tree,
        .file-tree ul {
            margin: 0;
            padding: 0;
            list-style: none;
        }

        .file-tree ul { padding-left: 16px; }
        .file-tree li { min-width: 0; }

        .tree-directory > summary,
        .tree-file {
            display: block;
            overflow: hidden;
            padding: 4px 6px;
            color: #1f2328;
            border-radius: 4px;
            text-decoration: none;
            text-overflow: ellipsis;
            white-space: nowrap;
            cursor: pointer;
        }

        .tree-directory > summary {
            display: list-item;
            font-weight: 600;
        }

        .tree-file:hover,
        .tree-directory > summary:hover { background: #eaeef2; }

        .diffs {
            min-width: 0;
            padding: 12px 16px 24px;
        }

        .file-diff {
            margin: 0 0 20px;
            background: #fff;
            scroll-margin-top: 12px;
        }

        .file-diff + .file-diff {
            padding-top: 12px;
            border-top: 1px solid #eaeef2;
        }

        .file-diff > summary,
        .diff-section > summary {
            cursor: pointer;
            user-select: none;
        }

        .file-diff > summary {
            padding: 5px 7px;
            color: #f6f8fa;
            background: var(--file-header-background);
            font-weight: 600;
        }

        .diff-section { margin-top: 4px; }

        .diff-section > summary {
            padding: 4px 7px;
            color: #24292f;
            background: var(--section-header-background);
            font-weight: 600;
        }

        .file-diff > summary:hover { background: #32383f; }
        .diff-section > summary:hover { background: #c7cdd4; }

        .file-diff > summary:focus-visible,
        .diff-section > summary:focus-visible {
            outline: 1px solid #0969da;
            outline-offset: 1px;
        }

        .diff-table-wrap {
            width: 100%;
        }

        .diff-table {
            width: 100%;
            table-layout: fixed;
            border-collapse: collapse;
        }

        .diff-table col,
        .diff-table td { width: 50%; }

        .diff-table td {
            position: relative;
            padding: 3px 7px;
            vertical-align: top;
            background: #fff;
            line-height: 1.35;
            transition: background-color 80ms ease;
        }

        .diff-table td + td { border-left: 1px solid #eef0f2; }
        .diff-table td.empty-content { background: #fdfdfd; }

        .diff-table tr.diff-item:hover td { background: #f6f8fa; }
        .diff-table tr.diff-item:hover td.empty-content { background: #f3f4f6; }

        .diff-cell::before {
            position: absolute;
            top: 3px;
            bottom: 3px;
            left: 0;
            width: 2px;
            border-radius: 999px;
            content: "";
        }

        .diff-cell.deleted-cell::before { background: #cf222e; }
        .diff-cell.added-cell::before { background: #1a7f37; }

        .diff-cell-scroll {
            width: 100%;
            overflow-x: auto;
        }

        pre { margin: 0; }
        .added { color: #00ad14; }
        .deleted { color: #ad0500; }

        @media (max-width: 720px) {
            .page { display: block; }
            .sidebar {
                position: static;
                width: 100%;
                height: auto;
                max-height: 40vh;
                border-right: 0;
                border-bottom: 1px solid var(--border);
            }
            .sidebar-resizer { display: none; }
        }
    </style>
    <script>
        document.addEventListener("DOMContentLoaded", () => {
            const tree = document.getElementById("file-tree");
            const files = document.querySelectorAll(".file-diff[data-file-path]");
            const directories = new Map([["", tree]]);
            const page = document.querySelector(".page");
            const sidebarToggle = document.querySelector(".sidebar-toggle");
            const sidebarResizer = document.querySelector(".sidebar-resizer");

            const setSidebarWidth = (requestedWidth) => {
                const minimumWidth = 180;
                const maximumWidth = Math.max(minimumWidth, window.innerWidth - 240);
                const width = Math.min(maximumWidth, Math.max(minimumWidth, requestedWidth));
                document.documentElement.style.setProperty("--sidebar-width", `${width}px`);
                sidebarResizer.setAttribute("aria-valuenow", String(Math.round(width)));
                sidebarResizer.setAttribute("aria-valuemax", String(maximumWidth));
            };

            sidebarResizer.addEventListener("pointerdown", (event) => {
                if (page.classList.contains("sidebar-collapsed")) return;
                event.preventDefault();
                sidebarResizer.setPointerCapture(event.pointerId);
                document.body.classList.add("sidebar-resizing");
                setSidebarWidth(event.clientX - page.getBoundingClientRect().left);
            });

            sidebarResizer.addEventListener("pointermove", (event) => {
                if (sidebarResizer.hasPointerCapture(event.pointerId)) {
                    setSidebarWidth(event.clientX - page.getBoundingClientRect().left);
                }
            });

            const stopSidebarResize = (event) => {
                if (sidebarResizer.hasPointerCapture(event.pointerId)) {
                    sidebarResizer.releasePointerCapture(event.pointerId);
                }
                document.body.classList.remove("sidebar-resizing");
            };
            sidebarResizer.addEventListener("pointerup", stopSidebarResize);
            sidebarResizer.addEventListener("pointercancel", stopSidebarResize);
            sidebarResizer.addEventListener("keydown", (event) => {
                const currentWidth = document.querySelector(".sidebar").getBoundingClientRect().width;
                if (event.key === "ArrowLeft") setSidebarWidth(currentWidth - 10);
                else if (event.key === "ArrowRight") setSidebarWidth(currentWidth + 10);
                else if (event.key === "Home") setSidebarWidth(180);
                else if (event.key === "End") setSidebarWidth(window.innerWidth - 240);
                else return;
                event.preventDefault();
            });

            sidebarToggle.addEventListener("click", () => {
                const collapsed = page.classList.toggle("sidebar-collapsed");
                sidebarToggle.setAttribute("aria-expanded", String(!collapsed));
                sidebarToggle.title = collapsed ? "Expand sidebar" : "Collapse sidebar";
                sidebarToggle.textContent = collapsed ? "❯❯" : "❮";
            });

            files.forEach((file, index) => {
                file.id = `file-${index + 1}`;
                const path = file.dataset.filePath;
                const parts = path.split(/[\\/]/).filter(Boolean);
                let parent = tree;
                let directoryPath = "";

                parts.slice(0, -1).forEach((part) => {
                    directoryPath = directoryPath ? `${directoryPath}/${part}` : part;
                    if (!directories.has(directoryPath)) {
                        const item = document.createElement("li");
                        const details = document.createElement("details");
                        const summary = document.createElement("summary");
                        const children = document.createElement("ul");
                        details.className = "tree-directory";
                        details.open = true;
                        summary.textContent = `${part}/`;
                        details.append(summary, children);
                        item.append(details);
                        parent.append(item);
                        directories.set(directoryPath, children);
                    }
                    parent = directories.get(directoryPath);
                });

                const item = document.createElement("li");
                const link = document.createElement("a");
                link.className = "tree-file";
                link.href = `#${file.id}`;
                link.textContent = parts.at(-1) || path;
                link.title = path;
                link.addEventListener("click", () => { file.open = true; });
                item.append(link);
                parent.append(item);
            });

            if (files.length === 0) {
                document.querySelector(".sidebar").hidden = true;
                page.classList.add("sidebar-empty");
            }
        });
    </script>
</head>
<body>
<div class="page">
    <aside class="sidebar">
        <header class="sidebar-header">
            <h1 class="sidebar-title">Changed files</h1>
            <button class="sidebar-toggle" type="button" aria-label="Toggle sidebar" aria-expanded="true" title="Collapse sidebar">❮</button>
        </header>
        <nav aria-label="Changed files"><ul class="file-tree" id="file-tree"></ul></nav>
        <div class="sidebar-resizer" role="separator" aria-label="Resize sidebar" aria-orientation="vertical" aria-valuemin="180" aria-valuenow="288" tabindex="0"></div>
    </aside>
    <main class="diffs">
"#;

pub const HTML_EPILOGUE: &str = r#"    </main>
</div>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_shell_includes_navigation_and_fixed_diff_columns() {
        assert!(HTML_BOILERPLATE.contains("id=\"file-tree\""));
        assert!(HTML_BOILERPLATE.contains(".file-diff[data-file-path]"));
        assert!(HTML_BOILERPLATE.contains("table-layout: fixed"));
        assert!(HTML_BOILERPLATE.contains("class=\"sidebar-toggle\""));
        assert!(HTML_BOILERPLATE.contains("class=\"sidebar-resizer\""));
        assert!(HTML_BOILERPLATE.contains("setPointerCapture"));
        assert!(HTML_BOILERPLATE.contains("overflow-x: auto"));
        assert!(HTML_BOILERPLATE.contains(".diff-cell::before"));
        assert!(HTML_BOILERPLATE.contains("<main class=\"diffs\">"));
        assert!(HTML_EPILOGUE.ends_with("</html>"));
    }
}
