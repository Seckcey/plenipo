//! What a paid AI task costs (Phase 16 Wave 3, ADR-085; ADR-036 §2.5).
//!
//! A model's price is per million tokens, in millionths of a dollar ("micros"), for input, cached
//! input, and output. Before a paid request is sent, [`Price::most`] gives the most it could cost,
//! and the Ledger sets that amount aside under the owner's spending caps. After it, [`Price::bill`]
//! prices its token counts, unless the service sent its own bill ([`dollars_to_micros`]).
//! Everything is whole numbers, and every part is rounded **up**, so Plenipo never counts less
//! than was spent.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dto::TokenUsage;

/// One million: prices are per this many tokens, and a dollar is this many micros.
pub const MILLION: u64 = 1_000_000;
/// The dearest price Plenipo accepts: $100,000 per million tokens (a mistake, not a price).
pub const MAX_PRICE_MICROS: u64 = 100_000 * MILLION;

/// A model's price per million tokens, in micros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Price {
    /// Each million tokens sent in.
    #[ts(type = "number")]
    pub input: u64,
    /// Each million input tokens the service already had (cheaper where the service says so);
    /// none: they cost the same as other input.
    #[ts(type = "number | null")]
    pub cached_input: Option<u64>,
    /// Each million tokens written out.
    #[ts(type = "number")]
    pub output: u64,
    /// Each million input tokens the service stores for reuse, where it does so by itself and
    /// charges more for it (OpenAI's newest models, ADR-087). Plenipo cannot tell which tokens
    /// were stored, so it counts every fresh input token at this price when it is the dearer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub cache_write: Option<u64>,
}

/// `tokens` at `per_million` micros per million tokens, rounded up.
fn cost(tokens: u64, per_million: u64) -> u64 {
    let micros = (u128::from(tokens) * u128::from(per_million)).div_ceil(u128::from(MILLION));
    u64::try_from(micros).unwrap_or(u64::MAX)
}

impl Price {
    /// A price in dollars per million tokens (as the makers publish them), for built-in lists.
    pub const fn per_million_dollars(input: u64, output: u64) -> Self {
        Self {
            input: input * MILLION,
            cached_input: None,
            output: output * MILLION,
            cache_write: None,
        }
    }

    /// A price in micros per million tokens: input, cached input, and output.
    pub const fn micros(input: u64, cached_input: Option<u64>, output: u64) -> Self {
        Self {
            input,
            cached_input,
            output,
            cache_write: None,
        }
    }

    /// With a dearer price for input the service stores for reuse by itself.
    pub const fn with_cache_write(mut self, cache_write: u64) -> Self {
        self.cache_write = Some(cache_write);
        self
    }

    /// What each fresh input token is counted at: the input price, or the price of storing it
    /// for reuse when the service does that by itself and charges more.
    fn fresh_input(&self) -> u64 {
        match self.cache_write {
            Some(write) if write > self.input => write,
            _ => self.input,
        }
    }

    /// Each part is at most [`MAX_PRICE_MICROS`], and cached input costs no more than other
    /// input ([`Self::most`] counts every input token at the input price).
    pub fn is_sane(&self) -> bool {
        self.input <= MAX_PRICE_MICROS
            && self.output <= MAX_PRICE_MICROS
            && self.cache_write.is_none_or(|w| w <= MAX_PRICE_MICROS)
            && self.cached_input.is_none_or(|c| c <= self.input)
    }

    /// The most a request could cost: `input_tokens` sent in, none of them cached, and at most
    /// `max_output_tokens` written out.
    pub fn most(&self, input_tokens: u64, max_output_tokens: u64) -> u64 {
        cost(input_tokens, self.fresh_input()).saturating_add(cost(max_output_tokens, self.output))
    }

