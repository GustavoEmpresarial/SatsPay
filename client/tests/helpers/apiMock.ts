/**
 * Shared API stub returning rich fixtures so pages can paint async UI branches.
 */
import { vi } from 'vitest';

const airdropProfile = {
  season_id: 's1',
  season_number: 1,
  season_title: 'Season 1',
  base_points: 1000,
  bonus_points: 100,
  total_points: 1100,
  tier: 'BRONZE',
  multiplier: 1,
  global_rank: 10,
  total_participants: 100,
  claimed: false,
  projected_reward_usd: '1.00',
  days_remaining: 30,
};

const adminStats = {
  total_users: 10,
  new_users_24h: 1,
  active_users_24h: 2,
  total_wallets: 20,
  total_merchants: 1,
  total_faucet_sites: 1,
  pending_withdrawals: 0,
  total_deposits_count: 5,
  total_withdrawals_count: 2,
  deposits_by_coin: [{ coin: 'BTC', count: 5, total_amount: '50000000', total_fee: '0' }],
  withdrawals_by_coin: [{ coin: 'BTC', count: 2, total_amount: '2000000', total_fee: '2000' }],
  user_balances: [
    { coin: 'BTC', balance: '100000000' },
    { coin: 'LTC', balance: '500000000' },
  ],
  house_balances: [
    { coin: 'BTC', balance: '1000000000' },
    { coin: 'LTC', balance: '2000000000' },
  ],
  recent_deposits: [
    {
      id: 'rd1',
      email: 'cov@bitcosats.test',
      coin: 'BTC',
      amount: '10000000',
      tx_hash: 'abc123def456abc123def456abc123def456abc123def456abc123def456abcd',
      status: 'CREDITED',
      confirmations: 3,
      created_at: '2024-06-01T12:00:00.000Z',
    },
  ],
  recent_withdrawals: [
    {
      id: 'rw1',
      user_id: '11111111-1111-1111-1111-111111111111',
      email: 'cov@bitcosats.test',
      coin: 'BTC',
      to_address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amount: '5000000',
      fee: '1000',
      status: 'CONFIRMED',
      tx_hash: 'def456abc123def456abc123def456abc123def456abc123def456abc123def456abce',
      requires_approval: false,
      created_at: '2024-06-02T12:00:00.000Z',
    },
  ],
  server: {
    uptime_seconds: 3600,
    cpu_load_1m: 0.2,
    cpu_load_5m: 0.3,
    memory_used_mb: 100,
    memory_total_mb: 1024,
    memory_pct: 10,
    db_connections_active: 2,
    db_connections_idle: 3,
    db_connections_max: 20,
    status: 'ok',
  },
};

const telemetryOverview = {
  system_health_pct: 99.5,
  error_rate_24h: 0.1,
  critical_errors_24h: 0,
  critical_errors_count: 1,
  total_errors_24h: 2,
  open_errors_count: 2,
  avg_latency_ms: 40,
  requests_24h: 1000,
  active_db_connections: 5,
  total_users: 10,
  total_wallets: 20,
  total_merchants: 1,
  pending_withdrawals: 0,
};

const telemetryErrors = [
  {
    id: 'err1',
    fingerprint: 'fp1',
    service: 'api',
    level: 'ERROR',
    message: 'Test exception in swap handler',
    stack_trace: 'Error: test\n    at handler (swap.rs:1:1)',
    endpoint: '/swap',
    method: 'POST',
    status_code: 500,
    user_id: '11111111-1111-1111-1111-111111111111',
    ip_address: '127.0.0.1',
    occurrences_count: 3,
    status: 'OPEN' as const,
    first_seen_at: '2024-06-01T10:00:00.000Z',
    last_seen_at: '2024-06-01T12:00:00.000Z',
  },
  {
    id: 'err2',
    fingerprint: 'fp2',
    service: 'worker',
    level: 'WARN',
    message: 'Webhook retry exhausted',
    occurrences_count: 1,
    status: 'RESOLVED' as const,
    first_seen_at: '2024-05-30T10:00:00.000Z',
    last_seen_at: '2024-05-31T12:00:00.000Z',
    resolved_at: '2024-05-31T14:00:00.000Z',
  },
];

