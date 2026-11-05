pub const DEFAULT_USER_AGENT_CSS: &str = r#"
head, meta, link, script, style, title, noscript, template {
    display: none;
}

html, body {
    display: block;
    margin: 8px;
    font-size: 16px;
    color: #202124;
    background-color: #ffffff;
}

h1 {
    display: block;
    font-size: 28px;
    font-weight: bold;
    margin-top: 18px;
    margin-bottom: 12px;
    color: #1a1a1a;
}

h2 {
    display: block;
    font-size: 22px;
    font-weight: bold;
    margin-top: 16px;
    margin-bottom: 10px;
    color: #202124;
}

h3 {
    display: block;
    font-size: 18px;
    font-weight: bold;
    margin-top: 14px;
    margin-bottom: 8px;
    color: #3c4043;
}

h4 {
    display: block;
    font-size: 16px;
    font-weight: bold;
    margin-top: 12px;
    margin-bottom: 6px;
}

h5 {
    display: block;
    font-size: 14px;
    font-weight: bold;
    margin-top: 10px;
    margin-bottom: 4px;
}

h6 {
    display: block;
    font-size: 12px;
    font-weight: bold;
    margin-top: 8px;
    margin-bottom: 4px;
}

p {
    display: block;
    margin-top: 10px;
    margin-bottom: 10px;
    line-height: 1.5;
}

div, section, article, nav, header, footer, main, form {
    display: block;
}

a {
    display: inline;
    color: #1a73e8;
    text-decoration: underline;
}

b, strong {
    display: inline;
    font-weight: bold;
}

i, em {
    display: inline;
    font-style: italic;
}

small {
    display: inline;
    font-size: 12px;
    color: #5f6368;
}

code {
    display: inline;
    background-color: #f1f3f4;
    color: #d93025;
    padding: 2px 5px;
    border-radius: 4px;
    font-size: 14px;
}

pre {
    display: block;
    background-color: #282c34;
    color: #abb2bf;
    padding: 12px;
    margin: 12px 0;
    border-radius: 6px;
    font-size: 14px;
}

blockquote {
    display: block;
    margin: 12px 0;
    padding: 8px 16px;
    border-left: 4px solid #1a73e8;
    background-color: #f8f9fa;
    color: #5f6368;
}

hr {
    display: block;
    margin-top: 16px;
    margin-bottom: 16px;
    border: 1px solid #dadce0;
}

ul, ol {
    display: block;
    margin-top: 10px;
    margin-bottom: 10px;
    padding-left: 24px;
}

li {
    display: block;
    margin-top: 4px;
    margin-bottom: 4px;
}

button {
    display: inline-block;
    padding: 6px 14px;
    margin: 4px;
    background-color: #1a73e8;
    color: #ffffff;
    border-radius: 4px;
    font-weight: bold;
    font-size: 14px;
}

input {
    display: inline-block;
    padding: 6px 10px;
    margin: 4px;
    border: 1px solid #dadce0;
    border-radius: 4px;
    background-color: #ffffff;
    font-size: 14px;
    color: #202124;
}

img {
    display: inline-block;
}

table {
    display: block;
    margin: 12px 0;
    border-collapse: collapse;
}

tr {
    display: block;
}

td, th {
    display: inline-block;
    padding: 6px 10px;
    border: 1px solid #dadce0;
}

th {
    font-weight: bold;
    background-color: #f8f9fa;
}
"#;
