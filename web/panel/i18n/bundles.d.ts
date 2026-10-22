// `moai-i18n:<code>` is the page's slice (the `page.` keys) of ../../i18n/<code>.json, cut by web/i18n-bundles.mjs.
declare module 'moai-i18n:*' {
  const words: Readonly<Record<string, string>>
  export default words
}
