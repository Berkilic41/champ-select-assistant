import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { ToastContainer } from './Toast';
import type { Toast } from '../../hooks/useToast';

function toast(type: Toast['type'], message: string): Toast {
  return { id: `${type}-1`, type, message };
}

describe('ToastContainer a11y live regions', () => {
  it('announces error/warning toasts assertively (role=alert)', () => {
    const { container } = render(
      <ToastContainer toasts={[toast('error', 'sync hatası')]} onRemove={() => {}} />,
    );
    const el = container.querySelector('.toast--error');
    expect(el).toHaveAttribute('role', 'alert');
    expect(el).toHaveAttribute('aria-live', 'assertive');
  });

  it('announces info/success toasts politely (role=status)', () => {
    const { container } = render(
      <ToastContainer toasts={[toast('success', 'kaydedildi')]} onRemove={() => {}} />,
    );
    const el = container.querySelector('.toast--success');
    expect(el).toHaveAttribute('role', 'status');
    expect(el).toHaveAttribute('aria-live', 'polite');
  });
});
