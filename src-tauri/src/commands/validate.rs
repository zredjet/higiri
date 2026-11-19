//! Input checks shared by the commands. Messages follow Aizome's rule:
//! what is wrong, then what to do.

use chrono::NaiveDate;

use super::error::{AppError, CmdResult};

/// Trimmed, non-empty text; `what` names the field (「タイトル」).
pub fn required_text(value: &str, what: &str) -> CmdResult<String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(AppError::validation(format!("{what}を入力してください。")));
    }
    Ok(v.to_owned())
}

/// Like [`required_text`] for an optional input.
pub fn required_opt_text(value: Option<&str>, what: &str) -> CmdResult<String> {
    required_text(value.unwrap_or(""), what)
}

/// `#RRGGBB`.
pub fn color(value: &str) -> CmdResult<String> {
    let v = value.trim();
    let ok = v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !ok {
        return Err(AppError::validation(
            "色は #3B6FD4 のように、# と6桁の英数字で指定してください。",
        ));
    }
    Ok(v.to_ascii_uppercase())
}

/// Start must not be after end; `what` is 「予定」 or 「実績」.
pub fn period(start: Option<NaiveDate>, end: Option<NaiveDate>, what: &str) -> CmdResult<()> {
    if let (Some(s), Some(e)) = (start, end)
        && s > e
    {
        return Err(AppError::validation(format!(
            "{what}開始日が{what}終了日より後になっています。日付を直してください。"
        )));
    }
    Ok(())
}

/// Number in `min..=max`; `what` names the field, `unit` follows the numbers.
pub fn in_range(value: i64, min: i64, max: i64, what: &str, unit: &str) -> CmdResult<()> {
    if value < min || value > max {
        return Err(AppError::validation(format!(
            "{what}は{min}〜{max}{unit}の範囲で入力してください。"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::d;

    #[test]
    fn required_text_trims_and_rejects_blank() {
        assert_eq!(
            required_text("  移行手順書 ", "タイトル").unwrap(),
            "移行手順書"
        );
        assert_eq!(
            required_text(" \n", "タイトル").unwrap_err().message,
            "タイトルを入力してください。"
        );
        assert!(required_opt_text(None, "中止の理由").is_err());
    }

    #[test]
    fn color_accepts_six_hex_digits() {
        assert_eq!(color("#3b6fd4").unwrap(), "#3B6FD4");
        for bad in ["3B6FD4", "#3B6FD", "#3B6FD4F", "#GGGGGG", ""] {
            assert!(color(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn period_rejects_reversed_dates() {
        assert!(period(Some(d("2026-11-19")), Some(d("2026-11-19")), "予定").is_ok());
        assert!(period(None, Some(d("2026-11-19")), "予定").is_ok());
        assert_eq!(
            period(Some(d("2026-11-20")), Some(d("2026-11-19")), "実績")
                .unwrap_err()
                .message,
            "実績開始日が実績終了日より後になっています。日付を直してください。"
        );
    }

    #[test]
    fn in_range_message() {
        assert!(in_range(3, 1, 30, "日数", "日").is_ok());
        assert_eq!(
            in_range(0, 1, 30, "「間近」とみなす日数", "日")
                .unwrap_err()
                .message,
            "「間近」とみなす日数は1〜30日の範囲で入力してください。"
        );
    }
}
