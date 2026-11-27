//! 🗂️ 펫 고르기 메뉴의 묶음 (히어로 · 해리포터 · 크리스마스 · 할로윈 · 할로윈엔 흐물이도). 트레이의 "종류 바꾸기"·"🐾 펫 추가"가
//! 묶음마다 흐린 머리 칸을 하나 넣고 그 아래에 그대로 나열함 (하위 메뉴로 들어가지 않음). 모르는 종류는 동물로.

/// 펫 묶음임 (메뉴에 이 순서로 나옴)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PetGroup {
    /// 🦸 거미인간·박쥐인간·슈퍼인간·무쇠인간
    Hero,
    /// 🧙 해리·헤르미온느·볼드모트·디멘터·해그리드
    HarryPotter,
    /// 🎄 산타·루돌프·눈사람
    Christmas,
    /// 🎃 호박·흐물이
    Halloween,
    /// 🐾 동물들
    Animal,
}

impl PetGroup {
    /// 메뉴 순서
    pub const ALL: [PetGroup; 5] = [PetGroup::Hero, PetGroup::HarryPotter, PetGroup::Christmas, PetGroup::Halloween, PetGroup::Animal];

    /// 메뉴 머리 칸 글자
    pub fn label(self) -> &'static str {
        match self {
            PetGroup::Hero => "── 🦸 히어로 ──",
            PetGroup::HarryPotter => "── 🧙 해리포터 ──",
            PetGroup::Christmas => "── 🎄 크리스마스 ──",
            PetGroup::Halloween => "── 🎃 할로윈 ──",
            PetGroup::Animal => "── 🐾 동물 ──",
        }
    }

    /// 종류 id의 묶음 (모르는 것은 동물)
    pub fn of(species: &str) -> PetGroup {
        match species {
            "spider" | "bat" | "super" | "musoe" => PetGroup::Hero,
            "harry" | "hermione" | "voldemort" | "dementor" | "hagrid" => PetGroup::HarryPotter,
            "santa" | "rudolph" | "snowman" => PetGroup::Christmas,
            "pumpkin" | "ghost" | "heumul" => PetGroup::Halloween,
            _ => PetGroup::Animal,
        }
    }
}

/// 메뉴 줄 하나: 묶음 머리 칸이거나 고를 수 있는 것
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuRow<'a, T> {
    /// 흐린 머리 칸
    Header(PetGroup),
    /// 고를 수 있는 것
    Item(&'a T),
}

/// items를 묶음 순서로 늘어놓고 묶음마다 머리 칸을 앞에 넣음 (묶음 안에서는 원래 순서, 빈 묶음은 머리 칸도 없음)
pub fn menu_rows<'a, T>(items: &'a [T], species: impl Fn(&T) -> &str) -> Vec<MenuRow<'a, T>> {
    let mut rows = Vec::with_capacity(items.len() + PetGroup::ALL.len());
    for g in PetGroup::ALL {
        let mut first = true;
        for it in items.iter().filter(|it| PetGroup::of(species(it)) == g) {
            if first {
                rows.push(MenuRow::Header(g));
                first = false;
            }
            rows.push(MenuRow::Item(it));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_PETS: [&str; 20] = [
        "bat", "cat", "dementor", "dino_green", "dino_yellow", "hagrid", "harry", "hermione", "heumul", "musoe", "penguin", "pumpkin", "rabbit",
        "rudolph", "santa", "snowman", "spider", "squirrel", "super", "voldemort",
    ];

    fn shown(rows: &[MenuRow<'_, &str>]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                MenuRow::Header(g) => g.label().to_string(),
                MenuRow::Item(s) => s.to_string(),
            })
            .collect()
    }

    #[test]
    fn pets_are_listed_under_hero_harry_potter_christmas_halloween_and_animal_headers() {
        let rows = menu_rows(&ALL_PETS, |s| s);
        assert_eq!(
            shown(&rows),
            [
                "── 🦸 히어로 ──", "bat", "musoe", "spider", "super",
                "── 🧙 해리포터 ──", "dementor", "hagrid", "harry", "hermione", "voldemort",
                "── 🎄 크리스마스 ──", "rudolph", "santa", "snowman",
                "── 🎃 할로윈 ──", "heumul", "pumpkin",
                "── 🐾 동물 ──", "cat", "dino_green", "dino_yellow", "penguin", "rabbit", "squirrel",
            ]
        );
    }

    #[test]
    fn heumul_is_with_the_pumpkin_on_halloween_and_an_unknown_kind_is_an_animal() {
        // 사용자: 흐물이는 호박이랑 같이 할로윈에
        assert_eq!(PetGroup::of("heumul"), PetGroup::Halloween);
        assert_eq!(PetGroup::of("unicorn"), PetGroup::Animal);
    }

    #[test]
    fn an_empty_group_gets_no_header_and_every_pet_appears_once() {
        let rows = menu_rows(&["cat", "santa"], |s| s);
        assert_eq!(shown(&rows), ["── 🎄 크리스마스 ──", "santa", "── 🐾 동물 ──", "cat"]);
        let all = menu_rows(&ALL_PETS, |s| s);
        assert_eq!(all.iter().filter(|r| matches!(r, MenuRow::Item(_))).count(), ALL_PETS.len());
    }
}
