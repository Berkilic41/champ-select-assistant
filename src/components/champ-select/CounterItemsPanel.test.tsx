import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CounterItemsPanel } from './CounterItemsPanel';
import type { CounterItemHint } from '../../types/recommendation';

describe('CounterItemsPanel', () => {
  it('renders nothing when there are no hints', () => {
    const { container } = render(<CounterItemsPanel items={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders category and reason', () => {
    const items: CounterItemHint[] = [
      { category: 'MR', reason: 'Düşman AP ağırlıklı — magic resist al', item_ids: [3001] },
    ];
    render(<CounterItemsPanel items={items} />);
    expect(screen.getByText('MR')).toBeInTheDocument();
    expect(screen.getByText(/magic resist al/)).toBeInTheDocument();
  });

  it('falls back to an empty box when an item icon fails to load (404/403)', () => {
    const items: CounterItemHint[] = [
      { category: 'MR', reason: 'Düşman AP — magic resist al', item_ids: [3001] },
    ];
    const { container } = render(<CounterItemsPanel items={items} />);
    const img = container.querySelector('img.counter-items__icon') as HTMLImageElement;
    expect(img).toBeInTheDocument();
    fireEvent.error(img);
    expect(container.querySelector('.counter-items__icon--empty')).toBeInTheDocument();
    expect(container.querySelector('img')).toBeNull();
  });
});
