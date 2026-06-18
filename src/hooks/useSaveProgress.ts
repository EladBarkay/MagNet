import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "../constants";

export type SaveProgress = {
  done: number;
  total: number;
  current_file: string;
};

export type SaveResult = {
  errors: string[];
  output_dir: string;
};

export function useSaveProgress() {
  const [progress, setProgress] = useState<SaveProgress | null>(null);
  const [result, setResult] = useState<SaveResult | null>(null);

  useEffect(() => {
    const p = listen<SaveProgress>(EVENTS.SAVE_PROGRESS, (e) =>
      setProgress(e.payload)
    );
    const c = listen<SaveResult>(EVENTS.SAVE_COMPLETE, (e) => {
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
