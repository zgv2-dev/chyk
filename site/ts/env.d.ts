// CHANGELOG.md is imported as text (`with { type: "text" }`), which esbuild bundles as a string.
declare module "*.md" {
  const text: string;
  export default text;
}
