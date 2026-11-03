import { useEffect, useRef } from 'react';

/** Auto-dismiss clock for toasts. Re-arms when `armKey` changes; hover or focus
 *  inside the toast holds it, so an action never vanishes under the pointer.
 *  Ref-based on purpose: holding must not re-render the toast mid-click. */
export function useDismissTimer(armKey: unknown, ms: number, onExpire: () => void) {
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const expire = useRef(onExpire);
  expire.current = onExpire;
  const arm = () => { clearTimeout(timer.current); timer.current = setTimeout(() => expire.current(), ms); };
  const hold = () => clearTimeout(timer.current);

  useEffect(() => {
    if (!armKey) return;
    arm();
    return hold;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [armKey, ms]);

  return { onPointerEnter: hold, onPointerLeave: arm, onFocus: hold, onBlur: arm };
}
