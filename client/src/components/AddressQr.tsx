import { useEffect, useState } from 'react';
import { addressQrDataUrl } from '../lib/qr.js';

interface Props {
  address: string;
  className?: string;
  sizeClassName?: string;
}

export function AddressQr({ address, className = '', sizeClassName = 'h-48 w-48' }: Props) {
  const [dataUrl, setDataUrl] = useState<string>('');

  useEffect(() => {
    let cancelled = false;
    addressQrDataUrl(address).then((url) => {
      if (!cancelled) setDataUrl(url);
    });
    return () => {
      cancelled = true;
    };
  }, [address]);

  if (!dataUrl) return <div className={`${sizeClassName} mx-auto animate-pulse rounded-lg bg-surface`} />;

  return (
    <div className={`flex justify-center ${className}`}>
      <div className="relative rounded-2xl border-2 border-border bg-white p-3 shadow-md transition-all hover:shadow-lg">
        <img
          src={dataUrl}
          alt="QR Code"
          className={`${sizeClassName} rounded-xl block`}
        />
      </div>
    </div>
  );
}
