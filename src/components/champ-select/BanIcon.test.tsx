import { describe, it, expect } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { BanIcon } from './BanIcon';

describe('BanIcon', () => {
  it('renders the unknown fallback box when no champ key is given', () => {
    const { container } = render(<BanIcon />);
    expect(container.querySelector('.cs-ban-img--unknown')).toBeInTheDocument();
    expect(container.querySelector('img')).toBeNull();
  });

  it('renders the champion image when a key is given', () => {
    const { container } = render(<BanIcon champKey="Garen" />);
    expect(container.querySelector('img.cs-ban-img')).toBeInTheDocument();
  });

  it('falls back to the unknown box when the image fails to load (404/403)', () => {
    const { container } = render(<BanIcon champKey="Garen" />);
    const img = container.querySelector('img.cs-ban-img') as HTMLImageElement;
    expect(img).toBeInTheDocument();
    fireEvent.error(img);
    expect(container.querySelector('.cs-ban-img--unknown')).toBeInTheDocument();
    expect(container.querySelector('img')).toBeNull();
  });
});