const metricsSnapshots = [
  {
    id: 'snap1',
    rpm: 40,
    avg_latency_ms: 42,
    p95_latency_ms: 90,
    error_rate_pct: 0.08,
    active_db_connections: 3,
    total_users: 10,
    total_wallets: 20,
    total_merchants: 1,
    pending_withdrawals: 0,
    volume_usd_24h: '8000.00',
    captured_at: '2024-06-01T08:00:00.000Z',
  },
  {
    id: 'snap2',
    rpm: 80,
    avg_latency_ms: 38,
    p95_latency_ms: 85,
    error_rate_pct: 0.06,
    active_db_connections: 4,
    total_users: 10,
    total_wallets: 20,
    total_merchants: 1,
    pending_withdrawals: 0,
    volume_usd_24h: '10000.00',
    captured_at: '2024-06-01T10:00:00.000Z',
  },
  {
    id: 'snap3',
    rpm: 120,
    avg_latency_ms: 35,
    p95_latency_ms: 80,
    error_rate_pct: 0.05,
    active_db_connections: 4,
    total_users: 10,
    total_wallets: 20,
    total_merchants: 1,
    pending_withdrawals: 0,
    volume_usd_24h: '12500.50',
    captured_at: '2024-06-01T12:00:00.000Z',
  },
  {
    id: 'snap4',
    rpm: 95,
    avg_latency_ms: 36,
    p95_latency_ms: 78,
    error_rate_pct: 0.04,
    active_db_connections: 5,
    total_users: 10,
    total_wallets: 20,
    total_merchants: 1,
    pending_withdrawals: 0,
    volume_usd_24h: '13000.00',
    captured_at: '2024-06-01T14:00:00.000Z',
  },
];

const treasuryWallets = {
  wallets: [
    {
      role: 'hot',
      coin: 'BTC',
      address: 'bc1qplatformhotwallet0000000000000000000',
      hd_index: 0,
      email: null,
      onchain: '500000000',
      ledger: '500000000',
      error: null,
    },
    {
      role: 'hot',
      coin: 'LTC',
      address: 'ltc1qplatformhot0000000000000000000000',
      hd_index: 0,
      email: null,
      onchain: '1000000000',
      ledger: '1000000000',
      error: null,
    },
    {
      role: 'deposit',
      coin: 'BTC',
      address: 'bc1quserdeposit000000000000000000000000',
      hd_index: 42,
      email: 'user@example.com',
      onchain: '25000000',
      ledger: '25000000',
      error: null,
    },
  ],
  explorers: {
    BTC: 'https://mempool.space',
    LTC: 'https://litecoinspace.org',
  },
};

const adminFaucetSites = [
  {
    id: 'fs1',
    owner_id: 'u1',
    owner_email: 'owner@faucet.test',
    name: 'Demo Faucet',
    url: 'https://faucet.example.com',
    description: 'Free test coins',
    coins: ['BTC', 'LTC'],
    reward_info: '100 sats/hour',
    status: 'PENDING',
    clicks: 42,
    created_at: '2024-06-01T08:00:00.000Z',
  },
  {
    id: 'fs2',
    owner_id: 'u2',
    owner_email: 'live@faucet.test',
    name: 'Live Faucet',
    url: 'https://live-faucet.example.com',
    description: 'Approved partner',
    coins: ['DOGE'],
    status: 'APPROVED',
    clicks: 900,
    created_at: '2024-05-01T08:00:00.000Z',
  },
];

const publicFaucetListSites = [
  {
    id: 'pub1',
    name: 'Public Faucet',
    url: 'https://public-faucet.example.com',
    description: 'Listed faucet',
    coins: ['BTC'],
    status: 'APPROVED',
    clicks: 100,
    created_at: '2024-06-01T00:00:00.000Z',
  },
];

const lendMarkets = [
  {
    coin: 'USDT',
    total_supply: '1000000000000',
    total_debt: '500000000000',
    available: '500000000000',
    utilization_bps: 5000,
    supply_apy_bps: 337,
    borrow_apy_bps: 687,
    collateral_factor_bps: 8000,
    liquidation_threshold_bps: 8500,
    borrow_enabled: true,
    can_be_collateral: true,
  },
  {
    coin: 'BTC',
    total_supply: '200000000',
    total_debt: '50000000',
    available: '150000000',
    utilization_bps: 2500,
    supply_apy_bps: 10,
    borrow_apy_bps: 71,
    collateral_factor_bps: 7500,
    borrow_enabled: true,
    can_be_collateral: true,
  },
];

