import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
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
});
