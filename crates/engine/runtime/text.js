// deflorta/text — dialogue text tags and translations.
//
// Text tags:
//   {b}…{/b} {i}…{/i} {u}…{/u} {s}…{/s}   bold, italic, underline, strikethrough
//   {color=#f88}…{/color}                  color
//   {size=32} {size=+4} {size=*1.5}…{/size} absolute or relative size
//   {font=Noto Serif}…{/font}               font family
//   {ruby=かんじ}漢字{/ruby}                 annotation above the text (furigana)
//   {w} {w=0.5}                             wait for a click / pause for seconds
//   {p} {p=1}                               like {w}, then a line break
//   {nw}                                    advance automatically once typed
//   {fast}                                  show everything before it instantly
//   {{ }}                                   literal braces
// Closing tags pop the most recent style tag.

import { readText } from "deflorta/core";

const STYLE_TAGS = new Set([
  "b",
  "i",
  "u",
  "s",
  "color",
  "size",
  "font",
  "ruby",
]);

function styleValue(name, arg, current, baseSize) {
  switch (name) {
    case "color":
      return { color: arg };
    case "font":
      return { font: arg };
    case "ruby":
      return { ruby: arg };
    case "size": {
      const base = current.size ?? baseSize;
      if (arg?.startsWith("+") || arg?.startsWith("-"))
        return { size: base + Number(arg) };
      if (arg?.startsWith("*")) return { size: base * Number(arg.slice(1)) };
      return { size: Number(arg) };
    }
    default:
      return { [name]: true };
  }
}

/**
 * Parses text with tags into spans for the engine.
 * Returns { spans, noWait }.
 */
export function parseMarkup(markup, { baseSize = 24 } = {}) {
  const source = String(markup ?? "");
  const spans = [];
  const stack = [];
  let style = {};
  let buffer = "";
  let noWait = false;

  const flush = () => {
    if (buffer) spans.push({ text: buffer, ...style });
    buffer = "";
  };
  const marker = (props) => {
    flush();
    spans.push({ text: "", ...props });
  };

  for (let i = 0; i < source.length; i++) {
    const c = source[i];
    if (c === "}" && source[i + 1] === "}") {
      buffer += "}";
      i++;
      continue;
    }
    if (c !== "{") {
      buffer += c;
      continue;
    }
    if (source[i + 1] === "{") {
      buffer += "{";
      i++;
      continue;
    }
    const end = source.indexOf("}", i);
    if (end < 0) {
      buffer += source.slice(i);
      break;
    }
    const tag = source.slice(i + 1, end);
    i = end;
    const eq = tag.indexOf("=");
    const name = eq < 0 ? tag : tag.slice(0, eq);
    const arg = eq < 0 ? undefined : tag.slice(eq + 1);

    if (name.startsWith("/") && STYLE_TAGS.has(name.slice(1))) {
      flush();
      style = stack.pop() ?? {};
    } else if (STYLE_TAGS.has(name)) {
      flush();
      stack.push(style);
      style = { ...style, ...styleValue(name, arg, style, baseSize) };
    } else if (name === "w") {
      marker(arg ? { wait: Number(arg) } : { click: true });
    } else if (name === "p") {
      marker(arg ? { wait: Number(arg) } : { click: true });
      buffer += "\n";
    } else if (name === "nw") {
      noWait = true;
    } else if (name === "fast") {
      marker({ fast: true });
    } else {
      // Unknown tags are shown as written.
      buffer += `{${tag}}`;
    }
  }
  flush();
  return { spans, noWait };
}

/** Text with all tags removed (for history previews, save names, logs). */
export function plainText(markup) {
  return parseMarkup(markup)
    .spans.map((s) => s.text)
    .join("");
}

// ---------------------------------------------------------------------------
// Translations
// ---------------------------------------------------------------------------

const tables = {};
const loadedFiles = new Set();
let current = null;

/** Adds translations for `language`: { "source text": "translated text" }. */
export function translations(language, table) {
  tables[language] = { ...tables[language], ...table };
}

/**
 * Switches the language; null restores the source language. Strings from
 * `tl/<language>.json` in the game directory are loaded on first use.
 */
export function useLanguage(language) {
  if (language && !loadedFiles.has(language)) {
    loadedFiles.add(language);
    const json = readText(`tl/${language}.json`);
    if (json) {
      try {
        // Strings registered in code take precedence over the file.
        tables[language] = { ...JSON.parse(json), ...tables[language] };
      } catch (e) {
        console.warn(`translation file tl/${language}.json is invalid: ${e}`);
      }
    }
  }
  current = language ? (tables[language] ?? {}) : null;
}

const missing = new Set();

/** Translates a string into the current language (returns it unchanged when untranslated). */
export function _(source) {
  if (!current) return source;
  const translated = current[source];
  if (translated == null) {
    missing.add(source);
    return source;
  }
  return translated;
}

/** Strings looked up but missing in the current language, for translators. */
export function missingTranslations() {
  return [...missing];
}
