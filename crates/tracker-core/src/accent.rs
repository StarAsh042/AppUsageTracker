//! 悬浮窗亚克力着色：把主题色名换成窗口合成层要的 ABGR 分量。

/// 主题色 → 亚克力着色的 (r, g, b, a)。alpha 固定 110，
/// 三通道各自与深色底 (16, 18, 24) 做 9:1 混合，避免纯色块过亮。
pub fn accent_tint(accent: &str) -> (u8, u8, u8, u8) {
    let (r, g, b) = match accent {
        "cyan" => (34u8, 211u8, 238u8),
        "violet" => (167, 139, 250),
        "green" => (52, 211, 153),
        "orange" => (251, 146, 60),
        "pink" => (244, 114, 182),
        _ => (79, 140, 255),
    };
    let mix = |base: u32, c: u32| ((base * 9 + c) / 10) as u8;
    (mix(16, r as u32), mix(18, g as u32), mix(24, b as u32), 110)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 已知主题色的混合结果() {
        // 16*9+34=178 → 17；18*9+211=373 → 37；24*9+238=454 → 45
        assert_eq!(accent_tint("cyan"), (17, 37, 45, 110));
        // green：144+52=196→19；162+211=373→37；216+153=369→36
        assert_eq!(accent_tint("green"), (19, 37, 36, 110));
    }

    #[test]
    fn 未知主题色回落到默认蓝() {
        // 144+79=223→22；162+140=302→30；216+255=471→47
        assert_eq!(accent_tint("whatever"), (22, 30, 47, 110));
        assert_eq!(accent_tint(""), accent_tint("default"));
    }

    #[test]
    fn alpha_恒为_110() {
        for a in ["cyan", "violet", "green", "orange", "pink", "x"] {
            assert_eq!(accent_tint(a).3, 110);
        }
    }
}
