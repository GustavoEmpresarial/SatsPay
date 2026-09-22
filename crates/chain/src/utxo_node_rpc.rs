//! Bitcoin-style JSON-RPC against a self-hosted node with `disablewallet=1`.
//! Uses `scantxoutset` / `estimatesmartfee` / `sendrawtransaction` so deposits
//! and broadcasts work without `importaddress`.

use crate::btc_sign::Utxo;
use crate::types::{ChainError, OnchainTx};
use serde_json::{json, Value};

const SATOSHIS_PER_COIN: u128 = 100_000_000;

pub struct UtxoNodeRpc {
    http: reqwest::Client,
    endpoint: String,
    user: Option<String>,
    pass: Option<String>,
}

impl UtxoNodeRpc {
    pub fn new(rpc_url: &str) -> Result<Self, ChainError> {
        let (endpoint, auth) = split_rpc_endpoint(rpc_url)?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| ChainError { message: e.to_string() })?;
        Ok(Self {
            http,
            endpoint,
            user: auth.as_ref().map(|(u, _)| u.clone()),
            pass: auth.map(|(_, p)| p),
        })
    }

    pub(crate) async fn call(&self, method: &str, params: Value) -> Result<Value, ChainError> {
        let body = json!({
            "jsonrpc": "1.0",
            "id": "bitcosats",
            "method": method,
            "params": params,
        });
        let mut req = self.http.post(&self.endpoint).json(&body);
        if let (Some(user), Some(pass)) = (&self.user, &self.pass) {
            req = req.basic_auth(user, Some(pass));
        }
        let resp = req.send().await.map_err(|e| ChainError { message: e.to_string() })?;
        let status = resp.status();
        let v: Value = resp.json().await.map_err(|e| ChainError {
            message: format!("{method} HTTP {status}: {e}"),
        })?;
        if let Some(err) = v.get("error").filter(|e| !e.is_null()) {
            return Err(ChainError {
                message: format!("{method} RPC error: {err}"),
            });
        }
        v.get("result").cloned().ok_or_else(|| ChainError {
            message: format!("{method} missing result"),
        })
    }

    pub async fn scan_address(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        let scanned = self.scan_unspents(address).await?;
        Ok(scanned
            .into_iter()
            .map(|u| OnchainTx {
                tx_hash: u.txid,
                vout: u.vout,
                amount: u.value_sats as u128,
                confirmations: u.confirmations,
                address: address.to_string(),
            })
            .collect())
    }

    pub async fn fetch_utxos(&self, address: &str) -> Result<Vec<Utxo>, ChainError> {
        match self.scan_unspents(address).await {
            Ok(scanned) => {
                return Ok(scanned
                    .into_iter()
                    .map(|u| Utxo {
                        txid: u.txid,
                        vout: u.vout,
                        value: u.value_sats,
                        script_pubkey_hex: u.script_pubkey_hex,
                    })
                    .collect());
            }
            Err(e) => {
                // zerod (and some Zcash forks) lack scantxoutset; Insight
                // getaddressutxos works when experimentalfeatures+insightexplorer are on.
                tracing::warn!(error = %e.message, address, "scantxoutset failed; trying getaddressutxos");
            }
        }
        self.fetch_utxos_insight(address).await
    }

    /// Zcash Insight / addressindex `getaddressutxos`.
    /// zerod wants a JSON **array** param; Bitcoin Core style uses an object.
    pub async fn fetch_utxos_insight(&self, address: &str) -> Result<Vec<Utxo>, ChainError> {
        let result = match self.call("getaddressutxos", json!([{ "addresses": [address] }])).await {
            Ok(v) => v,
            Err(e) if e.message.contains("Params must be an array") || e.message.contains("-32600") => {
                self.call("getaddressutxos", json!({ "addresses": [address] })).await?
            }
            Err(e) => return Err(e),
        };
        let rows = result.as_array().ok_or_else(|| ChainError {
            message: format!("getaddressutxos unexpected: {result}"),
        })?;
        let mut out = Vec::with_capacity(rows.len());
        for u in rows {
            let txid = u
                .get("txid")
                .and_then(Value::as_str)
                .ok_or_else(|| ChainError {
                    message: "getaddressutxos row missing txid".into(),
                })?
                .to_string();
            let vout = u.get("outputIndex").or_else(|| u.get("vout")).and_then(Value::as_u64).ok_or_else(|| {
                ChainError {
                    message: "getaddressutxos row missing outputIndex".into(),
                }
            })? as u32;
            let value = u
                .get("satoshis")
                .and_then(Value::as_u64)
                .or_else(|| u.get("value").and_then(coin_amount_to_sats))
                .ok_or_else(|| ChainError {
                    message: "getaddressutxos row missing satoshis".into(),
                })?;
            let script_pubkey_hex = u
                .get("script")
                .or_else(|| u.get("scriptPubKey"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            out.push(Utxo {
                txid,
                vout,
                value,
                script_pubkey_hex,
            });
        }
        Ok(out)
    }

    pub async fn get_balance(&self, address: &str) -> Result<u128, ChainError> {
        Ok(self.scan_unspents(address).await?.iter().map(|u| u.value_sats as u128).sum())
    }

    pub async fn estimate_fee_sat_per_byte(&self) -> Result<u64, ChainError> {
        let result = self.call("estimatesmartfee", json!([2])).await?;
        let feerate = result.get("feerate").ok_or_else(|| ChainError {
            message: "estimatesmartfee missing feerate".into(),
        })?;
        feerate_to_sat_per_byte(feerate).ok_or_else(|| ChainError {
            message: format!("estimatesmartfee bad feerate: {feerate}"),
        })
    }

    pub async fn broadcast(&self, raw_hex: &str) -> Result<String, ChainError> {
        let result = self.call("sendrawtransaction", json!([raw_hex])).await?;
        result
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| ChainError {
                message: format!("sendrawtransaction unexpected result: {result}"),
            })
    }

    /// Build an unsigned raw tx. Amounts are decimal coin strings (`"1.00000000"`).
    /// `expiry_height` is required for Zcash-family coins when the signing node
    /// lags network tip — otherwise Insight rejects with `tx-expiring-soon`.
    pub async fn create_raw_transaction(
        &self,
        inputs: &[(String, u32)],
        outputs: &[(String, String)],
    ) -> Result<String, ChainError> {
        self.create_raw_transaction_ex(inputs, outputs, None).await
    }

    pub async fn create_raw_transaction_ex(
        &self,
        inputs: &[(String, u32)],
        outputs: &[(String, String)],
        expiry_height: Option<u64>,
    ) -> Result<String, ChainError> {
        let vins: Vec<Value> = inputs.iter().map(|(txid, vout)| json!({ "txid": txid, "vout": vout })).collect();
        let mut vouts = serde_json::Map::new();
        for (addr, amount) in outputs {
            vouts.insert(addr.clone(), json!(amount));
        }
        let params = match expiry_height {
            // locktime=0, expiryheight=<network tip + buffer>
            Some(h) => json!([vins, Value::Object(vouts), 0, h]),
            None => json!([vins, Value::Object(vouts)]),
        };
        let result = self.call("createrawtransaction", params).await?;
        result.as_str().map(str::to_string).ok_or_else(|| ChainError {
            message: format!("createrawtransaction unexpected: {result}"),
        })
    }

    pub async fn sign_raw_transaction_with_key(&self, raw_hex: &str, privkey: &str) -> Result<String, ChainError> {
        let result = self.call("signrawtransactionwithkey", json!([raw_hex, [privkey]])).await?;
        if result.get("complete").and_then(Value::as_bool) == Some(false) {
            return Err(ChainError {
                message: format!("signrawtransactionwithkey incomplete: {result}"),
            });
        }
        result
            .get("hex")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ChainError {
                message: format!("signrawtransactionwithkey missing hex: {result}"),
            })
    }

    /// Pre-0.17 name used by some Zcash-family daemons.
    /// Pass prevouts so signing works even when the node UTXO set is behind tip.
    pub async fn sign_raw_transaction_legacy(
        &self,
        raw_hex: &str,
        privkey: &str,
        prevouts: &[Utxo],
    ) -> Result<String, ChainError> {
        let prev: Vec<Value> = prevouts
            .iter()
            .map(|u| {
                json!({
                    "txid": u.txid,
                    "vout": u.vout,
                    "scriptPubKey": u.script_pubkey_hex,
                    "amount": format!("{}.{:08}", u.value / 100_000_000, u.value % 100_000_000),
                })
            })
            .collect();
        let result = self
            .call("signrawtransaction", json!([raw_hex, prev, [privkey]]))
            .await?;
        if result.get("complete").and_then(Value::as_bool) == Some(false) {
            return Err(ChainError {
                message: format!("signrawtransaction incomplete: {result}"),
            });
        }
        result.get("hex").and_then(Value::as_str).map(str::to_string).ok_or_else(|| ChainError {
            message: format!("signrawtransaction missing hex: {result}"),
        })
    }

    /// Try `sendrawtransaction` on this node. Used in a cascade of endpoints.
    pub async fn broadcast_raw(&self, raw_hex: &str) -> Result<String, ChainError> {
        self.broadcast(raw_hex).await
    }

    async fn scan_unspents(&self, address: &str) -> Result<Vec<ScannedUtxo>, ChainError> {
        let result = self
            .call("scantxoutset", json!(["start", [format!("addr({address})")]]))
            .await?;
        parse_scantxoutset(&result).ok_or_else(|| ChainError {
            message: format!("scantxoutset unreadable: {result}"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScannedUtxo {
    txid: String,
    vout: u32,
    value_sats: u64,
    script_pubkey_hex: String,
    confirmations: u32,
}

fn split_rpc_endpoint(raw: &str) -> Result<(String, Option<(String, String)>), ChainError> {
    let url = raw.trim();
    let (scheme, rest) = url.split_once("://").ok_or_else(|| ChainError {
        message: "RPC URL must include a scheme".into(),
    })?;
    if let Some(at) = rest.rfind('@') {
        let (creds, host) = rest.split_at(at);
        let host = &host[1..];
        let (user, pass) = creds.split_once(':').ok_or_else(|| ChainError {
            message: "RPC URL userinfo must be user:pass".into(),
        })?;
        Ok((
            format!("{scheme}://{}", host.trim_end_matches('/')),
            Some((percent_decode(user), percent_decode(pass))),
        ))
    } else {
        Ok((url.trim_end_matches('/').to_string(), None))
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((a << 4) | b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn parse_scantxoutset(result: &Value) -> Option<Vec<ScannedUtxo>> {
    let tip = result.get("height").and_then(Value::as_u64);
    let unspents = result.get("unspents")?.as_array()?;
    let mut out = Vec::with_capacity(unspents.len());
    for u in unspents {
        let txid = u.get("txid")?.as_str()?.to_string();
        let vout = u.get("vout")?.as_u64()? as u32;
        let value_sats = coin_amount_to_sats(u.get("amount")?)?;
        let script_pubkey_hex = u
            .get("scriptPubKey")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let utxo_height = u.get("height").and_then(Value::as_u64);
        let confirmations = match (tip, utxo_height) {
            (Some(scan), Some(h)) => scan.saturating_sub(h).saturating_add(1) as u32,
            _ => 1,
        };
        out.push(ScannedUtxo {
            txid,
            vout,
            value_sats,
            script_pubkey_hex,
            confirmations,
        });
    }
    Some(out)
}

fn coin_amount_to_sats(v: &Value) -> Option<u64> {
    let s = match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    parse_decimal_coins(&s)?.try_into().ok()
}

fn parse_decimal_coins(s: &str) -> Option<u128> {
    let s = s.trim();
    if s.starts_with('-') {
        return Some(0);
    }
    let (int_part, frac_part) = match s.split_once('.') {
        Some((a, b)) => (a, b),
        None => (s, ""),
    };
    let int_part = if int_part.is_empty() { "0" } else { int_part };
    let int: u128 = int_part.parse().ok()?;
    let digits: String = frac_part.chars().filter(|c| c.is_ascii_digit()).collect();
    let mut frac = digits;
    while frac.len() < 8 {
        frac.push('0');
    }
    let frac: u128 = frac.chars().take(8).collect::<String>().parse().ok()?;
    Some(int.saturating_mul(SATOSHIS_PER_COIN).saturating_add(frac))
}

/// `estimatesmartfee` feerate is coin/kvB → sat/vB.
fn feerate_to_sat_per_byte(v: &Value) -> Option<u64> {
    let sats_per_kvb = coin_amount_to_sats(v)? as u128;
    Some((sats_per_kvb / 1000).max(1) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn splits_userinfo_and_percent_decodes_password() {
        let (endpoint, auth) = split_rpc_endpoint("http://satspay:p%40ss@10.0.0.2:14022/").unwrap();
        assert_eq!(endpoint, "http://10.0.0.2:14022");
        assert_eq!(auth, Some(("satspay".into(), "p@ss".into())));
    }

    #[test]
    fn parses_scantxoutset_amounts_without_float() {
        let result = json!({
            "height": 100,
            "unspents": [{
                "txid": "aa".repeat(32),
                "vout": 1,
                "scriptPubKey": "0014deadbeef",
                "amount": "0.00010000",
                "height": 90
            }]
        });
        let parsed = parse_scantxoutset(&result).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].vout, 1);
        assert_eq!(parsed[0].value_sats, 10_000);
        assert_eq!(parsed[0].confirmations, 11);
        assert_eq!(parsed[0].script_pubkey_hex, "0014deadbeef");
    }

    #[test]
    fn feerate_converts_coin_per_kvb_to_sat_per_byte() {
        assert_eq!(feerate_to_sat_per_byte(&json!(0.00001000)), Some(1));
        assert_eq!(feerate_to_sat_per_byte(&json!("0.00002000")), Some(2));
    }
}
