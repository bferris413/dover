pub const HTML_BOILERPLATE: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Diff OVERview</title>
    <style>
        body {
            font-family: monospace;
            margin: 0;
            padding: 0;
        }
        .diff-container {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 0.5rem;
            padding: 0.25rem;
        }
        .diff-section {
            border: 1px solid #f7f7f7;
            border-radius: 2px;
            padding: 0.5rem;
            background-color: #fdfdfd;
            overflow: auto;
        }
        .diff-section.old {
            /* border-color: #ffe6e6; */
        }
        .diff-section.new {
            /* border-color: #c8f2c8; */
        }
        .diff-header {
            font-weight: bold;
            margin-bottom: 0.5rem;
        }
        .diff-content {
            white-space: pre-wrap;
        }
        .deleted {
            color: #db0210;
        }
        .added {
            color: #00ad14;

        }
    </style>
</head>
<body>"#;
