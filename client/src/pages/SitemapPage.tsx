import { MarketingPage } from '../components/MarketingPage.js';
import { Link } from 'react-router-dom';

export function SitemapPage() {
  const links = [
    { to: '/', label: 'Home / Dashboard' },
    { to: '/welcome', label: 'Welcome / Landing' },
    { to: '/login', label: 'Sign In' },
    { to: '/register', label: 'Sign Up' },
    { to: '/wallets', label: 'Wallets' },
    { to: '/faucet', label: 'Faucet' },
    { to: '/faucetlist', label: 'Faucet Directory' },
    { to: '/swap', label: 'Swap' },
    { to: '/stake', label: 'Stake' },
    { to: '/lend', label: 'Lend' },
    { to: '/analytics', label: 'Analytics' },
    { to: '/documentation', label: 'Documentation' },
    { to: '/api', label: 'API Reference' },
    { to: '/privacy', label: 'Privacy Policy' },
    { to: '/terms', label: 'Terms of Service' },
    { to: '/cookies', label: 'Cookie Policy' },
    { to: '/security', label: 'Security' },
    { to: '/faq', label: 'FAQ' },
  ];

  return (
    <MarketingPage title="Sitemap" subtitle="Index of all available pages on BitcoSats">
      <ul className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {links.map((l) => (
          <li key={l.to}>
            <Link to={l.to} className="card p-3 text-sm font-medium hover:text-bitcoin-dark block">
              {l.label}
            </Link>
          </li>
        ))}
      </ul>
    </MarketingPage>
  );
}
