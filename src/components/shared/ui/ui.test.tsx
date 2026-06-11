import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Card } from './Card';
import { Badge } from './Badge';
import { Button } from './Button';
import { Tabs } from './Tabs';
import { ConfidenceRing } from './ConfidenceRing';
import { StatBar } from './StatBar';

describe('UI primitives', () => {
  it('Card applies variant + accent classes', () => {
    const { container } = render(
      <Card variant="raised" accent="teal">
        body
      </Card>,
    );
    const el = container.firstChild as HTMLElement;
    expect(el).toHaveClass('ui-card', 'ui-card--raised', 'ui-card--accent-teal');
    expect(screen.getByText('body')).toBeInTheDocument();
  });

  it('Badge applies tone + pill', () => {
    const { container } = render(
      <Badge tone="danger" pill>
        Risk
      </Badge>,
    );
    const el = container.firstChild as HTMLElement;
    expect(el).toHaveClass('ui-badge', 'ui-badge--danger', 'ui-badge--pill');
  });

  it('Button is type=button by default and fires onClick', () => {
    const onClick = vi.fn();
    render(
      <Button variant="primary" onClick={onClick}>
        Go
      </Button>,
    );
    const btn = screen.getByRole('button', { name: 'Go' });
    expect(btn).toHaveAttribute('type', 'button');
    expect(btn).toHaveClass('ui-button--primary');
    fireEvent.click(btn);
    expect(onClick).toHaveBeenCalledOnce();
  });

  it('StatBar renders label + clamped percent', () => {
    render(<StatBar label="Matchup" value={1.4} />);
    expect(screen.getByText('Matchup')).toBeInTheDocument();
    expect(screen.getByText('100%')).toBeInTheDocument();
  });

  it('ConfidenceRing shows the score percent with an aria label', () => {
    render(<ConfidenceRing score={0.72} confidence="high" ariaLabel="güven yüksek" />);
    expect(screen.getByText('72')).toBeInTheDocument();
    expect(screen.getByRole('img', { name: 'güven yüksek' })).toBeInTheDocument();
  });

  it('Tabs: selects on click, exposes aria, and arrow-key navigates', () => {
    const onChange = vi.fn();
    const tabs = [
      { value: 'build', label: 'Build' },
      { value: 'matchup', label: 'Matchup' },
      { value: 'team', label: 'Team' },
    ];
    const { rerender } = render(
      <Tabs tabs={tabs} value="build" onChange={onChange} ariaLabel="sections" />,
    );
    const list = screen.getByRole('tablist', { name: 'sections' });
    const buildTab = screen.getByRole('tab', { name: 'Build' });
    expect(buildTab).toHaveAttribute('aria-selected', 'true');

    fireEvent.click(screen.getByRole('tab', { name: 'Matchup' }));
    expect(onChange).toHaveBeenCalledWith('matchup');

    // ArrowRight from the active 'build' → 'matchup'.
    fireEvent.keyDown(list, { key: 'ArrowRight' });
    expect(onChange).toHaveBeenCalledWith('matchup');

    // Wrap-around: ArrowLeft from 'build' → last tab 'team'.
    fireEvent.keyDown(list, { key: 'ArrowLeft' });
    expect(onChange).toHaveBeenCalledWith('team');

    rerender(<Tabs tabs={tabs} value="team" onChange={onChange} ariaLabel="sections" />);
    expect(screen.getByRole('tab', { name: 'Team' })).toHaveAttribute('aria-selected', 'true');
  });
});