const merchantDepositInvoices = [
  {
    id: 'minv1',
    orderId: 'ORD-88219',
    siteUserId: 'cust-1',
    siteName: 'Demo Shop',
    coin: 'BTC',
    amount: '100000',
    feeAmount: '1000',
    netAmount: '99000',
    depositAddress: 'bc1qmerchantdeposit0000000000000000000',
    status: 'CONFIRMED',
    callbackUrl: 'https://shop.example.com/webhook',
    customerEmail: 'buyer@example.com',
    webhookDelivered: true,
    webhookStatusCode: 200,
    webhookAttempts: 1,
    txHash: 'abc123def456abc123def456abc123def456abc123def456abc123def456abcd',
    paidAt: '2024-06-01T14:00:00.000Z',
    createdAt: '2024-06-01T12:00:00.000Z',
  },
  {
    id: 'minv2',
    orderId: 'ORD-PEND',
    siteName: 'Demo Shop',
    coin: 'LTC',
    amount: '50000000',
    feeAmount: '50000',
    netAmount: '49950000',
    depositAddress: 'ltc1qmerchant000000000000000000000000',
    status: 'PENDING',
    callbackUrl: 'https://shop.example.com/webhook',
    webhookDelivered: false,
    webhookAttempts: 0,
    createdAt: '2024-06-02T12:00:00.000Z',
  },
  {
    id: 'minv3',
    orderId: 'ORD-EXP',
    siteName: 'Demo Shop',
    coin: 'BTC',
    amount: '200000',
    feeAmount: '2000',
    netAmount: '198000',
    depositAddress: 'bc1qexpired000000000000000000000000000',
    status: 'EXPIRED',
    callbackUrl: 'https://shop.example.com/webhook',
    webhookDelivered: false,
    webhookAttempts: 3,
    webhookStatusCode: 500,
    createdAt: '2024-05-01T12:00:00.000Z',
  },
  {
    id: 'minv4',
    orderId: 'ORD-DET',
    siteName: 'Demo Shop',
    coin: 'USDT',
    amount: '100000000',
    feeAmount: '1000000',
    netAmount: '99000000',
    depositAddress: '0xmerchant000000000000000000000000000001',
    status: 'DETECTED',
    callbackUrl: 'https://shop.example.com/webhook',
    webhookDelivered: false,
    webhookAttempts: 2,
    createdAt: '2024-06-03T08:00:00.000Z',
  },
];

const oauthAppsFixture = [
  {
    id: 'app1',
    user_id: '11111111-1111-1111-1111-111111111111',
    name: 'Test App',
    description: 'OAuth test application',
    website_url: 'https://example.com',
    logo_url: 'https://cdn.example.com/icon.png',
    client_id: 'sats_app_test',
    client_secret_prefix: 'sats_sec',
    redirect_uris: ['https://example.com/cb', 'http://localhost:3000/cb'],
    is_active: true,
    created_at: '2024-01-01T00:00:00.000Z',
    updated_at: '2024-01-01T00:00:00.000Z',
  },
  {
    id: 'app2',
    user_id: '11111111-1111-1111-1111-111111111111',
    name: 'Partner Two',
    description: 'Second OAuth app',
    website_url: 'https://partner-two.example.com',
    logo_url: 'https://www.satspay.pro/logo.png',
    client_id: 'sats_app_partner2',
    client_secret_prefix: 'sats_p2',
    redirect_uris: ['https://partner-two.example.com/oauth/cb'],
    is_active: true,
    created_at: '2024-02-01T00:00:00.000Z',
    updated_at: '2024-02-01T00:00:00.000Z',
  },
];

export function installApiMock() {
  return vi.mock('../../../src/lib/api.js', () => ({
    api: vi.fn(async (path: string) => mockApi(path)),
    bootstrapSession: vi.fn(async () => null),
    forceReauth: vi.fn(),
  }));
}