    /// What a finished request cost from its token counts. `input_tokens` includes the cached
    /// ones, as every AI tool reports them; a report with more cached tokens than input counted
    /// them apart, so all of its input is priced as fresh and its cached tokens on top (never
    /// less than was spent).
    pub fn bill(&self, usage: &TokenUsage) -> u64 {
        let (fresh, cached) = if usage.cached_input_tokens > usage.input_tokens {
            (usage.input_tokens, usage.cached_input_tokens)
        } else {
            (
                usage.input_tokens - usage.cached_input_tokens,
                usage.cached_input_tokens,
            )
        };
        cost(fresh, self.fresh_input())
            .saturating_add(cost(cached, self.cached_input.unwrap_or(self.input)))
            .saturating_add(cost(usage.output_tokens, self.output))
    }
}

/// An exact decimal number of dollars (`"0.0012345"`, `"12"`, `"3e-6"`) as `10^-scale` dollars,
/// rounded up past `scale` decimals. None for a negative, malformed, or too large number.
fn decimal(text: &str, scale: u32) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() || text.len() > 64 || text.starts_with('-') {
        return None;
    }
    let text = text.strip_prefix('+').unwrap_or(text);
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(i) => (&text[..i], text[i + 1..].parse::<i32>().ok()?),
        None => (text, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if whole.is_empty() && fraction.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    // All the digits as one integer, and where the decimal point goes.
    let digits: String = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0');
    let point = i64::from(exponent) - i64::try_from(fraction.len()).ok()? + i64::from(scale);
    if digits.is_empty() {
        return Some(0);
    }
    if point >= 0 {
        let shifted = u128::try_from(point).ok().filter(|p| *p <= 30)?;
        let value: u128 = digits.parse().ok()?;
        let value = value.checked_mul(10u128.checked_pow(u32::try_from(shifted).ok()?)?)?;
        return u64::try_from(value).ok();
    }
    // Fewer places than digits: cut, and round up if anything was cut.
    let cut = usize::try_from(-point).ok()?;
    if cut >= digits.len() {
        return Some(1);
    }
    let (kept, dropped) = digits.split_at(digits.len() - cut);
    let kept: u64 = kept.parse().ok()?;
    Some(if dropped.bytes().any(|b| b != b'0') {
        kept.checked_add(1)?
    } else {
        kept
    })
}

/// A number of dollars as micros, rounded up past the sixth decimal (a service's own bill:
/// OpenRouter's `usage.cost`).
pub fn dollars_to_micros(dollars: &str) -> Option<u64> {
    decimal(dollars, 6)
}

