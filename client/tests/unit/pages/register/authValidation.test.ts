import { describe, expect, it } from 'vitest';
import {
  passwordIssues,
  passwordsMatch,
  usernameIssue,
} from '../../../../src/lib/authValidation.js';

describe('usernameIssue', () => {
  it('accepts valid usernames', () => {
    expect(usernameIssue('alice')).toBeNull();
    expect(usernameIssue('Elon_Musk1')).toBeNull();
  });

  it('rejects length and format', () => {
    expect(usernameIssue('ab')).toBe('usernameLength');
    expect(usernameIssue('a'.repeat(25))).toBe('usernameLength');
    expect(usernameIssue('1alice')).toBe('usernameFormat');
    expect(usernameIssue('alice-bob')).toBe('usernameFormat');
    expect(usernameIssue('alice bob')).toBe('usernameFormat');
  });
});

describe('passwordIssues', () => {
  it('accepts strong password', () => {
    expect(passwordIssues('GoodPass1x')).toEqual([]);
  });

  it('flags weak passwords', () => {
    expect(passwordIssues('short1A')).toContain('minLength');
    expect(passwordIssues('a'.repeat(129) + 'A1')).toContain('maxLength');
    expect(passwordIssues('nouppercase1')).toContain('uppercase');
    expect(passwordIssues('NOLOWERCASE1')).toContain('lowercase');
    expect(passwordIssues('NoDigitsHere')).toContain('digit');
  });
});

describe('passwordsMatch', () => {
  it('compares exactly', () => {
    expect(passwordsMatch('GoodPass1x', 'GoodPass1x')).toBe(true);
    expect(passwordsMatch('GoodPass1x', 'GoodPass1y')).toBe(false);
  });
});
