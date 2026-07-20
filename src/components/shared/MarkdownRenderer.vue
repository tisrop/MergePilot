<script setup lang="ts">
import { computed } from "vue";
import { marked } from "marked";

const props = defineProps<{
  content: string;
}>();

const allowedTags = new Set([
  "A",
  "B",
  "BLOCKQUOTE",
  "BR",
  "CODE",
  "DEL",
  "EM",
  "H1",
  "H2",
  "H3",
  "H4",
  "H5",
  "H6",
  "HR",
  "I",
  "IMG",
  "INPUT",
  "LI",
  "OL",
  "P",
  "PRE",
  "S",
  "STRONG",
  "TABLE",
  "TBODY",
  "TD",
  "TFOOT",
  "TH",
  "THEAD",
  "TR",
  "UL",
]);

const allowedAttributes: Record<string, Set<string>> = {
  A: new Set(["href", "title"]),
  CODE: new Set(["class"]),
  IMG: new Set(["alt", "height", "src", "title", "width"]),
  INPUT: new Set(["checked", "disabled", "type"]),
  OL: new Set(["start"]),
  TABLE: new Set(["align"]),
  TD: new Set(["align", "colspan", "rowspan"]),
  TH: new Set(["align", "colspan", "rowspan"]),
};

const safeRelativeUrlBase = new URL("https://mergebeacon.invalid");
const explicitSchemePattern = /^[a-z][a-z\d+.-]*:/i;
const protocolRelativePattern = /^[\\/]{2}/;

function isSafeUrl(value: string, attribute: "href" | "src"): boolean {
  const trimmed = value.trim();
  if (!trimmed) return true;
  if (protocolRelativePattern.test(trimmed)) return false;
  try {
    const parsed = new URL(trimmed, safeRelativeUrlBase);
    if (!explicitSchemePattern.test(trimmed)) {
      return parsed.origin === safeRelativeUrlBase.origin;
    }
    const protocol = parsed.protocol;
    return attribute === "href"
      ? ["http:", "https:", "mailto:"].includes(protocol)
      : ["http:", "https:"].includes(protocol);
  } catch {
    return false;
  }
}

function sanitizeHtml(rawHtml: string): string {
  const document = new DOMParser().parseFromString(`<div>${rawHtml}</div>`, "text/html");
  const root = document.body.firstElementChild;
  if (!root) return "";

  for (const element of Array.from(root.querySelectorAll("*"))) {
    if (!allowedTags.has(element.tagName)) {
      if (["SCRIPT", "STYLE", "IFRAME", "OBJECT", "EMBED", "FORM"].includes(element.tagName)) {
        element.remove();
      } else {
        element.replaceWith(...Array.from(element.childNodes));
      }
      continue;
    }

    const allowed = allowedAttributes[element.tagName] ?? new Set<string>();
    for (const attribute of Array.from(element.attributes)) {
      const name = attribute.name.toLowerCase();
      if (name.startsWith("on") || name === "style" || !allowed.has(name)) {
        element.removeAttribute(attribute.name);
      }
    }
    for (const name of ["href", "src"] as const) {
      const value = element.getAttribute(name);
      if (value && !isSafeUrl(value, name)) element.removeAttribute(name);
    }
    if (element.tagName === "INPUT") {
      if (element.getAttribute("type") !== "checkbox") {
        element.remove();
        continue;
      }
      element.setAttribute("disabled", "");
    }
    if (element.tagName === "A" && element.hasAttribute("href")) {
      element.setAttribute("rel", "noopener noreferrer");
    }
  }
  return root.innerHTML;
}

const html = computed(() =>
  sanitizeHtml(marked.parse(props.content, { async: false, gfm: true, breaks: true }) as string),
);
</script>

<template>
  <div class="markdown-renderer" v-html="html" />
</template>
