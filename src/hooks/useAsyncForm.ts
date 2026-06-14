import { useState, useCallback } from "react";

/**
 * Shared error + loading state for dialog submit actions. `run` wraps an async
 * function: it clears the error, flips `loading` on, awaits the work, and
 * captures any throw as an error message — replacing the repeated
 * error/busy useState + try/catch/finally boilerplate across dialogs.
 *
 * Not suitable for fire-and-forget flows that must stay "busy" after the
 * awaited call resolves (e.g. background export) — `loading` always resets.
 */
export function useAsyncForm() {
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const run = useCallback(async (fn: () => Promise<void>) => {
    setError("");
    setLoading(true);
    try {
      await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  return { error, setError, loading, run };
}
