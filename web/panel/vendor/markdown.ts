import MarkdownIt from 'markdown-it'
import { footnote } from '@mdit/plugin-footnote'
import { cjkFriendly } from '@mdit/plugin-cjk-friendly'
import { katex as katexPlugin } from '@mdit/plugin-katex'
import { snippet } from '@mdit/plugin-snippet'
import katex from 'katex'

export const markdownVendor = { MarkdownIt, footnote, cjkFriendly, katexPlugin, snippet, katex }
