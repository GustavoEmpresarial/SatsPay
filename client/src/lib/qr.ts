import QRCode from 'qrcode';

/** Build a PNG data-URL QR for deposit addresses (API no longer embeds QR). */
export async function addressQrDataUrl(address: string): Promise<string> {
  return QRCode.toDataURL(address, {
    errorCorrectionLevel: 'M',
    margin: 2,
    width: 256,
    color: { dark: '#000000', light: '#ffffff' },
  });
}
