import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { ConnectionBadge } from './ConnectionBadge';
import type { AppStatus } from '../../types/app';

describe('ConnectionBadge', () => {
  it('shows connecting state', () => {
    const { container } = render(<ConnectionBadge status={{ kind: 'connecting' } as AppStatus} />);
    expect(container.textContent).toContain('Bağlanıyor');
    expect(container.querySelector('.badge--connecting')).not.toBeNull();
  });

  it('shows disconnected state', () => {
    const { container } = render(
      <ConnectionBadge status={{ kind: 'disconnected' } as AppStatus} />,
    );
    expect(container.textContent).toContain('Bağlantı yok');
    expect(container.querySelector('.badge--disconnected')).not.toBeNull();
  });

  it('shows the summoner name when connected', () => {
    const { container } = render(
      <ConnectionBadge
        status={{ kind: 'lobby', summonerName: 'Faker#KR1', port: 1234 } as AppStatus}
      />,
    );
    expect(container.textContent).toContain('Faker#KR1');
    expect(container.querySelector('.badge--connected')).not.toBeNull();
  });
});
