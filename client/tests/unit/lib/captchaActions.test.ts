import { describe, expect, it } from 'vitest';
import {
  FAUCET_CAPTCHA_ACTION,
  LOGIN_CAPTCHA_ACTION,
  REGISTER_CAPTCHA_ACTION,
} from '../../../src/lib/captchaActions.js';

describe('captchaActions', () => {
  it('pins Turnstile actions to backend expected_action constants', () => {
    expect(FAUCET_CAPTCHA_ACTION).toBe('faucet_claim');
    expect(LOGIN_CAPTCHA_ACTION).toBe('login');
    expect(REGISTER_CAPTCHA_ACTION).toBe('register');
  });
});
