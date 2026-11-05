/**
 * Toast notifications.
 *
 * Deliberately tiny: no queue animations, no stacking limits beyond a hard cap,
 * and every toast is announced through an aria-live region.
 */
export type ToastKind = 'info' | 'success' | 'error';

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
}

let nextId = 1;

class ToastStore {
  items = $state<Toast[]>([]);

  push(kind: ToastKind, message: string, ttl = 5000): number {
    const id = nextId++;
    this.items = [...this.items, { id, kind, message }].slice(-4);
    if (ttl > 0) {
      setTimeout(() => this.dismiss(id), ttl);
    }
    return id;
  }

  dismiss(id: number): void {
    this.items = this.items.filter((toast) => toast.id !== id);
  }
}

export const toasts = new ToastStore();
