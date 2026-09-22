/** Block explorer deep-links for withdrawal / deposit tx hashes. */

export function getExplorerTxUrl(coin: string, txHash: string): string {
  const c = coin.toUpperCase();
  switch (c) {
    case 'BTC':
      return `https://mempool.space/tx/${txHash}`;
    case 'LTC':
      return `https://litecoinspace.org/tx/${txHash}`;
    case 'DOGE':
      return `https://dogechain.info/tx/${txHash}`;
    case 'BCH':
      return `https://blockchair.com/bitcoin-cash/transaction/${txHash}`;
    case 'POL':
    case 'USDT':
    case 'USDC':
      return `https://polygonscan.com/tx/${txHash}`;
    case 'DGB':
      return `https://digiexplorer.info/tx/${txHash}`;
    case 'ZER':
      return `https://zerochain.info/tx/${txHash}`;
    case 'PEPE':
      return `https://bscscan.com/tx/${txHash}`;
    case 'SOL':
      return `https://solscan.io/tx/${txHash}`;
    default:
      return `https://polygonscan.com/tx/${txHash}`;
  }
}
