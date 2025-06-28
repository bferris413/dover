pub const HTML_BOILERPLATE: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Diff OVERview</title>
    <style>
        body {
            padding: 5px;
        }
        /* General table styling */
        table {
            width: 100%;
            margin-bottom: 10px;
            border-collapse: separate;
            border: 1px solid #d0d7de;
            border-radius: 5px;
            overflow: hidden;
            font-size: 12px;
        }

        /* Filename row styling */
        th.filename {
            text-align: left;
            font-family: monospace;
            font-weight: normal;
            background-color: #f4f6fc;
            padding: 8px 12px;
            border-bottom: 1px solid #d0d7de;
        }

        /* Section headers (e.g., "Uses", "Impls") */
        tr th {
            text-align: left;
            font-family: monospace;
            font-weight: normal;
            background-color: #fafafa;
            padding: 6px 12px;
            border-bottom: 1px solid #d0d7de;
        }

        /* Diff content cells */
        td {
            min-width: 90ch;
            padding: 8px 12px;
            border-bottom: 1px solid #d0d7de;
            vertical-align: top;
            background-color: #ffffff;
            border-right: 1px solid #d0d7de;
        }

        td.empty-content {
            background-color: #fdfdfd;
        }

        /* Border for the last column */
        td:last-child {
            border-right: none;
        }

        /* Added content */
        .added {
            color: #00ad14;
            padding: 2px 4px;
            border-radius: 4px;
        }

        /* Remove the bottom border from the last row */
        tr:last-child td {
            border-bottom: none;
        }

        /* Removed content */
        .deleted {
            color: #ad0500;
            padding: 2px 4px;
            border-radius: 4px;
        }
    </style>
</head>
<body>"#;
