// ABOUTME: Typed toast injection key and composable.
// ABOUTME: Single source of truth for toast API — eliminates bare string key duplication.
import type { InjectionKey } from "vue";
import { inject } from "vue";

export type ToastApi = {
  addToast: (message: string, type?: "success" | "error" | "info", duration?: number) => void;
};

export const TOAST_KEY: InjectionKey<ToastApi> = Symbol("toast");

export function useToast(): ToastApi {
  const api = inject(TOAST_KEY);
  if (!api) {
    throw new Error("useToast() called outside ToastProvider");
  }
  return api;
}
