function convertInlineMarkdown(value: string) {
  const protectedValues: string[] = [];
  const protect = (content: string) => {
    const token = `\u0000${protectedValues.length}\u0000`;
    protectedValues.push(content);
    return token;
  };

  let converted = value
    .replace(/`([^`\n]+)`/g, (_match, code: string) => protect(code))
    .replace(
      /!\[([^\]]*)\]\((\S+?)(?:\s+["'][^"']*["'])?\)/g,
      (_match, alt: string, url: string) =>
        alt
          ? `[[${alt}>${protect(url)}]]`
          : `[[${protect(url)}]]`,
    )
    .replace(
      /\[([^\]]+)\]\((\S+?)(?:\s+["'][^"']*["'])?\)/g,
      (_match, label: string, url: string) =>
        `[[${label}>${protect(url)}]]`,
    )
    .replace(/\*\*\*([^*\n]+)\*\*\*/g, "'''''$1'''''")
    .replace(
      /(^|[^\p{L}\p{N}_])___([^_\n]+)___(?![\p{L}\p{N}_])/gu,
      "$1'''''$2'''''",
    )
    .replace(/\*\*([^*\n]+)\*\*/g, "''$1''")
    .replace(
      /(^|[^\p{L}\p{N}_])__([^_\n]+)__(?![\p{L}\p{N}_])/gu,
      "$1''$2''",
    )
    .replace(/~~([^~\n]+)~~/g, "%%$1%%")
    .replace(/\*([^*\n]+)\*/g, "'''$1'''")
    .replace(
      /(^|[^\p{L}\p{N}_])_([^_\n]+)_(?![\p{L}\p{N}_])/gu,
      "$1'''$2'''",
    );

  converted = converted.replace(/\u0000(\d+)\u0000/g, (_match, index) => {
    return protectedValues[Number(index)] ?? "";
  });
  return converted;
}

function markdownListIndent(lines: string[]) {
  const indents = lines
    .map((line) => {
      const match = line.match(/^([ \t]+)(?:[-*+]|\d+[.)])\s+/);
      if (!match) return 0;
      return match[1].replace(/\t/g, "    ").length;
    })
    .filter((indent) => indent > 0);
  return indents.length > 0 ? Math.min(...indents) : 2;
}

function isTableSeparator(line: string) {
  const trimmed = line.trim().replace(/^\|/, "").replace(/\|$/, "");
  const cells = trimmed.split("|").map((cell) => cell.trim());
  return cells.length > 0 && cells.every((cell) => /^:?-{3,}:?$/.test(cell));
}

export function markdownToBacklog(markdown: string) {
  const lines = markdown.replace(/\r\n?/g, "\n").split("\n");
  const indentUnit = markdownListIndent(lines);
  const result: string[] = [];
  let fence: { marker: string } | null = null;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    if (fence) {
      if (new RegExp(`^\\s*${fence.marker}\\s*$`).test(line)) {
        result.push("{/code}");
        fence = null;
      } else {
        result.push(line);
      }
      continue;
    }

    const fenceStart = line.match(/^\s*(```|~~~)\s*([^ ]*)\s*$/);
    if (fenceStart) {
      const language = fenceStart[2].replace(/[^a-zA-Z0-9#+.-]/g, "");
      result.push(language ? `{code:${language}}` : "{code}");
      fence = { marker: fenceStart[1] };
      continue;
    }

    const nextLine = lines[index + 1];
    if (
      line.trim() &&
      nextLine &&
      (/^\s*=+\s*$/.test(nextLine) || /^\s*-+\s*$/.test(nextLine))
    ) {
      result.push(
        `${nextLine.includes("=") ? "*" : "**"} ${convertInlineMarkdown(line.trim())}`,
      );
      index += 1;
      continue;
    }

    if (
      line.trimStart().startsWith("|") &&
      nextLine &&
      isTableSeparator(nextLine)
    ) {
      result.push(`${convertInlineMarkdown(line.trimEnd())}h`);
      index += 1;
      continue;
    }

    if (isTableSeparator(line)) continue;

    if (!line.trim()) continue;

    const heading = line.match(/^(#{1,6})\s+(.+)$/);
    if (heading) {
      result.push(
        `${"*".repeat(heading[1].length)} ${convertInlineMarkdown(heading[2])}`,
      );
      continue;
    }

    const unordered = line.match(/^([ \t]*)[-*+]\s+(.+)$/);
    if (unordered) {
      const width = unordered[1].replace(/\t/g, "    ").length;
      const depth = Math.floor(width / indentUnit) + 1;
      result.push(
        `${"-".repeat(depth)} ${convertInlineMarkdown(unordered[2])}`,
      );
      continue;
    }

    const ordered = line.match(/^([ \t]*)\d+[.)]\s+(.+)$/);
    if (ordered) {
      const width = ordered[1].replace(/\t/g, "    ").length;
      const depth = Math.floor(width / indentUnit) + 1;
      result.push(
        `${"+".repeat(depth)} ${convertInlineMarkdown(ordered[2])}`,
      );
      continue;
    }

    const quote = line.match(/^(\s*>+\s?)(.*)$/);
    if (quote) {
      result.push(`${quote[1]}${convertInlineMarkdown(quote[2])}`);
      continue;
    }

    result.push(convertInlineMarkdown(line));
  }

  if (fence) result.push("{/code}");
  return result.join("\n").trimEnd();
}

export function noteToBacklog(title: string, content: string) {
  const normalizedTitle = title.trim() || "無題のメモ";
  const convertedContent = markdownToBacklog(content);
  return convertedContent
    ? `* ${normalizedTitle}\n${convertedContent}`
    : `* ${normalizedTitle}`;
}
