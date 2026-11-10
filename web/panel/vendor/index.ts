import { markdownVendor } from './markdown.ts'
import mermaid from 'mermaid'

globalThis.moaiVendor = Object.freeze({ ...markdownVendor, mermaid })

declare global {
  var moaiVendor: Readonly<typeof markdownVendor & { mermaid: typeof mermaid }>
}
