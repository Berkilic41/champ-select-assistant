import { afterEach } from 'vitest';
import '@testing-library/jest-dom';
// Initialize i18n (lng='tr') so components using useTranslation render real
// strings in tests instead of raw keys.
import '../i18n';

afterEach(() => {
  vi.clearAllMocks();
});

// Host adaptörü (Electron window.api sarmalayıcısı) global mock — testler
// invoke/listen handle'larını `import { invoke } from '<...>/lib/host'` ile alır.
vi.mock('../lib/host', () => ({
  invoke: vi.fn(),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
