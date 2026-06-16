import { useEffect, type RefObject } from 'react';

const FOCUSABLE =
  'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

/**
 * Modal a11y: açılışta odağı panel içindeki ilk odaklanabilir öğeye taşır,
 * kapanışta (unmount) odağı modalı açan önceki öğeye geri verir. WCAG 2.4.3
 * odak sırası / klavye kullanıcısı için temel davranış. (Tab döngü-kapanı ayrı.)
 */
export function useModalFocus(
  panelRef: RefObject<HTMLElement | null>,
  active = true,
): void {
  useEffect(() => {
    if (!active) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const first = panelRef.current?.querySelector<HTMLElement>(FOCUSABLE);
    (first ?? panelRef.current)?.focus?.();
    return () => {
      previouslyFocused?.focus?.();
    };
  }, [panelRef, active]);
}
