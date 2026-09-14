export * from './coins.js';

export interface PublicUser {
  id: string;
  email: string;
  username: string;
  role?: 'ADMIN' | 'USER' | string;
  twoFactorEnabled: boolean;
  merchantStatus: 'NONE' | 'PENDING' | 'APPROVED' | 'REJECTED' | string;
  createdAt: string;
}

export interface AuthLoginResponse {
  user: PublicUser;
  accessToken: string;
  refreshToken?: string;
  codeSent?: boolean;
  message?: string;
}
