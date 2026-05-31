import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import i18n from '../../i18n';
import { ErrorBoundary } from './ErrorBoundary';

let shouldThrow = true;
function Boom() {
  if (shouldThrow) throw new Error('boom');
  return <div>recovered content</div>;
}

describe('ErrorBoundary', () => {
  let errSpy: ReturnType<typeof vi.spyOn>;
  beforeEach(() => {
    shouldThrow = true;
    // React logs caught render errors to console.error — silence the noise.
    errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });
  afterEach(() => errSpy.mockRestore());

  it('renders children when there is no error', () => {
    shouldThrow = false;
    const { container } = render(
      <ErrorBoundary>
        <div>healthy</div>
      </ErrorBoundary>,
    );
    expect(container.textContent).toContain('healthy');
  });

  it('renders the localized fallback when a child throws', () => {
    const { container, getByText } = render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );
    expect(container.textContent).toContain(i18n.t('app.errorTitle'));
    expect(getByText(i18n.t('app.errorRetry'))).toBeTruthy();
  });

  it('recovers when retry is clicked after the cause is gone', () => {
    const { container, getByText } = render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );
    shouldThrow = false; // cause resolved
    fireEvent.click(getByText(i18n.t('app.errorRetry')));
    expect(container.textContent).toContain('recovered content');
  });
});
