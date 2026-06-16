import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { BuildSummary } from './BuildSummary';

const base = {
  coreItems: [] as number[],
  situationalItems: [] as number[],
  primaryRuneTree: 0,
  keystone: 0,
  championName: 'Garen',
};

describe('BuildSummary', () => {
  it('shows an honest "no build data" message when build_source is none', () => {
    render(<BuildSummary {...base} buildSource="none" />);
    expect(screen.getByText('Bu şampiyon için build verisi yok')).toBeInTheDocument();
  });

  it('shows the loading message when no build data has arrived yet', () => {
    render(<BuildSummary {...base} />);
    expect(screen.getByText('Build verisi yükleniyor…')).toBeInTheDocument();
  });

  it('renders the build when core items are present', () => {
    render(<BuildSummary {...base} coreItems={[3071]} keystone={8010} buildSource="seed" />);
    expect(screen.getByText('Garen Build')).toBeInTheDocument();
  });
});
