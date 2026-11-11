//! 추정 비용: 토큰을 API 정가로 환산한다. 요금표는 pricing.json에 있고 빌드할 때 함께 묶인다.
//! 요금은 자주 바뀌므로 화면에는 항상 「추정」으로 표시하고 기준 날짜를 함께 보여준다.

use crate::usage::Tokens;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Price {
    prefix: String,
    input: f64,
    cache_write: f64,
    /// 1시간 캐시 쓰기 요금. 적지 않으면 입력의 2배로 본다.
    cache_write_1h: Option<f64>,
    cache_read: f64,
    output: f64,
}

#[derive(Deserialize)]
struct Table {
    as_of: String,
    models: Vec<Price>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(include_str!("../pricing.json")).unwrap_or(Table { as_of: String::new(), models: Vec::new() })
    })
}

pub fn as_of() -> &'static str {
    &table().as_of
}

/// 모델 이름에 맞는 요금 줄의 번호. 앞부분이 가장 길게 맞는 줄을 고른다.
pub fn index_of(model: &str) -> Option<u16> {
    table()
        .models
        .iter()
        .enumerate()
        .filter(|(_, p)| model.starts_with(p.prefix.as_str()))
        .max_by_key(|(_, p)| p.prefix.len())
        .map(|(i, _)| i as u16)
}

/// 달러 단위 추정 비용.
pub fn cost(index: u16, t: &Tokens) -> f64 {
    let Some(p) = table().models.get(index as usize) else { return 0.0 };
    let long = t.cache_write_1h.min(t.cache_write);
    let short = t.cache_write - long;
    (t.input as f64 * p.input
        + short as f64 * p.cache_write
        + long as f64 * p.cache_write_1h.unwrap_or(p.input * 2.0)
        + t.cache_read as f64 * p.cache_read
        + t.output as f64 * p.output)
        / 1e6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 요금표가_읽히고_가장_길게_맞는_줄을_고른다() {
        assert!(!as_of().is_empty());
        let a = index_of("claude-opus-5-5").unwrap();
        let b = index_of("claude-opus-5").unwrap();
        assert_ne!(a, b);
        assert_eq!(index_of("claude-opus-5-5-20261001"), Some(a));
        assert_eq!(index_of("<synthetic>"), None);
        assert_eq!(index_of("모르는-모델"), None);
    }

    #[test]
    fn 비용은_종류별_단가를_곱해_더한다() {
        let i = index_of("claude-opus-5-5").unwrap();
        let t = Tokens { input: 1_000_000, cache_read: 2_000_000, cache_write: 1_000_000, output: 500_000, ..Default::default() };
        // 4 + 0.2*2 + 5 + 20*0.5 = 19.4
        assert!((cost(i, &t) - 19.4).abs() < 1e-9);
        assert_eq!(cost(9999, &t), 0.0);
    }
}
