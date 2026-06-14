import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

export type ExportProgress = {
  done: number;
  total: number;
  current_file: string;
};

export type ExportResult = {
  errors: string[];
  output_dir: string;
};

export function useExportProgress() {
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  const [result, setResult] = useState<ExportResult | null>(null);

  useEffect(() => {
    const p = listen<ExportProgress>("export-progress", (e) =>
      setProgress(e.payload)
    );
    const c = listen<ExportResult>("export-complete", (e) => {
      setProgress(null);
      setResult(e.payload);
    });
    return () => {
      p.then((f) => f());
      c.then((f) => f());
    };
  }, []);

  function clear() {
    setResult(null);
    setProgress(null);
  }

  return { progress, result, clear };
}
