// Ambient declaration for Bun's global, used by the entry-point check
// in src/index.ts. The check works in any environment (the `Bun` global
// is simply `undefined` in Node, so the `typeof` check short-circuits).
declare const Bun:
  | {
      main: string;
    }
  | undefined;

interface ImportMeta {
  // Bun extension. https://bun.sh/docs/api/import-meta
  readonly path: string;
}
