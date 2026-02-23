use ratatui::style::Color;

pub struct NordTheme {
    // Polar Night
    pub nord0: Color, // #2E3440 - Самый тёмный фон. Панели, statusbar.
    pub nord1: Color, // #3B4252 - Вторичный фон. Активная строка, hover.
    pub nord2: Color, // #4c566a- Третичный фон. Разделители, borders.
    pub nord3: Color, // rgba(255, 255, 255, 1) - Мягкий акцент. Неактивный текст, комменты.

    // Snow Storm
    pub nord4: Color, // #D8DEE9 - Основной текст.
    pub nord5: Color, // #E5E9F0 - Яркий текст. Заголовки, выделения.
    pub nord6: Color, // #ECEFF4 - Белейший. Только для курсора и critical UI.

    // Frost
    pub nord7: Color,  // #8FBCBB - Мятный. Пути, хлебные крошки.
    pub nord8: Color,  // #88C0D0 - Ледяной синий. Ссылки, выделенный элемент.
    pub nord9: Color,  // #81A1C1 - Синий. Ключевые слова, операторы.
    pub nord10: Color, // #5E81AC - Тёмно-синий. Активные элементы, cursor block.

    // Aurora
    pub nord11: Color, // #BF616A - Красный. Ошибки, удаление, danger.
    pub nord12: Color, // #D08770 - Оранжевый. Предупреждения.
    pub nord13: Color, // #EBCB8B - Жёлтый. Метки, рейтинг (★★★).
    pub nord14: Color, // #A3BE8C - Зелёный. Успех, "read", confirmed.
    pub nord15: Color, // #B48EAD - Фиолетовый. AI-индикатор, magic.
}

impl Default for NordTheme {
    fn default() -> Self {
        Self {
            nord0: Color::Rgb(46, 52, 64),
            nord1: Color::Rgb(59, 66, 82),
            nord2: Color::Rgb(76, 86, 106),
            nord3: Color::Rgb(255, 255, 255),
            nord4: Color::Rgb(216, 222, 233),
            nord5: Color::Rgb(229, 233, 240),
            nord6: Color::Rgb(236, 239, 244),
            nord7: Color::Rgb(143, 188, 187),
            nord8: Color::Rgb(136, 192, 208),
            nord9: Color::Rgb(129, 161, 193),
            nord10: Color::Rgb(94, 129, 172),
            nord11: Color::Rgb(191, 97, 106),
            nord12: Color::Rgb(208, 135, 112),
            nord13: Color::Rgb(235, 203, 139),
            nord14: Color::Rgb(163, 190, 140),
            nord15: Color::Rgb(180, 142, 173),
        }
    }
}

impl NordTheme {
    // Semantic aliases
    pub fn bg(&self) -> Color {
        self.nord0
    }
    pub fn bg_secondary(&self) -> Color {
        self.nord1
    }
    pub fn border(&self) -> Color {
        self.nord2
    }
    pub fn muted(&self) -> Color {
        self.nord3
    }

    pub fn fg(&self) -> Color {
        self.nord4
    }
    pub fn fg_bright(&self) -> Color {
        self.nord5
    }
    pub fn fg_white(&self) -> Color {
        self.nord6
    }

    pub fn frost_mint(&self) -> Color {
        self.nord7
    }
    pub fn frost_ice(&self) -> Color {
        self.nord8
    }
    pub fn frost_blue(&self) -> Color {
        self.nord9
    }
    pub fn frost_dark(&self) -> Color {
        self.nord10
    }

    pub fn red(&self) -> Color {
        self.nord11
    }
    pub fn orange(&self) -> Color {
        self.nord12
    }
    pub fn yellow(&self) -> Color {
        self.nord13
    }
    pub fn green(&self) -> Color {
        self.nord14
    }
    pub fn purple(&self) -> Color {
        self.nord15
    }

    // Logic aliases
    pub fn cursor_bg(&self) -> Color {
        self.frost_dark()
    }
    pub fn cursor_fg(&self) -> Color {
        self.fg_white()
    }
    pub fn selection_bg(&self) -> Color {
        self.bg_secondary()
    }
    pub fn active_panel(&self) -> Color {
        self.frost_ice()
    }
    pub fn inactive_panel(&self) -> Color {
        self.muted()
    }
    pub fn tag_color(&self) -> Color {
        self.frost_blue()
    }
    pub fn path_color(&self) -> Color {
        self.frost_mint()
    }
    pub fn ai_color(&self) -> Color {
        self.purple()
    }
    pub fn danger(&self) -> Color {
        self.red()
    }
    pub fn success(&self) -> Color {
        self.green()
    }
    pub fn warning(&self) -> Color {
        self.orange()
    }
    pub fn star_color(&self) -> Color {
        self.yellow()
    }
}