/// A price in dollars per token (`"0.000003"`, as OpenRouter lists them) as micros per million
/// tokens, rounded up.
pub fn per_token_to_per_million(dollars_per_token: &str) -> Option<u64> {
    decimal(dollars_per_token, 12)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(input: u64, cached: u64, output: u64) -> TokenUsage {
        TokenUsage {
            input_tokens: input,
            cached_input_tokens: cached,
            output_tokens: output,
        }
    }

    #[test]
    fn the_most_a_request_could_cost_rounds_up() {
        // Kimi K3 on OpenRouter: $3 in, $15 out per million tokens.
        let kimi = Price::per_million_dollars(3, 15);
        assert_eq!(kimi.input, 3_000_000);
        assert_eq!(kimi.most(1_000_000, 0), 3_000_000);
        assert_eq!(kimi.most(10_000, 4_000), 30_000 + 60_000);
        // One token is 3 micros in and 15 out.
        assert_eq!(kimi.most(1, 1), 18);
        let cheap = Price {
            input: 1,
            cached_input: None,
            output: 1,
            cache_write: None,
        };
        assert_eq!(
            cheap.most(1, 1),
            2,
            "a millionth of a micro rounds up to one"
        );
        assert_eq!(cheap.most(0, 0), 0);
        assert_eq!(
            Price::per_million_dollars(100_000, 100_000).most(u64::MAX, u64::MAX),
            u64::MAX
        );
    }

    #[test]
    fn a_bill_prices_cached_input_at_its_own_price() {
        let p = Price {
            input: 3_000_000,
            cached_input: Some(300_000),
            output: 15_000_000,
            cache_write: None,
        };
        // 1,000 fresh in, 9,000 cached in, 2,000 out.
        assert_eq!(p.bill(&usage(10_000, 9_000, 2_000)), 3_000 + 2_700 + 30_000);
        // No cached price: cached input costs the same as the rest.
        let flat = Price {
            cached_input: None,
            ..p
        };
        assert_eq!(flat.bill(&usage(10_000, 9_000, 0)), 30_000);
        // A report with more cached than input counted them apart: never less than was spent.
        assert_eq!(p.bill(&usage(10, 50, 0)), 30 + 15);
    }

    #[test]
    fn input_stored_for_reuse_by_the_service_is_counted_at_its_dearer_price() {
        // OpenAI's GPT-6.1 Sol: $2 in, $0.10 cached, $10 out, and $2.50 to store input for reuse.
        let p = Price::micros(2_000_000, Some(100_000), 10_000_000).with_cache_write(2_500_000);
        assert!(p.is_sane());
        // A million fresh input tokens at the storing price, none written out.
        assert_eq!(p.most(1_000_000, 0), 2_500_000);
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            cached_input_tokens: 400_000,
            output_tokens: 0,
        };
        // 600,000 fresh at $2.50, 400,000 cached at $0.10.
        assert_eq!(p.bill(&usage), 1_500_000 + 40_000);
        // A storing price below the input price changes nothing.
        let cheap = Price::micros(2_000_000, None, 0).with_cache_write(1_000_000);
        assert_eq!(cheap.most(1_000_000, 0), 2_000_000);
    }

    #[test]
    fn a_price_list_mistake_is_caught() {
        assert!(Price::per_million_dollars(3, 15).is_sane());
        assert!(!Price {
            input: MAX_PRICE_MICROS + 1,
            cached_input: None,
            output: 0,
            cache_write: None,
        }
        .is_sane());
        // Cached input dearer than other input would make the most a request could cost wrong.
        assert!(!Price {
            input: 1_000_000,
            cached_input: Some(2_000_000),
            output: 0,
            cache_write: None,
        }
        .is_sane());
    }

    #[test]
    fn dollars_are_read_exactly_and_rounded_up() {
        assert_eq!(dollars_to_micros("0"), Some(0));
        assert_eq!(dollars_to_micros("0.000000"), Some(0));
        assert_eq!(dollars_to_micros("12"), Some(12_000_000));
        assert_eq!(
            dollars_to_micros("0.0012345"),
            Some(1_235),
            "past six decimals rounds up"
        );
        assert_eq!(dollars_to_micros("0.001234"), Some(1_234));
        assert_eq!(
            dollars_to_micros("0.00123400"),
            Some(1_234),
            "trailing zeros cut nothing"
        );
        assert_eq!(dollars_to_micros(".5"), Some(500_000));
        assert_eq!(dollars_to_micros("5."), Some(5_000_000));
        assert_eq!(dollars_to_micros("1.5e-3"), Some(1_500));
        assert_eq!(dollars_to_micros("2E1"), Some(20_000_000));
        assert_eq!(
            dollars_to_micros("1e-9"),
            Some(1),
            "less than a micro is still one"
        );
        for bad in ["", "-1", "abc", "1.2.3", "1e", ".", "0x10", "1e99", "1 000"] {
            assert_eq!(dollars_to_micros(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_price_per_token_becomes_a_price_per_million_tokens() {
        assert_eq!(per_token_to_per_million("0.000003"), Some(3_000_000));
        assert_eq!(per_token_to_per_million("0.00000228"), Some(2_280_000));
        assert_eq!(per_token_to_per_million("0.0000006712"), Some(671_200));
        assert_eq!(per_token_to_per_million("0"), Some(0));
        assert_eq!(per_token_to_per_million("3e-6"), Some(3_000_000));
        assert_eq!(per_token_to_per_million("0.0000000000001"), Some(1));
        // OpenRouter lists "-1" for a price only known after the request.
        assert_eq!(per_token_to_per_million("-1"), None);
    }
}
