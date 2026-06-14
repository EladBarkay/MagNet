/** Last path segment (handles both `/` and `\` separators). */
export function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Parent directory (handles both `/` and `\` separators). */
export function parentDir(path: string): string {
  const norm = path.replace(/\\/g, "/").replace(/\/$/, "");
  const idx = norm.lastIndexOf("/");
  return idx > 0 ? norm.slice(0, idx) : norm;
}

/**
 * How a batch's source path is shown in the sidebar: `root/relative` when the
 * batch lives under the event root, otherwise the absolute path unchanged.
 */
export function batchDisplayPath(batchPath: string, rootPath: string | null): string {
  if (!rootPath) return batchPath;
  const norm = (s: string) => s.replace(/\\/g, "/");
  const root = norm(rootPath).replace(/\/$/, "");
  const path = norm(batchPath);
  if (path.startsWith(root + "/")) {
    return path.slice(root.length + 1);
  }
  return path;
}
