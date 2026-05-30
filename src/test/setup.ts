import { afterEach } from 'vitest';
import '@testing-library/jest-dom';

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