export async function mockApi(path: string): Promise<unknown> {
  const p = path.split('?')[0] || path;

  if (p.includes('/airdrop/overview')) return airdropProfile;
  if (p.includes('/airdrop/leaderboard')) {
    return {
      leaderboard: [
        {
          rank: 1,
          user_id: 'u1',
          username: 'top',
          tier: 'GOLD',
          total_points: 9999,
          multiplier: 2,
        },
      ],
    };
  }
  if (p.includes('/airdrop/logs')) {
    return {
      logs: [
        {
          id: '1',
          activity_type: 'FAUCET_CLAIM',
          description: 'claim',
          points: 10,
          bonus_points: 0,
          created_at: '2024-01-01T00:00:00Z',
        },
      ],
    };
  }
  if (p.includes('/admin/telemetry/overview')) return telemetryOverview;
  if (p.includes('/admin/telemetry/metrics-history')) return { snapshots: metricsSnapshots };
  if (p.includes('/admin/telemetry/metrics')) return { snapshots: metricsSnapshots };
  if (p.includes('/admin/telemetry/errors')) return { errors: telemetryErrors };
  if (p.includes('/admin/treasury-wallets')) return treasuryWallets;
  if (p.includes('/admin/stats')) return adminStats;
  if (p.includes('/admin/withdrawals')) {
    return {
      withdrawals: [
        {
          id: 'aw1',
          user_id: '11111111-1111-1111-1111-111111111111',
          email: 'cov@bitcosats.test',
          coin: 'BTC',
          to_address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
          amount: '5000000',
          fee: '1000',
          status: 'PENDING',
          requires_approval: true,
          created_at: '2024-06-01T12:00:00.000Z',
        },
      ],
    };
  }
  if (p.includes('/admin/economics')) {
    return {
      all_time: {
        faucet_claims: 4,
        faucet_unique_users: 2,
        faucet_by_coin: [{ coin: 'BTC', count: 4, volume: '4000', fees: '0' }],
        gateway_created: 12,
        gateway_paid: 8,
        gateway_by_coin: [
          { coin: 'BTC', count: 5, volume: '50000000', fees: '250000' },
          { coin: 'LTC', count: 3, volume: '300000000', fees: '1500000' },
        ],
        deposits_count: 10,
        deposits_by_coin: [{ coin: 'BTC', count: 10, volume: '100000000', fees: '0' }],
        withdrawals_count: 3,
        withdrawals_by_coin: [{ coin: 'BTC', count: 3, volume: '3000000', fees: '3000' }],
        swap_count: 1,
        swap_fees_by_coin: [{ coin: 'BTC', count: 1, volume: '10000000', fees: '50000' }],
        network_by_kind: [
          { coin: 'BTC', kind: 'WITHDRAWAL', count: 2, amount: '800' },
          { coin: 'BTC', kind: 'SWEEP', count: 1, amount: '400' },
        ],
        fee_margin_by_coin: [
          {
            coin: 'BTC',
            fees_earned: '303000',
            network_paid: '1200',
            faucet_cost: '4000',
            fee_margin: '301800',
            operating_margin: '297800',
            healthy: true,
          },
          {
            coin: 'LTC',
            fees_earned: '1500000',
            network_paid: '0',
            faucet_cost: '0',
            fee_margin: '1500000',
            operating_margin: '1500000',
            healthy: true,
          },
        ],
      },
      last_24h: {
        faucet_claims: 1,
        faucet_unique_users: 1,
        faucet_by_coin: [{ coin: 'BTC', count: 1, volume: '1000', fees: '0' }],
        gateway_created: 2,
        gateway_paid: 1,
        gateway_by_coin: [{ coin: 'BTC', count: 1, volume: '10000000', fees: '50000' }],
        deposits_count: 2,
        deposits_by_coin: [{ coin: 'BTC', count: 2, volume: '20000000', fees: '0' }],
        withdrawals_count: 0,
        withdrawals_by_coin: [],
        swap_count: 0,
        swap_fees_by_coin: [],
        network_by_kind: [],
        fee_margin_by_coin: [
          {
            coin: 'BTC',
            fees_earned: '50000',
            network_paid: '0',
            faucet_cost: '1000',
            fee_margin: '50000',
            operating_margin: '49000',
            healthy: true,
          },
        ],
      },
    };
  }
  if (p.includes('/admin/treasury-health')) {
    return {
      pnl_usd: {
        fees_earned_usd: '100000000',
        network_paid_usd: '5000000',
        faucet_cost_usd: '1000000',
        fee_margin_usd: '95000000',
        operating_margin_usd: '94000000',
        price_decimals: 8,
        prices_available: true,
      },
      break_even: [
        {
          coin: 'BTC',
          withdrawal_fee: '1000',
          avg_network_fee: '500',
          sample_count: 2,
          covers: true,
          gap: '500',
        },
      ],
      house_runway: [],
      pending_liabilities: [],
      hot_buffers: [
        {
          coin: 'BTC',
          onchain: '0',
          custody: '0',
          pending_out: '0',
          target: '20000',
          shortfall: '20000',
          status: 'low',
        },
      ],
      fee_series_7d: [],
      fee_series_30d: [],
      fee_margin_block: { enabled: true, blocked_coins: [] },
      unswept_deposits: [],
    };
  }
  if (p.includes('/admin/merchants/stats')) {
    return {
      accounts_total: 2,
      accounts_approved: 1,
      accounts_pending: 1,
      accounts_rejected: 0,
      invoices_all: { created: 12, paid: 8, pending: 2, expired: 2 },
      invoices_24h: { created: 3, paid: 2, pending: 1, expired: 0 },
      invoices_7d: { created: 7, paid: 5, pending: 1, expired: 1 },
      invoices_30d: { created: 11, paid: 8, pending: 2, expired: 1 },
      conversion_pct: 66.7,
      conversion_24h_pct: 66.7,
      conversion_7d_pct: 71.4,
      merchants_active_30d: 1,
      merchants_new_7d: 1,
      merchants_new_30d: 2,
      webhook_success_pct: 87.5,
      avg_confirm_minutes: 18.5,
      volume_by_coin: [
        {
          coin: 'BTC',
          paid_count: 5,
          amount: '50000000',
          fee_amount: '500000',
          net_amount: '49500000',
        },
      ],
      volume_by_coin_30d: [
        {
          coin: 'BTC',
          paid_count: 4,
          amount: '40000000',
          fee_amount: '400000',
          net_amount: '39600000',
        },
      ],
      api_keys_active: 3,
      api_keys_used_7d: 2,
      webhooks_delivered: 7,
      webhooks_failed: 1,
      top_merchants: [
        {
          merchant_id: 'm2',
          email: 'live@merchant.test',
          name: 'Live Store',
          paid_count: 5,
          volume_paid: '40000000',
          fees_paid: '400000',
        },
      ],
      series_14d: [
        { day: '2026-09-01', created: 2, paid: 1, expired: 0 },
        { day: '2026-09-05', created: 3, paid: 2, expired: 1 },
        { day: '2026-09-10', created: 1, paid: 1, expired: 0 },
      ],
      recent_invoices: [
        {
          id: 'inv1',
          merchant_id: 'm2',
          merchant_email: 'live@merchant.test',
          merchant_name: 'Live Store',
          coin: 'BTC',
          amount: '10000000',
          fee_amount: '100000',
          status: 'CONFIRMED',
          order_id: 'ord-1',
          site_name: 'shop.example',
          webhook_delivered: true,
          created_at: '2026-09-10T12:00:00Z',
          paid_at: '2026-09-10T12:20:00Z',
        },
      ],
    };
  }
  if (p.includes('/admin/merchants')) {
    return {
      merchants: [
        {
          id: 'm1',
          user_id: 'u1',
          email: 'shop@merchant.test',
          name: 'Demo Merchant',
          website_url: 'https://shop.example.com',
          webhook_url: 'https://shop.example.com/hook',
          description: 'Test merchant',
          status: 'PENDING',
          is_verified: false,
          created_at: '2024-06-01T08:00:00.000Z',
          invoices_total: 2,
          invoices_paid: 0,
          volume_paid: '0',
          fees_paid: '0',
          api_keys_count: 1,
          last_invoice_at: '2024-06-02T10:00:00.000Z',
        },
        {
          id: 'm2',
          user_id: 'u2',
          email: 'live@merchant.test',
          name: 'Live Store',
          website_url: 'https://live.example.com',
          status: 'APPROVED',
          is_verified: true,
          created_at: '2024-05-01T08:00:00.000Z',
          invoices_total: 10,
          invoices_paid: 8,
          volume_paid: '40000000',
          fees_paid: '400000',
          api_keys_count: 2,
          last_invoice_at: '2024-06-10T12:00:00.000Z',
        },
      ],
    };
  }
  if (p.includes('/admin/faucetlist')) return { sites: adminFaucetSites };
  if (p.includes('/admin/faucet')) return { sites: adminFaucetSites };
  if (p.includes('/admin/stake')) {
    return { plans: [], treasury: [], positions: [] };
  }
  if (p.includes('/oauth/authorized-apps')) {
    return [
      {
        application_id: 'authz1',
        name: 'Partner App',
        description: 'Connected OAuth partner',
        website_url: 'https://example.com',
        granted_scopes: 'openid profile',
        authorized_at: '2024-01-01T00:00:00.000Z',
      },
    ];
  }
  if (p.includes('/oauth/apps')) {
    return oauthAppsFixture;
  }
  if (p.includes('/auth/security-logs')) {
    return {
      logs: [
        {
          id: 'sl1',
          action: 'LOGIN_SUCCESS',
          ip: '1.2.3.4',
          metadata: { ua: 'vitest' },
          createdAt: '2024-06-01T12:00:00.000Z',
        },
      ],
    };
  }
  if (p.includes('/pricing')) {
    return {
      BTC: '8000000000000',
      LTC: '10000000000',
      USDT: '100000000',
      USDC: '100000000',
      DOGE: '10000000',
      BCH: '50000000000',
      POL: '50000000',
      DGB: '100000',
      SOL: '15000000000',
    };
  }
  if (p.includes('/oauth/authorize/info')) {
    return {
      app_id: 'app1',
      app_name: 'Partner App',
      description: 'Test OAuth app',
      website_url: 'https://example.com',
      redirect_uri: 'https://example.com/cb',
      scopes: ['openid', 'profile', 'email'],
      user: { id: '1', username: 'covuser', email: 'cov@bitcosats.test' },
    };
  }
  if (p.includes('/oauth/authorize')) {
    return { redirect_url: 'https://example.com/cb?code=abc&state=e2e' };
  }
  if (p.includes('/deposits/address') || p.includes('/deposits/address/')) {
    return { address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh' };
  }
  if (p.includes('/deposits/history')) {
    return {
      deposits: [
        {
          id: 'd1',
          coin: 'BTC',
          txHash: 'abc123def456abc123def456abc123def456abc123def456abc123def456abcd',
          vout: 0,
          amount: '10000000',
          confirmations: 3,
          minConfirmations: 2,
          status: 'CREDITED',
          detectedAt: '2024-06-01T12:00:00.000Z',
          creditedAt: '2024-06-01T12:30:00.000Z',
        },
        {
          id: 'd2',
          coin: 'LTC',
          txHash: 'fff123def456abc123def456abc123def456abc123def456abc123def456abce',
          vout: 1,
          amount: '50000000',
          confirmations: 1,
          minConfirmations: 6,
          status: 'PENDING',
          detectedAt: '2024-06-02T12:00:00.000Z',
        },
      ],
    };
  }
  if (p.includes('/withdrawals/addresses')) {
    return { addresses: [] };
  }
  if (p.includes('/withdrawals/history')) {
    return {
      withdrawals: [
        {
          id: 'w1',
          coin: 'BTC',
          toAddress: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
          amount: '5000000',
          feeAmount: '1000',
          status: 'CONFIRMED',
          txHash: 'abc123def456abc123def456abc123def456abc123def456abc123def456abcd',
          requiresApproval: false,
          createdAt: '2024-06-01T12:00:00.000Z',
          updatedAt: '2024-06-01T13:00:00.000Z',
        },
        {
          id: 'w2',
          coin: 'BTC',
          toAddress: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
          amount: '2000000',
          feeAmount: '1000',
          status: 'PENDING',
          txHash: null,
          requiresApproval: true,
          createdAt: '2024-06-03T12:00:00.000Z',
          updatedAt: '2024-06-03T12:00:00.000Z',
        },
      ],
    };
  }
  if (p.includes('/public/pay/') && p.includes('/balance')) {
    return { ok: true, status: 'CONFIRMED' };
  }
  if (p.includes('/public/pay/')) {
    const idMatch = p.match(/\/public\/pay\/([^/?]+)/);
    const invId = idMatch?.[1] ?? 'inv1';
    const base = {
      id: invId,
      coin: 'BTC' as const,
      amount: '100000',
      depositAddress: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      orderId: 'ord-1',
      siteName: 'Demo Shop',
      description: 'Test invoice',
      customerEmail: 'buyer@example.com',
      qrCode: 'bitcoin:bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      cancelUrl: 'https://shop.example.com/cancel',
    };
    if (/paid|confirm/i.test(invId)) {
      return {
        ...base,
        status: 'CONFIRMED',
        successUrl: 'https://shop.example.com/success',
        paidAt: '2024-06-01T14:00:00.000Z',
        txHash: 'abc123def456abc123def456abc123def456abc123def456abc123def456abcd',
        expiresAt: new Date(Date.now() + 3600_000).toISOString(),
      };
    }
    if (/expired|cancel/i.test(invId)) {
      return {
        ...base,
        status: invId.includes('cancel') ? 'CANCELLED' : 'EXPIRED',
        expiresAt: new Date(Date.now() - 60_000).toISOString(),
      };
    }
    if (/detected/i.test(invId)) {
      return {
        ...base,
        status: 'DETECTED',
        expiresAt: new Date(Date.now() + 1800_000).toISOString(),
      };
    }
    return {
      ...base,
      status: 'PENDING',
      expiresAt: new Date(Date.now() + 3600_000).toISOString(),
    };
  }
  if (p.includes('wallets') || p.includes('/wallet')) {
    const isMerchant = path.includes('MERCHANT');
    const isDev = path.includes('DEVELOPER') || isMerchant;
    const kind = isMerchant ? 'MERCHANT' : isDev ? 'DEVELOPER' : 'PERSONAL';
    const btcBal = isDev ? '50000000' : '100000000';
    const ltcBal = isDev ? '250000000' : '500000000';
    return {
      wallets: [
        {
          coin: 'BTC',
          balance: btcBal,
          lockedBalance: '0',
          kind,
          address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        },
        {
          coin: 'LTC',
          balance: ltcBal,
          lockedBalance: '0',
          kind,
          address: 'ltc1qtestaddress000000000000000000000',
        },
        {
          coin: 'USDT',
          balance: isDev ? '10000000000' : '20000000000',
          lockedBalance: '0',
          kind,
          address: '0x0000000000000000000000000000000000000000',
        },
      ],
    };
  }
  if (p.includes('/swap/history') || (p.includes('/swap') && p.includes('history'))) {
    return {
      swaps: [
        {
          id: 'sw-h1',
          fromCoin: 'BTC',
          toCoin: 'LTC',
          fromAmount: '1000000',
          toAmount: '80000000',
          status: 'COMPLETED',
          provider: 'HOUSE',
          source: 'house',
          createdAt: '2024-06-01T12:00:00.000Z',
        },
        {
          id: 'sw-h2',
          fromCoin: 'LTC',
          toCoin: 'BTC',
          fromAmount: '50000000',
          toAmount: '500000',
          status: 'PENDING',
          provider: 'HOUSE',
          source: 'house',
          createdAt: '2024-06-02T12:00:00.000Z',
        },
      ],
    };
  }
  if (p.includes('/swap/quote')) {
    const amt = {
      amount: '80000000',
      amountHuman: '0.8',
      coin: 'LTC',
    };
    return {
      routes: [
        {
          routeId: 'house',
          provider: 'HOUSE',
          providers: ['HOUSE'],
          tags: ['RECOMMENDED'],
          youPay: { amount: '1000000', amountHuman: '0.01', coin: 'BTC' },
          youReceive: amt,
          minReceive: amt,
          fees: {
            network: [],
            platform: {
              bps: 50,
              amount: '200000',
              amountHuman: '0.002',
              asset: 'LTC',
              label: 'SatsPay',
            },
            totalPlatformBps: 50,
          },
          etaSeconds: { total: 30 },
          txHint: null,
          source: 'HOUSE',
        },
        {
          routeId: 'swapkit-alt',
          provider: 'SWAPKIT',
          providers: ['SWAPKIT'],
          tags: [],
          youPay: { amount: '1000000', amountHuman: '0.01', coin: 'BTC' },
          youReceive: { amount: '79000000', amountHuman: '0.79', coin: 'LTC' },
          minReceive: { amount: '78000000', amountHuman: '0.78', coin: 'LTC' },
          fees: {
            network: [{ type: 'outbound', amount: '1000', asset: 'LTC' }],
            platform: {
              bps: 50,
              amount: '200000',
              amountHuman: '0.002',
              asset: 'LTC',
              label: 'SatsPay',
            },
            totalPlatformBps: 50,
          },
          etaSeconds: { total: 120 },
          txHint: null,
          source: 'swapkit',
        },
      ],
    };
  }
  if (p.includes('/swap/prices') || p.includes('/prices')) {
    return {
      priceDecimals: 8,
      prices: {
        BTC: '8000000000000',
        LTC: '10000000000',
        USDT: '100000000',
        USDC: '100000000',
        DOGE: '10000000',
        BCH: '50000000000',
        POL: '50000000',
        DGB: '100000',
        SOL: '15000000000',
      },
    };
  }
  if (p.includes('/faucet/status')) {
    return { cooldownMinutes: 660, coins: [] };
  }
  if (p.includes('/status')) {
    return {
      nodes: [{ coin: 'BTC', ok: true, latencyMs: 20, label: 'btc' }],
      ok: true,
    };
  }
  if (p.includes('/analytics')) {
    return {
      series: [{ day: '2024-01-01', in: '1', out: '0' }],
      totals: { usd: 100 },
      byType: [],
      allocation: [{ coin: 'BTC', usd: 100 }],
    };
  }
  if (p.includes('/faucetlist')) {
    return { sites: publicFaucetListSites };
  }
  if (p.includes('/faucet')) {
    return {
      sites: [],
      cooldownSeconds: 0,
      amount: '1',
      nextClaimAt: null,
      rewards: {},
    };
  }
  if (p.includes('/deposit') && !p.includes('/merchant')) {
    return {
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      invoices: [],
      deposits: [],
    };
  }
  if (p.includes('/withdraw')) {
    return { withdrawals: [], fee: '1000', history: [] };
  }
  if (p.includes('/stake')) return { positions: [], plans: [] };
  if (p.includes('/lend/markets')) return { markets: lendMarkets };
  if (p.includes('/lend/positions')) {
    return {
      positions: [
        { coin: 'USDT', supplied: '100000000', debt: '5000000', useAsCollateral: true },
        { coin: 'BTC', supplied: '0', debt: '0', useAsCollateral: false },
      ],
      debtUsd: '0',
      borrowPowerUsd: '500000000',
      availableBorrowUsd: '400000000',
      liquidationCollateralUsd: '450000000',
      healthFactorBps: 18500,
    };
  }
  if (p.includes('/lend')) return { positions: [], markets: lendMarkets };
  if (p.includes('/referral/stats')) {
    return {
      referral_code: 'ABC123',
      referral_link: 'https://www.satspay.pro/r/ABC123',
      total_referred: 5,
      active_referred_24h: 2,
      total_earned_usd: '42.50',
      faucet_commission_pct: 10,
      swap_commission_pct: 10,
      merchant_commission_pct: 0.1,
      earnings_by_coin: [{ coin: 'BTC', total_amount: '100000', total_usd: '8.00' }],
    };
  }
  if (p.includes('/referral/users')) {
    return {
      referred_users: [
        {
          id: 'ru1',
          username: 'refuser1',
          referral_code: 'REF1',
          is_active_24h: true,
          total_commissions_usd: '12.50',
          joined_at: '2024-06-01T00:00:00.000Z',
        },
      ],
    };
  }
  if (p.includes('/referral/commissions')) {
    return {
      commissions: [
        {
          id: 'com1',
          referred_id: 'ru1',
          activity_type: 'SWAP_FEE',
          coin: 'BTC',
          amount: '1000',
          amount_usd: '5.00',
          created_at: '2024-06-01T12:00:00.000Z',
        },
      ],
    };
  }
  if (p.includes('/referral')) {
    return { code: 'ABC123', stats: {}, invites: [] };
  }
  if (p.includes('/api-keys')) {
    return {
      keys: [
        {
          id: 'mk1',
          label: 'Merchant Live',
          keyPrefix: 'sats_m_live',
          scopes: ['deposits'],
          allowedIps: [],
          expiresAt: null,
          requireSignature: false,
          createdAt: '2024-01-01T00:00:00.000Z',
          disabledAt: null,
          lastUsedAt: '2024-06-01T10:00:00.000Z',
        },
      ],
    };
  }
  if (p.includes('/public/keys')) {
    return {
      keys: [
        {
          id: 'key1',
          label: 'Production',
          keyPrefix: 'sats_live_abc',
          scopes: ['deposits', 'send'],
          allowedIps: ['1.2.3.4'],
          expiresAt: null,
          requireSignature: false,
          createdAt: '2024-01-01T00:00:00.000Z',
          disabledAt: null,
          lastUsedAt: '2024-06-01T10:00:00.000Z',
        },
        {
          id: 'key2',
          label: 'Staging',
          keyPrefix: 'sats_test_xyz',
          scopes: ['deposits', 'history'],
          allowedIps: [],
          expiresAt: '2025-01-01T00:00:00.000Z',
          requireSignature: true,
          createdAt: '2024-02-01T00:00:00.000Z',
          disabledAt: null,
          lastUsedAt: null,
        },
      ],
    };
  }
  if (p.includes('/keys') || p.includes('/api-key')) return { keys: [] };
  if (p.includes('/merchant/sites')) {
    return {
      sites: [
        {
          id: 'site1',
          name: 'Demo Shop',
          url: 'https://shop.example.com',
          webhook_url: 'https://shop.example.com/hook',
          api_key_prefix: 'sats_m',
          is_active: true,
          created_at: '2024-01-01T00:00:00.000Z',
        },
      ],
    };
  }
  if (/\/merchant\/deposits\/[^/]+\/test-webhook/.test(p)) {
    if (p.includes('minv1')) return { delivered: true, statusCode: 200 };
    return { delivered: false, error: 'Connection reset by peer', statusCode: 502 };
  }
  if (p.includes('/merchant/deposits')) return { invoices: merchantDepositInvoices };
  if (p.includes('/merchant')) {
    return {
      sites: [
        {
          id: 'site1',
          name: 'Demo Shop',
          url: 'https://shop.example.com',
          api_key_prefix: 'sats_m',
          created_at: '2024-01-01T00:00:00.000Z',
        },
      ],
      invoices: merchantDepositInvoices,
      stats: {
        total_volume_usd: '1250.00',
        pending_invoices: 1,
        confirmed_invoices: 4,
        by_coin: [{ coin: 'BTC', volume: '50000000' }],
      },
    };
  }
  if (p.includes('/checkout')) {
    return {
      invoice: {
        id: 'inv1',
        amount: '1000',
        coin: 'BTC',
        status: 'PENDING',
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      },
    };
  }
  if (p.includes('security-log')) {
    return {
      logs: [
        {
          id: 'sl1',
          event: 'LOGIN_SUCCESS',
          ip: '1.2.3.4',
          userAgent: 'vitest',
          createdAt: '2024-06-01T12:00:00.000Z',
          created_at: '2024-06-01T12:00:00.000Z',
        },
        {
          id: 'sl2',
          event: 'PASSWORD_CHANGE',
          ip: '1.2.3.4',
          userAgent: 'vitest',
          createdAt: '2024-06-02T12:00:00.000Z',
          created_at: '2024-06-02T12:00:00.000Z',
        },
      ],
    };
  }
  if (p.includes('/auth') || p.includes('/me')) {
    return {
      user: {
        id: '11111111-1111-1111-1111-111111111111',
        email: 'cov@bitcosats.test',
        username: 'covuser',
        twoFactorEnabled: false,
        merchantStatus: 'APPROVED',
        createdAt: '2024-01-01T00:00:00.000Z',
        role: 'ADMIN',
      },
      logs: [],
    };
  }
  // default empty collections so `.length` checks don't explode
  return {
    logs: [],
    items: [],
    data: [],
    withdrawals: [],
    merchants: [],
    sites: [],
    keys: [],
    errors: [],
    swaps: [],
    deposits: [],
    routes: [],
  };
}
