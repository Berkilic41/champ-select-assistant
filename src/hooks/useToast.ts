import { useState, useCallback, useRef, useEffect } from 'react';

export interface Toast {
  id: string;
  message: string;
  type: 'info' | 'success' | 'warning' | 'error';
  duration?: number;
}

export function useToast() {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const counterRef = useRef(0);
  const timersRef = useRef<ReturnType<typeof setTimeout>[]>([]);

  const addToast = useCallback(
    (message: string, type: Toast['type'] = 'info', duration = 3000) => {
      const id = `toast-${++counterRef.current}`;
      setToasts(prev => [...prev, { id, message, type, duration }]);
      const timer = setTimeout(() => {
        setToasts(prev => prev.filter(t => t.id !== id));
      }, duration);
      timersRef.current.push(timer);
    },
    [],
  );

  // Unmount'ta bekleyen auto-dismiss timer'larını temizle — yoksa timer sızıntısı
  // + unmounted bileşende setToasts uyarısı olur.
  useEffect(
    () => () => {
      timersRef.current.forEach(clearTimeout);
      timersRef.current = [];
    },
    [],
  );

  const removeToast = useCallback((id: string) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  return { toasts, addToast, removeToast };
}
