import { afterEach } from 'vitest';
import '@testing-library/jest-dom';
// Initialize i18n (lng='tr') so components using useTranslation render real
// strings in tests instead of raw keys.
import '../i18n';

afterEach(() => {
  vi.clearAllMocks();
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
