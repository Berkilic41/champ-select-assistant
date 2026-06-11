import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { RoleSelector } from './RoleSelector';

describe('RoleSelector', () => {
  it('renders all five role buttons', () => {
    render(<RoleSelector role="middle" source="lcu" onChange={() => {}} />);
    for (const label of ['Üst', 'Orman', 'Orta', 'Alt', 'Destek']) {
      expect(screen.getByRole('button', { name: label })).toBeInTheDocument();
    }
  });

  it('marks the active role as pressed', () => {
    render(<RoleSelector role="middle" source="manual" onChange={() => {}} />);
    expect(screen.getByRole('button', { name: 'Orta' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: 'Üst' })).toHaveAttribute('aria-pressed', 'false');
  });

  it('calls onChange with the canonical LCU role key', () => {
    const onChange = vi.fn();
    render(<RoleSelector role="" source="none" onChange={onChange} />);
    fireEvent.click(screen.getByRole('button', { name: 'Orman' }));
    expect(onChange).toHaveBeenCalledWith('jungle');
  });

  it('prompts the user to choose when the role is unknown', () => {
    render(<RoleSelector role="" source="none" onChange={() => {}} />);
    expect(screen.getByText(/Rolünü seç/)).toBeInTheDocument();
  });
});
