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
            grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
            min-height: 100vh;
        }

        .page.sidebar-collapsed { --sidebar-width: 44px; }
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
        .sidebar-collapsed .sidebar nav { display: none; }
        .sidebar-collapsed .sidebar-toggle { transform: rotate(180deg); }

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
            padding: 16px;
        }

        .file-diff {
            margin-bottom: 16px;
            border: 1px solid var(--border);
            border-radius: 6px;
            background: #fff;
            overflow: hidden;
            scroll-margin-top: 16px;
        }

        .file-diff > summary,
        .diff-section > summary {
            cursor: pointer;
            user-select: none;
        }

        .file-diff > summary {
            padding: 10px 12px;
            background: #eef2f8;
            font-weight: 600;
        }

        .file-diff[open] > summary { border-bottom: 1px solid var(--border); }
        .diff-section + .diff-section { border-top: 1px solid var(--border); }

        .diff-section > summary {
            padding: 7px 12px;
            background: #fafbfc;
        }

        .diff-section[open] > summary { border-bottom: 1px solid var(--border); }

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
            padding: 8px 12px;
            border-right: 1px solid var(--border);
            border-bottom: 1px solid var(--border);
            vertical-align: top;
            background: #fff;
        }

        .diff-table tr:last-child td { border-bottom: 0; }
        .diff-table td:last-child { border-right: 0; }
        .diff-table td.empty-content { background: #fdfdfd; }

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
        }
    </style>
    <script>
        document.addEventListener("DOMContentLoaded", () => {
            const tree = document.getElementById("file-tree");
            const files = document.querySelectorAll(".file-diff[data-file-path]");
            const directories = new Map([["", tree]]);
            const page = document.querySelector(".page");
            const sidebarToggle = document.querySelector(".sidebar-toggle");

            sidebarToggle.addEventListener("click", () => {
                const collapsed = page.classList.toggle("sidebar-collapsed");
                sidebarToggle.setAttribute("aria-expanded", String(!collapsed));
                sidebarToggle.title = collapsed ? "Expand sidebar" : "Collapse sidebar";
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
                        summary.textContent = part;
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
        assert!(HTML_BOILERPLATE.contains("overflow-x: auto"));
        assert!(HTML_BOILERPLATE.contains("<main class=\"diffs\">"));
        assert!(HTML_EPILOGUE.ends_with("</html>"));
    }
}
